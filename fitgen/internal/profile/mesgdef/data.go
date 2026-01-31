// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package mesgdef

type Mod struct {
	SubMods []SubMod
}

type SubMod struct {
	Name     string
	Reexport string
}

type Message struct {
	Num               string
	Name              string
	NameSnakeCase     string
	Fields            []Field
	DynamicFields     []DynamicField
	StateSize         byte
	MaxFieldNum       byte
	MaxFieldExpandNum byte
}

type Field struct {
	Num              byte
	Name             string
	Name0            string // Name but with .0 if it's typedef
	NameUpperCase    string
	String           string
	ProfileType      string
	BaseType         string
	BaseType0        string // Underlying base_type, typedef::Weight -> u16
	BaseType0Invalid string // Invalid value of the underlying base type.
	MaxValue         string // Only if numeric
	Size             byte
	Type             string
	TypedValue       string
	PrimitiveValue   string
	ProtoValue       string
	IsValidValue     string
	ComparableValue  string
	InvalidValue     string
	Comment          string
	Units            string
	Scale            float64
	Offset           float64
	Array            bool
	CanExpand        bool

	FixedArraySize          byte
	InvalidArrayValueScaled string
}

type DynamicField struct {
	Name        string
	SwitchCases []SwitchCase
	Default     ReturnValue
}

type SwitchCase struct {
	Name       string
	CondValues []CondValue
}

type CondValue struct {
	Conds       []string
	ReturnValue ReturnValue
}

type ReturnValue struct {
	Name  string
	Units string
	Value string
}

type UtilData struct {
	Package      string
	MaxLenFields byte
}
