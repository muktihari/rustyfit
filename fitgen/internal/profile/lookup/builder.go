// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package rustfactory2

import (
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

	maxComponentBits int
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

	strbuf := new(strings.Builder)

	for _, message := range b.messages {
		constName := fmt.Sprintf("MesgNum::%s", strings.ToUpper(message.Name))

		fmt.Fprintf(strbuf, `
%s => { match field_num {
	%s
	_ => None,
	}
},`, constName, b.makeFieldRefs(message))
	}

	return []generator.Data{
		{
			Template:     b.template,
			TemplateExec: "lookup",
			Path:         b.path,
			Filename:     "lookup.rs",
			Data: Data{
				MaxComponentBits: b.maxComponentBits,
				Refs:             strbuf.String(),
			},
		},
	}, nil
}

func (b *Builder) makeFieldRefs(message parser.Message) string {
	if len(message.Fields) == 0 {
		return "nil"
	}

	strbuf := new(strings.Builder)
	for _, field := range message.Fields {
		// Scale and offset do not apply for field that has more than one components
		var scale, offset = 1.0, 0.0
		if len(field.Components) <= 1 {
			scale = scaleOrDefault(field.Scales, 0)
			offset = offsetOrDefault(field.Offsets, 0)
		}

		fmt.Fprintf(strbuf, `
%d => Some(FieldReference {
	name: %q,
	num: %d,
	base_type: FitBaseType::%s,
	profile_type: ProfileType::%s,
	array: %t %s,
	accumulate: %t,
	scale: %s,
	offset: %s,
	units: %q,
	components: %s,
	sub_fields: %s,
}),`,
			field.Num,
			field.Name,
			field.Num,
			strings.ToUpper(b.lookup.BaseType(field.Type).String()),
			strings.ToUpper(field.Type),
			field.Array != "", makeArrayComment(field.Array),
			accumulateOrDefault(field.Accumulate, 0),
			formatFloat(scale),
			formatFloat(offset),
			field.Units,
			b.makeComponents(field, message.Name),
			b.makeSubFields(field, message.Name),
		)
	}

	return strbuf.String()
}

func (b *Builder) makeComponents(compField parser.ComponentField, messageName string) string {
	if len(compField.GetComponents()) == 0 {
		return "&[]"
	}

	strbuf := new(strings.Builder)
	strbuf.WriteString("&[\n")
	var totalBits int
	for i, fieldNameRef := range compField.GetComponents() {
		fieldRef := b.lookup.FieldByName(messageName, fieldNameRef)
		strbuf.WriteString("Component {")
		fmt.Fprintf(strbuf, "field_num: %d /* %s */,", fieldRef.Num, fieldRef.Name)
		fmt.Fprintf(strbuf, "scale: %s,", formatFloat(scaleOrDefault(compField.GetScales(), i)))    // component index or default
		fmt.Fprintf(strbuf, "offset: %s,", formatFloat(offsetOrDefault(compField.GetOffsets(), i))) // component index or default
		fmt.Fprintf(strbuf, "accumulate: %t,", accumulateOrDefault(compField.GetAccumulate(), i))   // component index or default
		bits := bitsOrDefault(compField.GetBits(), i)                                               // component index or default
		fmt.Fprintf(strbuf, "bits: %d,", bits)
		strbuf.WriteString("},\n")

		totalBits += int(bits)
	}
	strbuf.WriteString("]")

	if totalBits > b.maxComponentBits {
		b.maxComponentBits = totalBits
	}

	return strbuf.String()
}

func (b *Builder) makeSubFields(field parser.Field, messageName string) string {
	if len(field.SubFields) == 0 {
		return "&[]"
	}

	strbuf := new(strings.Builder)
	strbuf.WriteString("&[\n")
	for _, subField := range field.SubFields {
		strbuf.WriteString("SubField {")
		fmt.Fprintf(strbuf, "name: %q,", subField.Name)
		fmt.Fprintf(strbuf, "profile_type: ProfileType::%s,", strings.ToUpper(subField.Type))
		fmt.Fprintf(strbuf, "scale: %s,", formatFloat(scaleOrDefault(subField.Scales, 0)))    // first index or default
		fmt.Fprintf(strbuf, "offset: %s,", formatFloat(offsetOrDefault(subField.Offsets, 0))) // first index or default
		fmt.Fprintf(strbuf, "units: %q,", subField.Units)
		if components := b.makeComponents(subField, messageName); components != "" {
			fmt.Fprintf(strbuf, "components: %s,", components)
		} else {
			strbuf.WriteString("components: &[],")
		}
		fmt.Fprintf(strbuf, "maps: %s,", b.makeSubFieldMaps(subField, messageName))
		strbuf.WriteString("},\n")
	}
	strbuf.WriteString("]")

	return strbuf.String()
}

func (b *Builder) makeSubFieldMaps(subfield parser.SubField, messageName string) string {
	if len(subfield.RefFieldNames) == 0 {
		return "&[]"
	}

	strbuf := new(strings.Builder)
	strbuf.WriteString("&[\n")
	for i, refValueName := range subfield.RefFieldNames {
		fieldRef := b.lookup.FieldByName(messageName, refValueName)
		strbuf.WriteString("SubFieldMap {")
		fmt.Fprintf(strbuf, "ref_field_num: %d /* %s */,", fieldRef.Num, fieldRef.Name)

		typeValue := b.lookup.TypeValue(fieldRef.Type, subfield.RefFieldValue[i])
		fmt.Fprintf(strbuf, "ref_field_value: %s /* %s */,", typeValue, subfield.RefFieldValue[i])
		strbuf.WriteString("},\n")
	}
	strbuf.WriteString("]")
	return strbuf.String()
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
