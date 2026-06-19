// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package profiletype

import (
	"fmt"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"text/template"

	"github.com/muktihari/rustyfit/fitgen/internal/generator"
	"github.com/muktihari/rustyfit/fitgen/internal/parser"
	"github.com/muktihari/rustyfit/fitgen/internal/pkg/strutil"
)

type Builder struct {
	template *template.Template

	path           string        // path to generate the file
	profileVersion string        // FIT SDK Profile Version
	types          []parser.Type // type parsed from profile.xlsx
}

var _ generator.Builder = (*Builder)(nil)

func NewBuilder(path, profileVersion string, types []parser.Type) *Builder {
	_, filename, _, _ := runtime.Caller(0)
	cd := filepath.Dir(filename)
	return &Builder{
		template: template.Must(template.New("main").
			ParseFiles(
				filepath.Join(cd, "profile_type.tmpl"),
			)),
		path:           filepath.Join(path, "profile"),
		profileVersion: profileVersion,
		types:          types,
	}
}

func (b *Builder) Build() ([]generator.Data, error) {
	profileDataBuilder := b.buildProfile()

	return []generator.Data{profileDataBuilder}, nil
}

func (b *Builder) buildProfile() generator.Data {
	constants := make([]Constant, 0, len(b.types))

	for _, t := range b.types {
		if t.Name == "fit_base_type" { // special types to be included, mapping to itself (profile.Uint8 == basetype.Uint8)
			for _, v := range t.Values {
				constantName := strutil.ToTitle(v.Name)
				constants = append(constants, Constant{
					Name:   constantName,
					String: v.Name,
				})
			}
			constants = append(constants, Constant{
				Name:   "Bool",
				String: "bool",
			},
			)
			break
		}
	}

	for _, t := range b.types {
		constants = append(constants, Constant{
			Name:   strutil.ToTitle(t.Name),
			String: t.Name,
		})
	}

	return generator.Data{
		Template:     b.template,
		TemplateExec: "profile_type",
		Path:         b.path,
		Filename:     "profile_type.rs",
		Data: ProfileData{
			VersionData: createVersionData(b.profileVersion),
			Constants:   constants,
		},
	}
}

func createVersionData(profileVersion string) VersionData {
	// On error, use panic so we can get stack trace, should not generate when version is invalid.
	parts := strings.Split(profileVersion, ".")
	if len(parts) < 2 {
		panic(fmt.Errorf("malformed profile version, should in the form of <major>.<minor>, got: %s", profileVersion))
	}
	var (
		majorPart = parts[0]
		minorPart = parts[1]
	)

	major, err := strconv.ParseUint(majorPart, 10, 16)
	if err != nil {
		panic(fmt.Errorf("invalid major version: %w", err))
	}
	minor, err := strconv.ParseUint(minorPart, 10, 16)
	if err != nil {
		panic(fmt.Errorf("invalid minor version: %w", err))
	}
	version, err := strconv.ParseUint(majorPart+minorPart, 10, 16)
	if err != nil {
		panic(fmt.Errorf("invalid version: %w", err))
	}

	return VersionData{
		ProfileVersion: profileVersion,
		Major:          uint16(major),
		Minor:          uint16(minor),
		Version:        uint16(version),
	}
}
