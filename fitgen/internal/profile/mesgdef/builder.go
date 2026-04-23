// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package mesgdef

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
	"github.com/muktihari/rustyfit/fitgen/internal/pkg/strutil"
)

type Builder struct {
	template     *template.Template
	templateExec string

	packageName string

	path string // path to generate the file

	lookup   *lookup.Lookup
	messages []parser.Message
	types    []parser.Type
}

var _ generator.Builder = (*Builder)(nil)

func NewBuilder(path string, lookup *lookup.Lookup, message []parser.Message, types []parser.Type) *Builder {
	_, filename, _, _ := runtime.Caller(0)
	cd := filepath.Dir(filename)
	return &Builder{
		template: template.Must(template.New("main").
			Funcs(template.FuncMap{
				"formatFloat": func(f float64) string {
					if f == float64(int64(f)) {
						return strconv.FormatFloat(f, 'f', 1, 64)
					}
					return strconv.FormatFloat(f, 'g', -1, 64)
				},
			}).
			ParseFiles(filepath.Join(cd, "mesgdef.tmpl"))),
		templateExec: "mesgdef",
		packageName:  "mesgdef",
		path:         filepath.Join(path, "profile", "mesgdef"),
		lookup:       lookup,
		messages:     message,
		types:        types,
	}
}

func (b *Builder) Build() ([]generator.Data, error) {
	var messages = make([]Message, 0, len(b.messages))

	var maxLenFields int
	for _, mesg := range b.messages {
		canExpand, maxFieldExpandNum := b.componentExpansionAbility(&mesg)

		if len(mesg.Fields) > maxLenFields {
			maxLenFields = len(mesg.Fields)
		}
		var (
			knownNums     [4]uint64
			maxFieldNum   byte
			dynamicFields []DynamicField
			fields        = make([]Field, 0, len(mesg.Fields))
		)
		for _, parserField := range mesg.Fields {
			knownNums[parserField.Num>>6] |= 1 << (parserField.Num & 63)
			if parserField.Num > maxFieldNum {
				maxFieldNum = parserField.Num
			}

			var fixedArraySize uint8
			if len(parserField.Array) > 1 && parserField.Array[1] != 'N' {
				n := strings.TrimFunc(parserField.Array, func(r rune) bool {
					return r == '[' || r == ']'
				})
				v, err := strconv.ParseInt(n, 10, 8)
				if err != nil {
					return nil, fmt.Errorf("parse array size: %w", err)
				}
				fixedArraySize = uint8(v)
			}

			baseType := b.lookup.BaseType(parserField.Type)

			field := Field{
				Num:  parserField.Num,
				Name: strutil.ToNonRustIdent(parserField.Name),
				Name0: func() string {
					if parserField.Type == baseType.String() {
						return parserField.Name
					}
					return fmt.Sprintf("%s.0", parserField.Name)
				}(),
				NameUpperCase: strings.ToUpper(parserField.Name),
				String:        parserField.Name,
				BaseType:      strings.ToUpper(baseType.String()),
				BaseType0:     rustTypeReplacer.Replace(baseType.GoType()),
				BaseType0Invalid: func() string {
					if strings.HasSuffix(baseType.String(), "z") {
						return fmt.Sprintf("%s::MIN", rustTypeReplacer.Replace(baseType.GoType()))
					}
					return fmt.Sprintf("%s::MAX", rustTypeReplacer.Replace(baseType.GoType()))
				}(),
				ProfileType: fmt.Sprintf("ProfileType::%s", strings.ToUpper(parserField.Type)),
				MaxValue: func() string {
					bt := baseType.GoType()
					rtyp := rustTypeReplacer.Replace(bt)
					if bt == "string" {
						return ""
					}
					return fmt.Sprintf("%s::MAX", rtyp)
				}(),
				Type:           b.transformType(parserField.Type, parserField.Array, fixedArraySize),
				TypedValue:     b.transformTypedValue(parserField.Num, parserField.Type, parserField.Array, fixedArraySize),
				ProtoValue:     b.transformToProtoValue(strutil.ToNonRustIdent(parserField.Name), parserField.Type, parserField.Array, fixedArraySize),
				InvalidValue:   b.invalidValueOf(parserField.Type, parserField.Array, fixedArraySize),
				Comment:        parserField.Comment,
				Scale:          1,
				Offset:         0,
				Array:          parserField.Array != "",
				FixedArraySize: fixedArraySize,
				Units:          parserField.Units,
			}

			if _, ok := canExpand[parserField.Name]; ok {
				field.CanExpand = true
			}

			if parserField.Array == "" && field.BaseType == "string" {
				field.InvalidValue += fmt.Sprintf("&& %s != \"\"", field.ComparableValue)
			}

			// Scale and offset do not apply for field that has more than one components
			if len(parserField.Components) > 1 {
				field.Scale, field.Offset = 1, 0
			} else {
				field.Scale = scaleOrDefault(parserField.Scales, 0)
				field.Offset = offsetOrDefault(parserField.Offsets, 0)
			}

			if field.FixedArraySize > 0 && (field.Scale != 1 || field.Offset != 0) {
				field.InvalidArrayValueScaled = b.invalidArrayValueScaled(field.FixedArraySize)
			}

			field.Comment = createComment(&field, parserField.Array)

			fields = append(fields, field)

			if len(parserField.SubFields) == 0 {
				continue
			}

			dynamicFields = append(dynamicFields, b.createDynamicField(mesg.Name, &field, &parserField))
		}

		messages = append(messages, Message{
			Num:               strings.ToUpper(mesg.Name),
			NameSnakeCase:     mesg.Name,
			Name:              strutil.ToTitle(mesg.Name),
			Fields:            fields,
			DynamicFields:     dynamicFields,
			KnownNums:         knownNums,
			StateSize:         (maxFieldExpandNum + 8) / 8,
			MaxFieldNum:       maxFieldNum + 1,
			MaxFieldExpandNum: maxFieldExpandNum + 1,
		})
	}

	data := make([]generator.Data, 0, len(messages)+1)
	subMods := make([]SubMod, 0, len(messages))
	for _, mesg := range messages {
		data = append(data, generator.Data{
			Template:     b.template,
			TemplateExec: "mesgdef",
			Path:         b.path,
			Filename:     fmt.Sprintf("%s.rs", mesg.NameSnakeCase),
			Data:         mesg,
		})
		subMods = append(subMods, SubMod{
			Name:     mesg.NameSnakeCase,
			Reexport: mesg.Name,
		})
	}

	// mod.rs
	data = append(data, generator.Data{
		Template:     b.template,
		TemplateExec: "mod",
		Path:         b.path,
		Filename:     "mod.rs",
		Data:         Mod{SubMods: subMods},
	})

	return data, nil
}

