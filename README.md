# RustyFIT

![GitHub Workflow Status](https://github.com/muktihari/rustyfit/workflows/CI/badge.svg)
[![docs.rs](https://img.shields.io/badge/docs.rs-rustyfit-gold?logo=docs.rs)](https://docs.rs/rustyfit)
[![Crates.io Version](https://img.shields.io/crates/v/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Crates.io Downloads](https://img.shields.io/crates/d/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Profile Version](https://img.shields.io/badge/profile-v21.205-lightblue.svg?style=flat)](https://github.com/garmin/fit-sdk-tools/releases)

The `#![no_std]` Rust implementation of [The Flexible and Interoperable Data Transfer (FIT) Protocol](https://developer.garmin.com/fit) for decoding and encoding Garmin FIT files, supporting FIT Protocol V2.

This library is a rewrite of [FIT SDK for Go](https://github.com/muktihari/fit) and is designed to run on baremetal Rust.

## Current State

At the moment, I can't make this library heapless without complicating the code. I have no elegant solution for pure baremetal without dynamic memory allocation. That being said, this library is efficient enough to work on resource-constrained environments. I've tested it on [a `#![no_std]` program](https://github.com/muktihari/rustyfit/tree/master/examples/no_std_decode) to decode a FIT file (statically embedded) and print some of its Session's fields to `stdout` with a custom global allocator having less than 50 KB heap size and it works like a charm.

## Usage

For [#![no_std]](https://docs.rust-embedded.org/book/intro/no-std.html), you need to provide [#[global_allocator]](https://doc.rust-lang.org/std/alloc/index.html#the-global_allocator-attribute) since this library requires allocation.

For [std](https://doc.rust-lang.org/std), you need to wrap `std::io`'s [Read](https://doc.rust-lang.org/std/io/trait.Read.html) or [Write](https://doc.rust-lang.org/std/io/trait.Write.html) with [embedded_io_adapters::std:FromStd](https://docs.rs/embedded-io-adapters/0.7.0/embedded_io_adapters/std/struct.FromStd.html).

```sh
cargo add rustyfit 
cargo add embedded-io-adapters --features std # only for std
```

Usage examples can be viewed on **https://docs.rs/rustyfit**

## License

This library is licensed under the [BSD 3-Clause License](./LICENSE-BSD-3-CLAUSE).

The file `fitgen/Profile.xlsx` and all files under `rustyfit/tests/data/from_official_sdk/*` are licensed under the [FIT SDK License].
These files are used for code generation and testing only.

The FIT Protocol and FIT file format are proprietary to Garmin and are licensed under the [FIT Protocol License].
Use of this library may require compliance with the [FIT Protocol License].

This project is not affiliated with or endorsed by Garmin.
It is provided "as is" without warranty. Users are responsible for complying with all applicable licenses and laws.

[FIT Protocol License]: https://www.thisisant.com/developer/ant/licensing/flexible-and-interoperable-data-transfer-fit-protocol-license
[FIT SDK License]: ./LICENSE-FIT-SDK "https://developer.garmin.com/fit/download"
