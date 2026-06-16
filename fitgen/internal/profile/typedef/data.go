// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package typedef

type Mod struct {
	SubMods []SubMod
}

type SubMod struct {
	Name     string
	Reexport string
}

type Type struct {
	Doc       string
	TypeName  string
	Base      string
	Invalid   string
	Constants []Constant
}

type Constant struct {
	Decorator   string
	Name        string
	Op          string
	Value       string
	String      string
	Comment     string
	IsDuplicate bool
}
