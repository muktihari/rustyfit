// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package profiletype

// ProfileData is data representative of profile.tmpl
type ProfileData struct {
	VersionData VersionData
	Constants   []Constant
}

// Constant represent declared constants within proto.
type Constant struct {
	Name     string
	BaseType string
	Value    string
	String   string
}

// VersionData is data representative of version.tmpl
type VersionData struct {
	ProfileVersion string
	Major          uint16
	Minor          uint16
	Version        uint16
}
