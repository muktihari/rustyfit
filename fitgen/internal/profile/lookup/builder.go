// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package rustfactory2

import (
	"bytes"
	"fmt"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"text/template"

	"github.com/muktihari/rustyfit/fitgen/internal/generator"
	"github.com/muktihari/rustyfit/fitgen/internal/lookup"
	"github.com/muktihari/rustyfit/fitgen/internal/parser"
)

type ( // type aliasing for better code reading.
	MessageName = string
	FieldName   = string
)

type Builder struct {
	template *template.Template

	path string // path to generate the file

	mesgnumPackageName string
	profilePackageName string

	lookup   *lookup.Lookup
	messages []parser.Message // messages parsed from profile.xlsx
	types    []parser.Type

	maxComponentBits    int
	totalAccumulate     int
	totalAccumulateList string
}

var _ generator.Builder = (*Builder)(nil)

func NewBuilder(path string, lookup *lookup.Lookup, types []parser.Type, messages []parser.Message) *Builder {
	_, filename, _, _ := runtime.Caller(0)
	cd := filepath.Dir(filename)
	f := &Builder{
		template:           template.Must(template.New("main").ParseFiles(filepath.Join(cd, "lookup.tmpl"))),
		path:               filepath.Join(path, "profile"),
		mesgnumPackageName: "typedef",
		profilePackageName: "profile",
		types:              types,
		messages:           messages,
		lookup:             lookup,
	}
	f.preproccessMessageField()
	return f
}

func (b *Builder) preproccessMessageField() {
	// Prepare lookup table for field indexes
	fieldIndexMapByMessageNameByFieldName := make(map[MessageName]map[FieldName]int)
	for _, message := range b.messages {
		fieldIndexMapByMessageNameByFieldName[message.Name] = make(map[FieldName]int)
		for i, field := range message.Fields {
			fieldIndexMapByMessageNameByFieldName[message.Name][field.Name] = i
		}
	}

	// NOTE: This is only a deduction since I can't find the proper explanation in the official documentation anywhere.
	// However, based on the example provided in the Official SDK, this seems to be the most sensible approach.
	//
	// Updating field's accumulate based on component ref:
	// When a field is being referred by components, accumulate value of that field is updated according to that component accumulate value.
	// For example "event_timestamp" accumulate value is false but it's is being referred as a component of "event_timestamp_12"
	// and that component accumulate is true so "event_timestamp" accumulate becomes true.
	for messageIndex, message := range b.messages {
		for _, field := range message.Fields {
			for i, fieldNameRef := range field.Components {
				indexFieldRef := fieldIndexMapByMessageNameByFieldName[message.Name][fieldNameRef]
				b.messages[messageIndex].Fields[indexFieldRef].Accumulate = []bool{accumulateOrDefault(field.Accumulate, i)}
			}
		}
	}
}

func (b *Builder) Build() ([]generator.Data, error) {
	// Create message/field lookup structure as string using strings.Builder{},
	// This way, we don't depend on generated value such as types and profile package to be able to generate factory.
	// And also we don't need to process the data in the template which is a bit painful for complex data structure.

	buf := new(bytes.Buffer)
	for _, message := range b.messages {
		constName := fmt.Sprintf("MesgNum::%s", strings.ToUpper(message.Name))

		fmt.Fprintf(buf, "\n%8s%s => { match field_num {\n%s %10s_ => None,\n%8s}},",
			"",
			constName,
			b.makeFieldRefs(message),
			"",
			"",
		)
	}

	return []generator.Data{
		{
			Template:     b.template,
			TemplateExec: "lookup",
			Path:         b.path,
			Filename:     "lookup.rs",
			Data: Data{
				MaxComponentBits:    b.maxComponentBits,
				TotalAccumulate:     b.totalAccumulate,
				TotalAccumulateList: b.totalAccumulateList,
				Refs:                buf.String(),
			},
		},
	}, nil
}

func (b *Builder) makeFieldRefs(message parser.Message) string {
	buf := new(bytes.Buffer)
	for _, field := range message.Fields {
		// Scale and offset do not apply for field that has more than one components
		var scale, offset = 1.0, 0.0
		if len(field.Components) <= 1 {
			scale = scaleOrDefault(field.Scales, 0)
			offset = offsetOrDefault(field.Offsets, 0)
		}

		fmt.Fprintf(buf, "%12s%d => Some(FieldReference { name: %q, num: %d, base_type: FitBaseType::%s, profile_type: ProfileType::%s, ",
			"",
			field.Num,
			field.Name,
			field.Num,
			strings.ToUpper(b.lookup.BaseType(field.Type).String()),
			strings.ToUpper(field.Type),
		)

		if field.Array != "" {
			fmt.Fprintf(buf, "array: %t %s, ", field.Array != "", makeArrayComment(field.Array))
		}

		accumulate := accumulateOrDefault(field.Accumulate, 0)
		if accumulate {
			fmt.Fprintf(buf, "accumulate: %t, ", accumulate)
			b.totalAccumulate += 1
			b.totalAccumulateList += fmt.Sprintf("/// - `%s`: %s\n", message.Name, field.Name)
		}

		if scale != 1 {
			fmt.Fprintf(buf, "scale: %s, ", formatFloat(scale))
		}

		if offset != 0 {
			fmt.Fprintf(buf, "offset: %s,", formatFloat(offset))
		}

		if field.Units != "" {
			fmt.Fprintf(buf, "units: %q, ", field.Units)
		}

		components := b.makeComponents(field, message.Name, "")
		if components != "&[]" {
			fmt.Fprintf(buf, "components: %s, ", components)
		}

		subFields := b.makeSubFields(field, message.Name)
		if subFields != "&[]" {
			fmt.Fprintf(buf, "sub_fields: %s, ", subFields)
		}

		if field.Array == "" ||
			!accumulate ||
			scale == 1 ||
			offset == 0 ||
			field.Units == "" ||
			components == "&[]" ||
			subFields == "&[]" {
			buf.WriteString("..FR_DEF, ")
		}

		buf.Truncate(buf.Len() - 2)
		buf.WriteString(" }),\n")
	}

	return buf.String()
}