// componentExpansionAbility checks whether fields or subfields have components that can be expanded.
// If they do, retrieve the largest field's number.
func (b *Builder) componentExpansionAbility(mesg *parser.Message) (canExpand map[string]byte, maxFieldExpandNum byte) {
	canExpand = make(map[string]byte)
	for _, field := range mesg.Fields {
		for _, component := range field.Components {
			ref := b.lookup.FieldByName(mesg.Name, component)
			canExpand[ref.Name] = ref.Num
			if ref.Num > maxFieldExpandNum {
				maxFieldExpandNum = ref.Num
			}
		}
		for _, subfield := range field.SubFields {
			for _, component := range subfield.Components {
				ref := b.lookup.FieldByName(mesg.Name, component)
				canExpand[ref.Name] = ref.Num
				if ref.Num > maxFieldExpandNum {
					maxFieldExpandNum = ref.Num
				}
			}
		}
	}
	return
}

func createComment(field *Field, array string) string {
	buf := new(strings.Builder)

	if strings.HasSuffix(field.BaseType, "z") {
		buf.WriteString("Base: ")
		buf.WriteString(field.BaseType)
		buf.WriteString("; ")
	}

	if strings.HasPrefix(field.Type, "[") {
		buf.WriteString("Array: ")
		buf.WriteString(array)
		buf.WriteString("; ")
	}

	if field.Scale != 1 {
		buf.WriteString("Scale: ")
		buf.WriteString(strconv.FormatFloat(field.Scale, 'g', -1, 64))
		buf.WriteString("; ")
	}

	if field.Offset != 0 {
		buf.WriteString("Offset: ")
		buf.WriteString(strconv.FormatFloat(field.Offset, 'g', -1, 64))
		buf.WriteString("; ")
	}

	if field.Units != "" {
		buf.WriteString("Units: ")
		buf.WriteString(field.Units)
		buf.WriteString("; ")
	}

	buf.WriteString(field.Comment)

	return strings.TrimSuffix(buf.String(), "; ")
}

