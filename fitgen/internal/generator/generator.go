// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package generator

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"text/template"
)

// Builder is template data builder
type Builder interface {
	// Build returns slice of template data and an error
	Build() ([]Data, error)
}

// Data wraps template and data for generating proto.
type Data struct {
	Template     *template.Template
	TemplateExec string
	Path         string
	Filename     string
	Data         any
}

// New return a generator
func New(verbose bool) *Generator {
	return &Generator{
		verbose: verbose,
		buf:     bytes.NewBuffer(nil),
	}
}

type Generator struct {
	verbose bool
	buf     *bytes.Buffer
}

func (g *Generator) Generate(builders []Builder, perm os.FileMode) error {
	for _, builder := range builders {
		dataBuilders, err := builder.Build()
		if err != nil {
			return err
		}
		for _, dataBuilder := range dataBuilders {
			if err := g.GenerateTemplateData(dataBuilder, perm); err != nil {
				return err
			}
		}
	}

	cmd := exec.Command("cargo", "fmt", "--all")
	cmd.Stderr = g.buf
	if err := cmd.Run(); err != nil {
		return fmt.Errorf("cmd: %s: err: %w", g.buf.String(), err)
	}

	return nil
}

func (g *Generator) GenerateTemplateData(dataBuilder Data, perm os.FileMode) error {
	if err := os.MkdirAll(dataBuilder.Path, 0755); err != nil {
		return fmt.Errorf("mkdir %q: %w", dataBuilder.Path, err)
	}
	name := filepath.Join(dataBuilder.Path, dataBuilder.Filename)
	name = abspath(name)

	g.buf.Reset()
	if err := dataBuilder.Template.ExecuteTemplate(g.buf, dataBuilder.TemplateExec, dataBuilder.Data); err != nil {
		return fmt.Errorf("template exec templ %q: %w", name, err)
	}

	if err := os.WriteFile(name, g.buf.Bytes(), perm); err != nil {
		return fmt.Errorf("write file %q: %w", name, err)
	}

	if g.verbose { // Print the list of written files.
		fmt.Println(name)
	}

	return nil
}

func abspath(path string) string {
	abspath, err := filepath.Abs(path)
	if err != nil {
		return path
	}
	return abspath
}
