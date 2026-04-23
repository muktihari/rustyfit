// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package typedef

import (
	"path/filepath"
	"runtime"
	"strings"
	"text/template"

	"github.com/muktihari/rustyfit/fitgen/internal/basetype"
	"github.com/muktihari/rustyfit/fitgen/internal/generator"
	"github.com/muktihari/rustyfit/fitgen/internal/parser"
	"github.com/muktihari/rustyfit/fitgen/internal/pkg/strutil"
)

const (
	FitBaseType string = "fit_base_type"
)

type Builder struct {
	template     *template.Template
	templateExec string

	path  string        // path to generate the file
	types []parser.Type // type parsed from profile.xlsx
}

var _ generator.Builder = (*Builder)(nil)

func NewBuilder(path string, types []parser.Type) *Builder {
	_, filename, _, _ := runtime.Caller(0)
	cd := filepath.Dir(filename)
	return &Builder{
		template: template.Must(template.New("main").
			ParseFiles(
				filepath.Join(cd, "typedef.tmpl"),
			)),
		templateExec: "typedef",
		path:         filepath.Join(path, "profile", "typedef"),
		types:        types,
	}
}

func (b *Builder) Build() ([]generator.Data, error) {
	data := make([]generator.Data, 0, len(b.types)+2)
	subMods := make([]SubMod, 0, len(b.types)+1)

	// additional type: bool
	data = append(data, generator.Data{
		Template:     b.template,
		TemplateExec: b.templateExec,
		Path:         b.path,
		Filename:     "bool.rs",
		Data: Type{
			TypeName: "Bool",
			Base:     "u8",
			Invalid:  "u8::MAX",
			Constants: []Constant{
				{
					Name:   "FALSE",
					Value:  "0",
					String: "false",
				},
				{
					Name:   "TRUE",
					Value:  "1",
					String: "true",
				},
			},
		},
	})

	subMods = append(subMods, SubMod{
		Name:     "bool",
		Reexport: "Bool",
	})

	for _, t := range b.types {
		typeName := strutil.ToTitle(t.Name)

		duplicates := make(map[string]int)
		constants := make([]Constant, 0, len(t.Values))
		for _, v := range t.Values {
			duplicates[v.Value]++
			constants = append(constants, Constant{
				Name:    strutil.ToLetterPrefix(strings.ToUpper(v.Name)),
				Value:   v.Value,
				String:  v.Name,
				Comment: v.Comment,
			})
		}

		// handling duplicate values caused by deprecated
		for value, count := range duplicates {
			if count == 1 {
				continue
			}
			for i := range constants {
				if constants[i].Value != value {
					continue
				}
				comment := strings.ToLower(constants[i].Comment)
				if strings.Contains(comment, "deprecated") {
					constants[i].Decorator = "// " + constants[i].Decorator
					constants[i].Comment = "[DUPLICATE!] " + constants[i].Comment
					constants[i].IsDuplicate = true
				}
			}
		}

		for i := range constants {
			if constants[i].Comment != "" {
				constants[i].Comment = "/// " + constants[i].Comment
			}
		}

		data = append(data, generator.Data{
			Template:     b.template,
			TemplateExec: b.templateExec,
			Path:         b.path,
			Filename:     t.Name + ".rs",
			Data: Type{
				TypeName: typeName,
				Base:     intoRustType(basetype.FromString(t.BaseType)),
				Invalid: func() string {
					rt := intoRustType(basetype.FromString(t.BaseType))
					if strings.HasSuffix(t.BaseType, "z") {
						return rt + "::MIN"
					}
					return rt + "::MAX"
				}(),
				Constants: constants,
			},
		})

		subMods = append(subMods, SubMod{
			Name:     t.Name,
			Reexport: typeName,
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

func intoRustType(bt basetype.BaseType) string {
	switch bt {
	case basetype.Enum, basetype.Uint8, basetype.Uint8z, basetype.Byte:
		return "u8"
	case basetype.Sint8:
		return "i8"
	case basetype.Sint16:
		return "i16"
	case basetype.Uint16, basetype.Uint16z:
		return "u16"
	case basetype.Sint32:
		return "i32"
	case basetype.Uint32, basetype.Uint32z:
		return "u32"
	case basetype.Float32:
		return "f32"
	case basetype.Float64:
		return "f64"
	case basetype.Sint64:
		return "i64"
	case basetype.Uint64, basetype.Uint64z:
		return "u64"
	default:
		return "<unsupported>"
	}
}
