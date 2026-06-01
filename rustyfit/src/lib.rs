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
//! ## Usage
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
//!
//! ### Decoding
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
//! #### Streaming Decoding
//!
//! `StreamDecoder` allows us to retrieve event data (`FileHeader`, `MessageDefinition`, `Message`, `CRC`) as soon as it is being decoded.
//! This way, users can have fine-grained control on how to interact with the data efficiently. And since this is lazily evaluated, users can
//! decide when to stop without required to read the whole reader.
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
//!     let mut stream = dec.stream(&mut reader);  // stateful but small since it borrow Decoder.
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
//! to point to next FIT sequence in the reader. If desired, users can also stop the process entirely.
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
//! #### DecoderBuilder
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

#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub use decoder::{
    Builder as DecoderBuilder, Decoder, Error as DecoderError, Event as DecoderEvent,
    Stream as StreamDecoder, StreamingIterator,
};
pub use encoder::{
    Builder as EncoderBuilder, Encoder, Endianness, Error as EncoderError, FieldValidationError,
    HeaderOption, MessageValidationError,
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
