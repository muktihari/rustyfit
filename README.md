# RustyFIT

![GitHub Workflow Status](https://github.com/muktihari/rustyfit/workflows/CI/badge.svg)
[![docs.rs](https://img.shields.io/badge/docs.rs-rustyfit-gold?logo=docs.rs)](https://docs.rs/rustyfit)
[![Crates.io Version](https://img.shields.io/crates/v/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Crates.io Downloads](https://img.shields.io/crates/d/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Profile Version](https://img.shields.io/badge/profile-v21.212-lightblue.svg?style=flat)](https://github.com/garmin/fit-sdk-tools/releases)

The `#![no_std]` Rust implementation of [The Flexible and Interoperable Data Transfer (FIT) Protocol](https://developer.garmin.com/fit) for decoding and encoding Garmin FIT files, supporting FIT Protocol V2.

This library is a rewrite of the Go implementation, https://github.com/muktihari/fit, and is designed to run on bare-metal Rust, where performance and memory efficiency are carefully considered. By being `#![no_std]`, a wide range of environments is supported, including bare-metal systems, WebAssembly, desktop applications, and servers.

This library now supports [serde](https://crates.io/crates/serde), allowing users to serialize FIT data into other formats, such as JSON, and deserialize it back into FIT by enabling the `serde` feature.

## Usage

Please see **https://docs.rs/rustyfit**.

## Benchmark

For performance comparisons against implementations across multiple programming languages, including the official SDKs, see https://github.com/roznet/fit-benchmarks.

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