func (b *Builder) createDynamicField(mesgName string, field *Field, parserField *parser.Field) DynamicField {
	var (
		rawSwitchCases      = make(map[string][]CondValue)
		rawSwitchCasesOrder = make(map[string]int)
		valuesOrder         = make(map[string]map[ReturnValue]int)
	)
	for _, subField := range parserField.SubFields {
		condValue := CondValue{
			ReturnValue: ReturnValue{
				Name:  subField.Name,
				Units: subField.Units,
			},
		}

		scale := scaleOrDefault(subField.Scales, 0)
		offset := offsetOrDefault(subField.Offsets, 0)
		if scale != 1 || offset != 0 {
			condValue.ReturnValue.Value = fmt.Sprintf("(float64(m.%s) * %g) - %g", field.Name, scale, offset)
		} else {
			condValue.ReturnValue.Value = fmt.Sprintf("%s(m.%s)", b.transformType(subField.Type, "", field.FixedArraySize), field.Name)
		}

		for i, refValueName := range subField.RefFieldNames {
			fieldRef := b.lookup.FieldByName(mesgName, refValueName)

			_, ok := rawSwitchCases[fieldRef.Name]
			if !ok {
				rawSwitchCasesOrder[fieldRef.Name] = len(rawSwitchCasesOrder)
				valuesOrder[fieldRef.Name] = make(map[ReturnValue]int)
			}

			valOrder, ok := valuesOrder[fieldRef.Name][condValue.ReturnValue]
			if !ok {
				valOrder = len(rawSwitchCases[fieldRef.Name])
				valuesOrder[fieldRef.Name][condValue.ReturnValue] = valOrder
				rawSwitchCases[fieldRef.Name] = append(rawSwitchCases[fieldRef.Name], condValue)
			}

			condValue = rawSwitchCases[fieldRef.Name][valOrder]
			condValue.Conds = append(condValue.Conds,
				fmt.Sprintf("%s%s",
					b.transformType(fieldRef.Type, fieldRef.Array, field.FixedArraySize), strutil.ToTitle(subField.RefFieldValue[i])))

			rawSwitchCases[fieldRef.Name][valOrder] = condValue
		}
	}

	switchCases := make([]SwitchCase, len(rawSwitchCases))
	for fieldNameRef, i := range rawSwitchCasesOrder {
		switchCases[i] = SwitchCase{
			Name:       fmt.Sprintf("m.%s", strutil.ToTitle(fieldNameRef)),
			CondValues: rawSwitchCases[fieldNameRef],
		}
	}

	return DynamicField{
		Name:        field.Name,
		SwitchCases: switchCases,
		Default: ReturnValue{
			Name:  parserField.Name,
			Units: parserField.Units,
			Value: fmt.Sprintf("m.%s", field.Name),
		},
	}
}

var rustTypeReplacer = strings.NewReplacer(
	"byte", "u8",
	"int", "i",
	"uint", "u",
	"float", "f",
	"string", "String",
)

func (b *Builder) transformType(fieldType, fieldArray string, fixedArraySize byte) string {
	var typ string
	if v := b.lookup.BaseType(fieldType).String(); v == fieldType {
		typ = rustTypeReplacer.Replace(b.lookup.GoType(fieldType))
	} else {
		typ = fmt.Sprintf("typedef::%s", strutil.ToTitle(fieldType))
	}

	if fieldArray == "" {
		return typ
	}

	if fixedArraySize > 0 {
		return fmt.Sprintf("[%s; %d]", typ, fixedArraySize)
	}

	return fmt.Sprintf("Vec<%s>", typ)
}

var baseTypeReplacer = strings.NewReplacer(
	"Enum", "Uint8",
	"Sint", "Int",
	"Byte", "Uint8",
)

func (b *Builder) transformToProtoValue(fieldName, fieldType, array string, fixedArraySize uint8) string {
	baseType := b.lookup.BaseType(fieldType).String()

	typ := strutil.ToTitle(baseType)
	typ = baseTypeReplacer.Replace(typ)
	typ = strings.TrimSuffix(typ, "z")

	if baseType != fieldType {
		if array != "" {
			return fmt.Sprintf(`Value::Vec%s({
				let mut v = Vec::with_capacity(m.%s.len());
				for x in &m.%s {
					v.push(x.0)
				}
				v
			})`, typ, fieldName, fieldName)
		}
		return fmt.Sprintf("Value::%s(m.%s.0)", typ, fieldName)
	}

	if array != "" {
		if fixedArraySize > 0 {
			return fmt.Sprintf("Value::Vec%s(Vec::from(&m.%s))", typ, fieldName)
		}
		return fmt.Sprintf("Value::Vec%s(m.%s)", typ, fieldName)
	}

	return fmt.Sprintf("Value::%s(m.%s)", typ, fieldName)
}

