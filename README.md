# RustyFIT

![GitHub Workflow Status](https://github.com/muktihari/rustyfit/workflows/CI/badge.svg)
[![docs.rs](https://img.shields.io/badge/docs.rs-rustyfit-gold?logo=docs.rs)](https://docs.rs/rustyfit)
[![Crates.io Version](https://img.shields.io/crates/v/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Crates.io Downloads](https://img.shields.io/crates/d/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Profile Version](https://img.shields.io/badge/profile-v21.205-lightblue.svg?style=flat)](https://github.com/garmin/fit-sdk-tools/releases)

The `#![no_std]` Rust implementation of [The Flexible and Interoperable Data Transfer (FIT) Protocol](https://developer.garmin.com/fit) for decoding and encoding Garmin FIT files, supporting FIT Protocol V2.

This library is a rewrite of [FIT SDK for Go](https://github.com/muktihari/fit) and is designed to run on baremetal Rust.

## Current State

At the moment, I can't make this library heapless without complicating the code. I have no elegant solution for pure baremetal without dynamic memory allocation. That being said, this library is efficient enough to work on resource-constrained environments. I've tested it on a `#![no_std]` `#![no_main]` program to decode a FIT file (statically embedded) and print some of its Session's fields to `stdout` with a custom global allocator having less than 50 KB heap size and it works like a charm.

## Usage

For [#![no_std]](https://docs.rust-embedded.org/book/intro/no-std.html), you need to provide [#[global_allocator]](https://doc.rust-lang.org/std/alloc/index.html#the-global_allocator-attribute) since this library requires allocation.

For [std](https://doc.rust-lang.org/std), you need to wrap `std::io`'s [Read](https://doc.rust-lang.org/std/io/trait.Read.html) or [Write](https://doc.rust-lang.org/std/io/trait.Write.html) with [embedded_io_adapters::std:FromStd](https://docs.rs/embedded-io-adapters/0.7.0/embedded_io_adapters/std/struct.FromStd.html). We will provide examples in `std` for a broad audience and simplicity.

### Decoding

#### Decode

Decoder's `decode` allows us to interact with FIT files directly through their original protocol messages' structure. 

NOTE: First call will return either Ok(Some(fit)) or Err(err), never Ok(None).
On next call, it may return Ok(None) to indicate that no more FIT sequence in the file.

```rust
use embedded_io_adapters::std::FromStd;
use rustyfit::{Decoder, profile::{mesgdef, typedef}};
use std::{error::Error, fs::File, io::BufReader};

fn main() -> Result<(), Box<dyn Error>> {
    let name = "Activity.fit";
    let f = File::open(name)?;
    let br = BufReader::new(f);
    let mut dec = Decoder::new(FromStd::new(br));

    // SAFETY: First decode call is either Ok(Some(fit)) or Err(err), never Ok(None).
    let fit = dec.decode()?.unwrap(); 

    println!("file_header's data_size: {}", fit.file_header.data_size);
    println!("messages count: {}", fit.messages.len());
    for field in &fit.messages[0].fields {
        // first message: file_id
        if field.num == mesgdef::FileId::TYPE {
            println!("file type: {}", typedef::File(field.value.as_u8()));
        }
    }
    
    Ok(())
    
    // # Output:
    // file_header's data_size: 94080
    // messages count: 3611
    // file type: activity
}
```

The `decode` method can be invoked multiple times to decode chained FIT file until it return Ok(None) or Err(err). 
We can decode chained FIT file using `while let`, e.g:

```rust
    while let Some(fit) = dec.decode()? {
        println!("file_header's data_size: {}", fit.file_header.data_size);
        println!("messages count: {}", fit.messages.len());
        for field in &fit.messages[0].fields {
            // first message: file_id
            if field.num == mesgdef::FileId::TYPE {
                println!("file type: {}", typedef::File(field.value.as_u8()));
            }
        }
    }
```

#### Decode with Closure

Decoder's `decode_with` allow us to retrieve event data (FileHeader, MessageDefinition, Message, CRC) as soon as it is being decoded. 
This way, users can have fine-grained control on how to interact with the data.

```rust
use embedded_io_adapters::std::FromStd;
use rustyfit::{Decoder, DecoderEvent, profile::{mesgdef, typedef}};
use std::{error::Error, fs::File, io::BufReader};

fn main() -> Result<(), Box<dyn Error>> {
    let name = "Activity.fit";
    let f = File::open(name)?;
    let br = BufReader::new(f);
    let mut dec = Decoder::new(FromStd::new(br));

    dec.decode_with(|event| match event {
        DecoderEvent::FileHeader(_) => {},
        DecoderEvent::MessageDefinition(_) => {},
        DecoderEvent::Message(mesg) => {
            if mesg.num == typedef::MesgNum::SESSION {
                // Convert mesg into Session struct
                let ses = mesgdef::Session::from(mesg);
                println!(
                    "session:\n start_time: {}\n sport: {}\n num_laps: {}",
                    ses.start_time.0, ses.sport, ses.num_laps
                );
            }
        }
        DecoderEvent::Crc(_) => {}
    })?;
    
    Ok(())

    // # Output
    // session:
    //  start_time: 995749880
    //  sport: stand_up_paddleboarding
    //  num_laps: 1
}
```

The `decode_with` method can be invoked multiple times to decode chained FIT file until it return Ok(false) or Err(err). 

```rust
    while dec.decode_with(|event| {
        if let DecoderEvent::Message(mesg) = event {
            if mesg.num == typedef::MesgNum::SESSION {
                // Convert mesg into Session struct
                let ses = mesgdef::Session::from(mesg);
                println!(
                    "session:\n start_time: {}\n sport: {}\n num_laps: {}",
                    ses.start_time.0, ses.sport, ses.num_laps
                );
            }
        }
    })? {}
```

#### DecoderBuilder

Create `Decoder` instance with options using `Decoder::builder()` or `DecoderBuilder::new()`.

```rust
let mut dec = Decoder::builder()
        .checksum(false)
        .expand_components(false)
        .build(reader);
```

### Encoding

#### Encode

Here is the example of manually encode FIT protocol using this library to give the idea how it works.

```rust
use embedded_io_adapters::std::FromStd;
use rustyfit::{Encoder, profile::{ProfileType, mesgdef, typedef}, proto::{FIT, Field, Message, Value}};
use std::{error::Error, fs::File, io::{BufWriter, Write}};

fn main() -> Result<(), Box<dyn Error>> {
    let fout_name = "output.fit";
    let fout = File::create(fout_name)?;
    let mut bw = BufWriter::new(fout);
    let mut enc = Encoder::new(FromStd::new(&mut bw));

    let mut fit = FIT {
        messages: vec![
            Message {
                num: typedef::MesgNum::FILE_ID,
                fields: vec![
                    Field {
                        num: mesgdef::FileId::MANUFACTURER,
                        profile_type: ProfileType::MANUFACTURER, // or ProfileType::UINT16 is also valid
                        value: Value::Uint16(typedef::Manufacturer::GARMIN.0),
                        is_expanded: false,
                    },
                    Field {
                        num: mesgdef::FileId::PRODUCT,
                        profile_type: ProfileType::UINT16,
                        value: Value::Uint16(typedef::GarminProduct::FENIX8_SOLAR.0),
                        is_expanded: false,
                    },
                    Field {
                        num: mesgdef::FileId::TYPE,
                        profile_type: ProfileType::UINT8,
                        value: Value::Uint8(typedef::File::ACTIVITY.0),
                        is_expanded: false,
                    },
                ],
                ..Default::default()
            },
            Message {
                num: typedef::MesgNum::RECORD,
                fields: vec![
                    Field {
                        num: mesgdef::Record::DISTANCE,
                        profile_type: ProfileType::UINT32,
                        value: Value::Uint32(100 * 100), // 100 m
                        is_expanded: false,
                    },
                    Field {
                        num: mesgdef::Record::HEART_RATE,
                        profile_type: ProfileType::UINT8,
                        value: Value::Uint8(70), // 70 bpm
                        is_expanded: false,
                    },
                    Field {
                        num: mesgdef::Record::SPEED,
                        profile_type: ProfileType::UINT16,
                        value: Value::Uint16(2 * 1000), // 2 m/s
                        is_expanded: false,
                    },
                ],
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    enc.encode(&mut fit)?;
    bw.flush()?;

    Ok(())
}
```

#### Encode using mesgdef module

Alternatively, users can create messages using the mesgdef module for convenience.

```rust
use embedded_io_adapters::std::FromStd;
use rustyfit::{Encoder, profile::{mesgdef, typedef}, proto::{FIT, Message}};
use std::{error::Error, fs::File, io::{BufWriter, Write}};

fn main() -> Result<(), Box<dyn Error>> {
    let fout_name = "output.fit";
    let fout = File::create(fout_name)?;
    let mut bw = BufWriter::new(fout);
    let mut enc = Encoder::new(FromStd::new(&mut bw));

    let mut fit = FIT {
        messages: vec![
            {
                let mut file_id = mesgdef::FileId::new();
                file_id.manufacturer = typedef::Manufacturer::GARMIN;
                file_id.product = typedef::GarminProduct::FENIX8_SOLAR.0;
                file_id.r#type = typedef::File::ACTIVITY;
                Message::from(file_id)
            },
            {
                let mut record = mesgdef::Record::new();
                record.distance = 100 * 100; // 100 m
                record.heart_rate = 70; // 70 bpm
                record.speed = 2 * 1000; // 2 m/s
                Message::from(record)
            },
        ],
        ..Default::default()
    };

    enc.encode(&mut fit)?;
    bw.flush()?;

    Ok(())
}
```

#### EncoderBuilder

Create `Encoder` instance with options using `Encoder::builder()` or `EncoderBuilder::new()`.

```rust
let mut enc = Encoder::builder()
        .endianness(Endianness::BigEndian)
        .protocol_version(ProtocolVersion::V2)
        .header_option(HeaderOption::Compressed(3))
        .build(writer);
```

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
