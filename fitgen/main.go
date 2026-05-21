// Copyright 2023 The FIT SDK for Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

package main

import (
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/muktihari/rustyfit/fitgen/internal/generator"
	"github.com/muktihari/rustyfit/fitgen/internal/lookup"
	"github.com/muktihari/rustyfit/fitgen/internal/parser"

	"github.com/muktihari/rustyfit/fitgen/internal/pkg/xlsxlite"
	pl "github.com/muktihari/rustyfit/fitgen/internal/profile/lookup"
	md "github.com/muktihari/rustyfit/fitgen/internal/profile/mesgdef"
	pt "github.com/muktihari/rustyfit/fitgen/internal/profile/profiletype"
	td "github.com/muktihari/rustyfit/fitgen/internal/profile/typedef"

	"github.com/thedatashed/xlsxreader"
)

func main() {
	var (
		generatePath   = flag.String("p", "", "root/path/to/generate/files")
		whichBuilder   = flag.String("b", "", "Value separated by comma: lookup,profile_type")
		profileVersion = flag.String("v", "", "Garmin FIT SDK Profile Version e.g. \"21.158\"")
	)

	flag.Parse()

	if *generatePath == "" {
		fatalf("missing flag: --p=root/path/to/generate/files\n")
	}

	if *profileVersion == "" {
		fatalf("missing flag: -v=<version> e.g 21.158\n")
	}

	xlsxreader, err := xlsxreader.OpenFile("./Profile.xlsx")
	if err != nil {
		fatalf("could not open Profile.xlsx: %v\n", err)
	}
	defer xlsxreader.Close()

	ps := parser.New(xlsxlite.New(xlsxreader), map[parser.Sheet]string{
		parser.SheetTypes:    "Types",    // maps the actual sheet name in the file
		parser.SheetMessages: "Messages", // to the one that the parser is using.
	})

	parsedtypes, err := ps.ParseTypes()
	if err != nil {
		fatalf("could no parse types: %v\n", err)
	}
	parsedmesgs, err := ps.ParseMessages()
	if err != nil {
		fatalf("could no parse message: %v\n", err)
	}

	if filepath.Base(*generatePath) != "src" {
		*generatePath = filepath.Join(*generatePath, "src")
	}

	path := abspath(*generatePath)
	lookup := lookup.New(parsedtypes, parsedmesgs)
	var (
		lookupBuilder      = pl.NewBuilder(path, lookup, parsedtypes, parsedmesgs)
		profileTypeBuilder = pt.NewBuilder(path, *profileVersion, parsedtypes)
		typedefBuilder     = td.NewBuilder(path, parsedtypes)
		mesgdefBuilder     = md.NewBuilder(path, lookup, parsedmesgs, parsedtypes)
	)

	var builders []generator.Builder
	whichBuilders := strings.Split(*whichBuilder, ",")

loop:
	for _, selected := range whichBuilders {
		switch s := strings.TrimSpace(selected); s {
		case "", "all":
			builders = []generator.Builder{lookupBuilder, profileTypeBuilder, typedefBuilder, mesgdefBuilder}
			break loop
		case "lookup":
			builders = append(builders, lookupBuilder)
		case "profile_type":
			builders = append(builders, profileTypeBuilder)
		case "typedef":
			builders = append(builders, typedefBuilder)
		case "mesgdef":
			builders = append(builders, mesgdefBuilder)
		default:
			fatalf("invalid builder name: %q\n", strings.TrimSpace(selected))
		}
	}

	if err := generator.New(true).Generate(builders, 0o755); err != nil {
		fatalf("could not generate files: %v\n", err)
	}
}

func fatalf(format string, args ...interface{}) {
	fmt.Printf(format, args...)
	os.Exit(1)
}

func abspath(path string) string {
	abspath, err := filepath.Abs(path)
	if err != nil {
		return path
	}
	return abspath
}