func (b *Builder) transformTypedValue(num byte, fieldType, array string, fixedArraySize uint8) string {
	baseType := b.lookup.BaseType(fieldType).String()
	baseTypeTitleCase := strutil.ToTitle(baseType)
	typ := baseTypeReplacer.Replace(baseTypeTitleCase)
	rustType := strings.NewReplacer(
		"enum", "u8",
		"byte", "u8",
		"sint", "i",
		"uint", "u",
		"float", "f",
	).Replace(baseType)

	if array != "" && strings.HasSuffix(typ, "z") {
		typ = strings.TrimSuffix(typ, "z")
	}

	var value string
	if array == "" {
		value = fmt.Sprintf(`vals[%d].as_%s()`, num, rustType)
	} else if fixedArraySize == 0 { // vector
		value = fmt.Sprintf(`vals[%d].as_vec_%s()`, num, strings.TrimSuffix(rustType, "z"))
	} else { // array
		rtype := rustTypeReplacer.Replace(strings.TrimSuffix(strings.ToLower(typ), "z"))
		arrayValue := fmt.Sprintf("[%s::MAX; %d]", rtype, fixedArraySize)
		rshValue := "*x"

		if fieldType == "string" {
			arrayValue = "Default::default()"
			rshValue = "x.to_owned()"
		}

		value = fmt.Sprintf(`match &vals[%d] {
			Value::Vec%s(v) => {
				let mut arr: [%s; %d] = %s;
				for (i, x) in v.iter().enumerate() {
					arr[i] = %s;
				}
				arr
			},
			_ => %s,
		}`,
			num,
			strings.TrimSuffix(typ, "z"),
			rtype, fixedArraySize, arrayValue,
			rshValue,
			arrayValue,
		)
	}

	if baseType == fieldType { // primitive-types
		return value
	}

	typdef := fmt.Sprintf("typedef::%s", strutil.ToTitle(fieldType))

	if array == "" {
		return fmt.Sprintf("%s(%s)", typdef, value)
	}

	return fmt.Sprintf(`match &vals[%d] {
		Value::Vec%s(v) => {
			let mut vs = Vec::with_capacity(v.len());
			for x in v {
				vs.push(%s(*x))
			}
			vs
		},
		_ => Vec::new(),
	}`, num, strings.TrimSuffix(typ, "z"), typdef)
}

func (b *Builder) invalidValueOf(fieldType, array string, fixedArraySize byte) string {
	baseType := b.lookup.BaseType(fieldType).String()
	baseTypeTitleCase := strutil.ToTitle(baseType)
	typ := baseTypeReplacer.Replace(baseTypeTitleCase)
	typ = strings.ToLower(typ)
	z := strings.HasSuffix(typ, "z")
	typ = rustTypeReplacer.Replace(strings.TrimSuffix(typ, "z"))
	var invalid string
	if baseType == "string" {
		invalid = "String::new()"
	} else {
		if z {
			invalid = fmt.Sprintf("%s::MIN", typ)
		} else {
			invalid = fmt.Sprintf("%s::MAX", typ)
		}
	}

	if array != "" {
		if fixedArraySize == 0 { // Slice
			if baseType == "string" {
				return "Vec::<String>::new()"
			}
			if baseType != fieldType {
				return fmt.Sprintf("Vec::<typedef::%s>::new()", strutil.ToTitle(fieldType))
			}
			return fmt.Sprintf("Vec::<%s>::new()", typ)
		}
		if baseType == "string" {
			invalid = fmt.Sprintf("const { %s }", invalid)
		}
		return fmt.Sprintf("[%s; %d]", invalid, int(fixedArraySize))
	}

	if baseType == fieldType {
		return invalid
	}

	return fmt.Sprintf("typedef::%s(%s)", strutil.ToTitle(fieldType), invalid)
}

func (b *Builder) invalidArrayValueScaled(fixedArraySize byte) string {
	return fmt.Sprintf(`[%d]float64{
		%s
	}`,
		fixedArraySize,
		strings.Repeat(
			"math.Float64frombits(basetype.Float64Invalid),\n",
			int(fixedArraySize),
		),
	)
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
