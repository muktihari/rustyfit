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
	Doc               string
	Num               string
	Name              string
	NameSnakeCase     string
	Fields            []Field
	KnownNums         [4]uint64
	StateSize         byte
	MaxFieldExpandNum byte
	Imports           map[string]struct{}
}

type Field struct {
	Num                   byte
	Name                  string
	Name0                 string // Name but with .0 if it's typedef
	NameRaw               string // Original Name retrieved from `Profile.xlsx`
	NameUpperCase         string
	String                string
	ProfileType           string
	BaseType              string
	BaseType0             string // Underlying base_type, typedef::Weight -> u16
	BaseType0Invalid      string // Invalid value of the underlying base type.
	MaxValue              string // Only if numeric
	Size                  byte
	Type                  string
	TypedValue            string
	PrimitiveValue        string
	ProtoValue            string
	IsValidValue          string
	ComparableValue       string
	InvalidValue          string
	InvalidValue0         string // Invalid value but with inner value if it's typedef: typedef::File(u32::MAX) -> u32::MAX
	Comment               string
	Units                 string
	Scale                 float64
	Offset                float64
	Array                 bool
	CanExpand             bool
	IfSelfEqualInvalid    string
	IfSelfNotEqualInvalid string
	IfNotEqualInvalid     string

	FixedArraySize          byte
	InvalidArrayValueScaled string
}

type UtilData struct {
	Package      string
	MaxLenFields byte
}