func (b *Builder) makeComponents(compField parser.ComponentField, messageName string, extraPadding string) string {
	if len(compField.GetComponents()) == 0 {
		return "&[]"
	}

	buf := new(bytes.Buffer)
	buf.WriteString("&[")
	var totalBits int
	for i, fieldNameRef := range compField.GetComponents() {
		fieldRef := b.lookup.FieldByName(messageName, fieldNameRef)
		fmt.Fprintf(buf, "\n%19s%s Component { ", "", extraPadding)
		fmt.Fprintf(buf, "field_num: %d /* %s */, ", fieldRef.Num, fieldRef.Name)
		fmt.Fprintf(buf, "scale: %s, ", formatFloat(scaleOrDefault(compField.GetScales(), i)))    // component index or default
		fmt.Fprintf(buf, "offset: %s, ", formatFloat(offsetOrDefault(compField.GetOffsets(), i))) // component index or default
		fmt.Fprintf(buf, "accumulate: %t, ", accumulateOrDefault(compField.GetAccumulate(), i))   // component index or default
		bits := bitsOrDefault(compField.GetBits(), i)                                             // component index or default
		fmt.Fprintf(buf, "bits: %d", bits)
		buf.WriteString(" },")

		totalBits += int(bits)
	}
	buf.Truncate(buf.Len() - 1)
	fmt.Fprintf(buf, "\n%16s%s]", "", extraPadding)

	if totalBits > b.maxComponentBits {
		b.maxComponentBits = totalBits
	}

	return buf.String()
}

func (b *Builder) makeSubFields(field parser.Field, messageName string) string {
	if len(field.SubFields) == 0 {
		return "&[]"
	}

	buf := new(bytes.Buffer)
	buf.WriteString("&[")
	for _, subField := range field.SubFields {
		fmt.Fprintf(buf, "\n%19s SubField { ", "")
		fmt.Fprintf(buf, "name: %q, ", subField.Name)
		fmt.Fprintf(buf, "profile_type: ProfileType::%s, ", strings.ToUpper(subField.Type))

		scale := scaleOrDefault(subField.Scales, 0) // first index or default
		if scale != 1 {
			fmt.Fprintf(buf, "scale: %s, ", formatFloat(scale))
		}

		offset := offsetOrDefault(subField.Offsets, 0) // first index or default
		if offset != 0 {
			fmt.Fprintf(buf, "offset: %s, ", formatFloat(offset))
		}

		if subField.Units != "" {
			fmt.Fprintf(buf, "units: %q, ", subField.Units)
		}

		components := b.makeComponents(subField, messageName, strings.Repeat(" ", 4))
		if components != "&[]" {
			fmt.Fprintf(buf, "components: %s, ", components)
		}

		subFields := b.makeSubFieldMaps(subField, messageName)
		if subFields != "&[]" {
			fmt.Fprintf(buf, "maps: %s, ", subFields)
		}

		if scale == 1 ||
			offset == 0 ||
			field.Units == "" ||
			components == "&[]" ||
			subFields == "&[]" {
			buf.WriteString("..SF_DEF, ")
		}

		buf.Truncate(buf.Len() - 2)
		buf.WriteString(" },")
	}
	buf.Truncate(buf.Len() - 1)
	fmt.Fprintf(buf, "\n%16s]", "")

	return buf.String()
}

func (b *Builder) makeSubFieldMaps(subfield parser.SubField, messageName string) string {
	if len(subfield.RefFieldNames) == 0 {
		return "&[]"
	}

	buf := new(bytes.Buffer)
	buf.WriteString("&[")
	for i, refValueName := range subfield.RefFieldNames {
		fieldRef := b.lookup.FieldByName(messageName, refValueName)
		fmt.Fprintf(buf, "\n%23s SubFieldMap { ", "")
		fmt.Fprintf(buf, "ref_field_num: %d /* %s */, ", fieldRef.Num, fieldRef.Name)

		typeValue := b.lookup.TypeValue(fieldRef.Type, subfield.RefFieldValue[i])
		fmt.Fprintf(buf, "ref_field_value: %s /* %s */ ", typeValue, subfield.RefFieldValue[i])
		buf.WriteString("},")
	}
	fmt.Fprintf(buf, "\n%20s]", "")
	return buf.String()
}

func bitsOrDefault(bits []byte, index int) byte {
	if index < len(bits) {
		return bits[index]
	}
	return 0
}

// Profile.xlsx says unless otherwise specified, scale of 1 is assumed.
func scaleOrDefault(scales []float64, index int) float64 {
	if index < len(scales) {
		return scales[index]
	}
	return 1.0
}

// Profile.xlsx says unless otherwise specified, offset of 0 is assumed.
func offsetOrDefault(offsets []float64, index int) float64 {
	if index < len(offsets) {
		return offsets[index]
	}
	return 0.0
}

func accumulateOrDefault(accumulates []bool, index int) bool {
	if index < len(accumulates) {
		return accumulates[index]
	}
	return false
}

func makeArrayComment(arr string) string {
	if arr == "" {
		return ""
	}
	return fmt.Sprintf("/* %s */", arr)
}

func formatFloat(v float64) string {
	if float64(int64(v)) == v {
		return fmt.Sprintf("%.1f", v)
	}
	return strconv.FormatFloat(v, 'g', -1, 64)
}
