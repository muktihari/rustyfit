// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package rustfactory2

// Data represent factory.tmpl
type Data struct {
	MaxComponentBits    int
	TotalAccumulate     int
	TotalAccumulateList string
	Refs                string
	Names               []Name
	Units               []Unit
}

type Name struct {
	Variant string
	String  string
}

type Unit struct {
	Variant string
	String  string
}
