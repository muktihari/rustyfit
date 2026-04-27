# RustyFIT

![GitHub Workflow Status](https://github.com/muktihari/rustyfit/workflows/CI/badge.svg)
[![Crates.io Version](https://img.shields.io/crates/v/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Crates.io Downloads](https://img.shields.io/crates/d/rustyfit.svg)](https://crates.io/crates/rustyfit)
[![Profile Version](https://img.shields.io/badge/profile-v21.201-lightblue.svg?style=flat)](https://developer.garmin.com/fit/download)

Rewrite of [FIT SDK for Go](https://github.com/muktihari/fit) in Rust.

## Current State

This project serves as an exercise for me to learn Rust. I believe the fastest way to learn something new is by reinventing the wheel or rewriting something that already exists. Although this is a learning project, this library generally works, is usable, and quite fast.

Missing features, test completeness, and more robust documentation may be added later through release iteration.

## Usage

### Decoding

#### Decode

Decoder's `decode` allows us to interact with FIT files directly through their original protocol messages' structure. 
First call will return either Ok(Some(fit)) or Err(err), never Ok(None).
On next call, it may return Ok(None) to indicate that no more FIT sequence in the file.

```rust
use rustyfit::{Decoder, profile::{mesgdef, typedef}};
use std::{error::Error, fs::File, io::BufReader};

fn main() -> Result<(), Box<dyn Error>> {
    let name = "Activity.fit";
    let f = File::open(name)?;
    let br = BufReader::new(f);
    let mut dec = Decoder::new(br);

    let fit = dec.decode()?.unwrap(); // First decode call is either Ok(Some(fit)) or Err(err), never Ok(None).

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
use rustyfit::{Decoder, DecoderEvent,profile::{mesgdef, typedef}};
use std::{error::Error, fs::File, io::BufReader};

fn main() -> Result<(), Box<dyn Error>> {
    let name = "Activity.fit";
    let f = File::open(name)?;
    let br = BufReader::new(f);
    let mut dec = Decoder::new(br);

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

Create `Decoder` instance with options using `DecoderBuilder`.

```rust
let mut dec: Decoder = DecoderBuilder::new(br)
        .checksum(false)
        .expand_components(false)
        .build();
```

### Encoding

#### Encode

Here is the example of manually encode FIT protocol using this library to give the idea how it works.

```rust
use std::{error::Error, fs::File, io::{BufWriter, Write}};
use rustyfit::{Encoder, profile::{ProfileType, mesgdef, typedef::{self}}, proto::{FIT, Field, Message, Value}};

fn main() -> Result<(), Box<dyn Error>> {
    let fout_name = "output.fit";
    let fout = File::create(fout_name)?;
    let mut bw = BufWriter::new(fout);
    let mut enc = Encoder::new(&mut bw);

    let mut fit = FIT {
        messages: vec![
            Message {
                num: typedef::MesgNum::FILE_ID,
                fields: vec![
                    Field {
                        num: mesgdef::FileId::MANUFACTURER,
                        profile_type: ProfileType::MANUFACTURER,
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
use std::{error::Error, fs::File, io::{BufWriter, Write}};
use rustyfit::{Encoder, profile::{mesgdef, typedef::{self}}, proto::{FIT, Message}};

fn main() -> Result<(), Box<dyn Error>> {
    let fout_name = "output.fit";
    let fout = File::create(fout_name)?;
    let mut bw = BufWriter::new(fout);
    let mut enc = Encoder::new(&mut bw);

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

Create `Encoder` instance with options using `EncoderBuilder`.

```rust
let mut enc: Encoder = EncoderBuilder::new(&mut bw)
        .endianness(Endianness::BigEndian)
        .protocol_version(ProtocolVersion::V2)
        .header_option(HeaderOption::Compressed(3))
        .build();
```
