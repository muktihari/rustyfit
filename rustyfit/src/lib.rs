//! A stateless and static-friendly [`#![no_std]`](https://docs.rust-embedded.org/book/intro/no-std.html)
//! library to decode and encode Garmin FIT files, supporting FIT Protocol V2.
//!
//! [The Flexible and Interoperable Data Transfer (FIT) Protocol](https://developer.garmin.com/fit)
//! is a protocol developed by Garmin for storing and sharing data originating from sports, fitness,
//! and health devices. Activities recorded using devices such as smartwatch and cycling computer
//! are now mostly in a FIT file format (\*.fit).
//!
//! This library is a rewrite of [FIT SDK for Go](https://github.com/muktihari/fit) and is designed to run on
//! baremetal Rust, where performance and memory efficiency is carefully considered.
//!
//! # Usage
//!
//! For [`#![no_std]`](https://docs.rust-embedded.org/book/intro/no-std.html), you need to provide
//! [`#[global_allocator]`](https://doc.rust-lang.org/std/alloc/index.html#the-global_allocator-attribute)
//! since this library requires allocation.
//!
//! For [`std`](https://doc.rust-lang.org/std), you need to wrap `std::io`'s
//! [Read](https://doc.rust-lang.org/std/io/trait.Read.html) or
//! [Write](https://doc.rust-lang.org/std/io/trait.Write.html) with
//! [embedded_io_adapters::std:FromStd](https://docs.rs/embedded-io-adapters/0.7.0/embedded_io_adapters/std/struct.FromStd.html).
//!
//! We will provide examples in `std` for simplicity and a wider audience, since `#![no_std]` is platform-dependent.
//! For additional examples, including `#![no_std]` use cases, please refer to the
//! [examples](https://github.com/muktihari/rustyfit/tree/master/examples) directory in the repository.
//!
//! ####
//!
//! ## Decoding
//!
//! `Decoder`'s `decode` method allows us to interact with FIT files directly through their original protocol messages' structure.
//! This method can be invoked multiple times to decode chained FIT file until it return Ok(None) or Err(err).
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Decoder, profile::{mesgdef, typedef}, proto::Value};
//! use std::{error::Error, fs::File, io::BufReader};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let name = "tests/data/from_official_sdk/Activity.fit";
//!     let f = File::open(name)?;
//!     let br = BufReader::new(f);
//!     let mut reader = FromStd::new(br);
//!
//!     let mut dec = Decoder::new();
//!
//!     let fit = match dec.decode(&mut reader)? {
//!         Some(fit) => fit,
//!         None => {
//!             // First decode call to reader should be `Ok` or `Err`.
//!             // Except, reader is already empty to begin with.
//!             return Err(Box::from("empty reader"));
//!         }
//!     };
//!
//!     println!("file_header's data_size: {}", fit.file_header.data_size);
//!     println!("messages count: {}", fit.messages.len());
//!     for field in &fit.messages[0].fields {
//!         // first message: file_id
//!         if field.num == mesgdef::FileId::TYPE
//!             && let Value::Uint8(v) = field.value
//!         {
//!             println!("file type: {}", typedef::File(v));
//!         }
//!     }
//!
//!     Ok(())
//!
//!     // # Output:
//!     // file_header's data_size: 94080
//!     // messages count: 3611
//!     // file type: activity
//! }
//!
//! ```
//!
//! ## Streaming Decoding
//!
//! `StreamDecoder` allows us to retrieve an enum, `DecoderEvent`, as soon as it is being decoded. The enum can hold a reference
//! to either a `FileHeader`, `MessageDefinition`, `Message` or `CRC`. This way, users can have fine-grained control on how to interact
//! with the data. And since this is lazily evaluated, users can decide when to stop without being required to read the entire reader.
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Decoder, DecoderEvent, StreamingIterator, profile::{mesgdef, typedef}};
//! use std::{error::Error, fs::File, io::BufReader};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let name = "tests/data/from_official_sdk/Activity.fit";
//!     let f = File::open(name)?;
//!     let br = BufReader::new(f);
//!     let mut reader = FromStd::new(br);
//!
//!     let mut dec = Decoder::new();              // stateless and static-friendly
//!     let mut stream = dec.stream(&mut reader);  // stateful but small since it borrows Decoder.
//!
//!     while let Some(event) = stream.next() {
//!         match event? {
//!             DecoderEvent::FileHeader(_) => {},
//!             DecoderEvent::MessageDefinition(_) => {},
//!             DecoderEvent::Message(mesg) => {
//!                 if mesg.num == typedef::MesgNum::SESSION {
//!                     // Convert mesg into Session struct
//!                     let ses = mesgdef::Session::from(mesg);
//!                     println!(
//!                         "session:\n start_time: {}\n sport: {}\n num_laps: {}",
//!                         ses.start_time.0, ses.sport, ses.num_laps
//!                     );
//!                 }
//!             }
//!             DecoderEvent::Crc(_) => {}
//!         }
//!     }
//!     
//!     Ok(())
//!
//!     // # Output
//!     // session:
//!     //  start_time: 995749880
//!     //  sport: stand_up_paddleboarding
//!     //  num_laps: 1
//! }
//! ```
//!
//! Users can also use `discard()` to discard this current FIT sequence and direct the `StreamDecoder`
//! to point to next FIT sequence in the reader in the case of chained FIT File.
//!
//! ```
//! # use std::{error::Error, fs::File, io::BufReader};
//! # use embedded_io_adapters::std::FromStd;
//! # use rustyfit::{Decoder, DecoderEvent, StreamingIterator, profile::{mesgdef, typedef}};
//! # fn main() -> Result<(), Box<dyn Error>> {
//!     # let name = "tests/data/from_official_sdk/Activity.fit";
//!     # let f = File::open(name)?;
//!     # let br = BufReader::new(f);
//!     # let mut reader = FromStd::new(br);
//!     # let mut dec = Decoder::new();
//!     # let mut stream = dec.stream(&mut reader);
//!     while let Some(event) = stream.next() {
//!         if let DecoderEvent::Message(mesg) = event? {
//!             if mesg.num == typedef::MesgNum::FILE_ID {
//!                 // Let's say we just want to decode Activity file,
//!                 let file_id = mesgdef::FileId::from(mesg);
//!                 if file_id.r#type != typedef::File::ACTIVITY {
//!                     stream.discard()?; // discard this sequence
//!                     continue;
//!                 }
//!             }
//!             // It's an Activity File!
//!         }
//!     }
//!     # Ok(())
//! # }
//! ```
//!
//! ## DecoderBuilder
//!
//! Create `Decoder` instance with options using `Decoder::builder()` or `DecoderBuilder::new()`.
//!
//! ```
//! # use rustyfit::Decoder;
//! let mut dec = Decoder::builder()
//!         .checksum(false)
//!         .expand_components(false)
//!         .build();
//! ```
//!
//! These associated functions and method are `const fn`, so we can use it to declare a static variable
//! as long as we wrap it with a lock, e.g. `Mutex`. This is useful on microcrontrollers where RAM is only hundred KBs.
//!
//! ####
//!
//! ## Encoding
//!
//! Here is the example of manually encode FIT protocol using this library to give the idea how it works.
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Encoder, profile::{ProfileType, mesgdef, typedef}, proto::{FIT, Field, Message, Value}};
//! use std::{error::Error, fs::File, io::{BufWriter, Write}};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut fit = FIT {
//!         messages: vec![
//!             Message {
//!                 num: typedef::MesgNum::FILE_ID,
//!                 fields: vec![
//!                     Field {
//!                         num: mesgdef::FileId::MANUFACTURER,
//!                         profile_type: ProfileType::MANUFACTURER, // or ProfileType::UINT16 is also valid
//!                         value: Value::Uint16(typedef::Manufacturer::GARMIN.0),
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::FileId::PRODUCT,
//!                         profile_type: ProfileType::UINT16,
//!                         value: Value::Uint16(typedef::GarminProduct::FENIX8_SOLAR.0),
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::FileId::TYPE,
//!                         profile_type: ProfileType::UINT8,
//!                         value: Value::Uint8(typedef::File::ACTIVITY.0),
//!                         is_expanded: false,
//!                     },
//!                 ],
//!                 ..Default::default()
//!             },
//!             Message {
//!                 num: typedef::MesgNum::RECORD,
//!                 fields: vec![
//!                     Field {
//!                         num: mesgdef::Record::DISTANCE,
//!                         profile_type: ProfileType::UINT32,
//!                         value: Value::Uint32(100 * 100), // 100 m
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::Record::HEART_RATE,
//!                         profile_type: ProfileType::UINT8,
//!                         value: Value::Uint8(70), // 70 bpm
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::Record::SPEED,
//!                         profile_type: ProfileType::UINT16,
//!                         value: Value::Uint16(2 * 1000), // 2 m/s
//!                         is_expanded: false,
//!                     },
//!                 ],
//!                 ..Default::default()
//!             },
//!         ],
//!         ..Default::default()
//!     };
//!
//!     let fout_name = "output.fit";
//!     let fout = File::create(fout_name)?;
//!     let mut bw = BufWriter::new(fout);
//!     let mut writer = FromStd::new(&mut bw);
//!
//!     let mut enc = Encoder::new();
//!     enc.encode(&mut writer, &mut fit)?;
//!     bw.flush()?;
//!     # let _ = std::fs::remove_file(fout_name);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Encode using mesgdef module
//!
//! Alternatively, users can create messages using the mesgdef module for convenience.
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Encoder, profile::{mesgdef, typedef}, proto::{FIT, Message}};
//! use std::{error::Error, fs::File, io::{BufWriter, Write}};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut fit = FIT {
//!         messages: vec![
//!             {
//!                 let mut file_id = mesgdef::FileId::new();
//!                 file_id.manufacturer = typedef::Manufacturer::GARMIN;
//!                 file_id.product = typedef::GarminProduct::FENIX8_SOLAR.0;
//!                 file_id.r#type = typedef::File::ACTIVITY;
//!                 Message::from(file_id)
//!             },
//!             {
//!                 let mut record = mesgdef::Record::new();
//!                 record.distance = 100 * 100; // 100 m
//!                 record.heart_rate = 70; // 70 bpm
//!                 record.speed = 2 * 1000; // 2 m/s
//!                 Message::from(record)
//!             },
//!         ],
//!         ..Default::default()
//!     };
//!     
//!     let fout_name = "output.fit";
//!     let fout = File::create(fout_name)?;
//!     let mut bw = BufWriter::new(fout);
//!     let mut writer = FromStd::new(&mut bw);
//!
//!     let mut enc = Encoder::new();
//!     enc.encode(&mut writer, &mut fit)?;
//!     bw.flush()?;
//!     # let _ = std::fs::remove_file(fout_name);
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Streaming Encoding
//!
//! `StreamEncoder` allows us to encode in streaming fashion. Write each message directly without retain them first in the memory.
//! This is useful when the device is the one who produce the data such as smartwatch, cycling computer or other health devices.
//!
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Encoder, profile::{mesgdef, typedef}, proto::Message};
//! use std::{error::Error, fs::File, io::{BufWriter, Write}};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let fout_name = "output.fit";
//!     let fout = File::create(fout_name)?;
//!     let mut bw = BufWriter::new(fout);
//!     let mut writer = FromStd::new(&mut bw);
//!
//!     let mut enc = Encoder::new();             // stateless and static-friendly
//!     let mut stream = enc.stream(&mut writer); // stateful but small since it borrows Encoder.
//!
//!     stream.write_message(&mut {
//!         let mut file_id = mesgdef::FileId::new();
//!         file_id.manufacturer = typedef::Manufacturer::GARMIN;
//!         file_id.product = typedef::GarminProduct::FENIX8_SOLAR.0;
//!         file_id.r#type = typedef::File::ACTIVITY;
//!         Message::from(file_id)
//!     })?;
//!
//!     stream.write_message(&mut {
//!         let mut record = mesgdef::Record::new();
//!         record.distance = 100 * 100; // 100 m
//!         record.heart_rate = 70; // 70 bpm
//!         record.speed = 2 * 1000; // 2 m/s
//!         Message::from(record)
//!     })?;
//!
//!     stream.finish()?;
//!     bw.flush()?;
//!     # let _ = std::fs::remove_file(fout_name);
//!
//!     Ok(())
//! }
//!
//! ```
//!
//! ## EncoderBuilder
//!
//! Create `Encoder` instance with options using `Encoder::builder()` or `EncoderBuilder::new()`.
//!
//! ```
//! # use rustyfit::{Encoder, HeaderOption, Endianness};
//! # use rustyfit::proto::ProtocolVersion;
//! let mut enc = Encoder::builder()
//!         .endianness(Endianness::BigEndian)
//!         .protocol_version(ProtocolVersion::V2)
//!         .header_option(HeaderOption::Compressed(3))
//!         .build();
//! ```
//!
//! These associated functions and method are `const fn`, so we can use it to declare a static variable
//! as long as we wrap it with a lock, e.g. `Mutex`. This is useful on microcrontrollers where RAM is only hundred KBs.

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use decoder::{
    Builder as DecoderBuilder, Decoder, Error as DecoderError, Event as DecoderEvent,
    Stream as StreamDecoder, StreamingIterator,
};
pub use encoder::{
    Builder as EncoderBuilder, Encoder, Endianness, Error as EncoderError, FieldValidationError,
    HeaderOption, MessageValidationError, Stream as StreamEncoder,
};

/// The `profile` module represents FIT Global Profile containing types and messages generated from Profile.xlsx.
pub mod profile;
/// The `proto` module provides FIT Protocol low level representation.
pub mod proto;
/// Semicircles/Degrees converter.
pub mod semconv;

mod crc16;
mod decoder;
mod encoder;
