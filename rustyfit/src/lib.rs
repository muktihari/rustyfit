//! A stateless and static-friendly [`#![no_std]`](https://docs.rust-embedded.org/book/intro/no-std.html)
//! library to decode and encode Garmin FIT files, supporting FIT Protocol V2.
//!
//! [The Flexible and Interoperable Data Transfer (FIT) Protocol](https://developer.garmin.com/fit)
//! is a protocol developed by Garmin for storing and sharing data originating from sports, fitness,
//! and health devices. Activities recorded using devices such as smartwatch and cycling computer
//! are now mostly in a FIT file format (\*.fit).
//!
//! This library is a rewrite of the Go implementation, [https://github.com/muktihari/fit](https://github.com/muktihari/fit),
//! and is designed to run on bare-metal Rust, where performance and memory efficiency are carefully considered.
//! By being `#![no_std]`, a wide range of environments is supported, including bare-metal systems, WebAssembly,
//! desktop applications, and servers.
//!
//! This library now supports [serde](https://crates.io/crates/serde), allowing users to serialize data into
//! other formats, such as JSON, and deserialize it back by enabling the `serde` feature.
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
//! ```sh
//! cargo add rustyfit
//! cargo add embedded-io-adapters --features std # only for std
//! ```
//!
//! We will provide examples in `std` for simplicity and a wider audience, since `#![no_std]` is platform-dependent.
//! For additional examples, including `#![no_std]` use cases, please refer to the
//! [examples](https://github.com/muktihari/rustyfit/tree/master/examples) directory in the repository.
//!
//! ####
//!
//! ## Decoding
//!
//! `Decoder`'s `decode()` method allows us to interact with FIT files directly through their original protocol messages' structure.
//!
//! This method returns a single FIT sequence, If it's a chained FIT file, call this method multiple times until it return Ok(None) or Err(err).
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
//!     let Some(fit) = dec.decode(&mut reader)? else {
//!         return Ok(()); // reader is empty from the start, skip.
//!     };
//!
//!     println!("file_header's data_size: {}", fit.file_header.data_size);
//!     println!("messages count: {}", fit.messages.len());
//!
//!     for mesg in &fit.messages {
//!         match mesg.num {
//!             typedef::MesgNum::FILE_ID => {
//!                 // We can manually iterate over fields
//!                 for field in &mesg.fields {
//!                     if field.num == mesgdef::FileId::TYPE
//!                         && let Value::Uint8(v) = field.value
//!                     {
//!                         println!("file_id:\n file_type: {}", typedef::File(v));
//!                     }
//!                 }
//!             }
//!             typedef::MesgNum::SESSION => {
//!                 // But it's more convenience to convert mesg into mesgdef's struct.
//!                 let ses = mesgdef::Session::from(mesg);
//!                 println!(
//!                     "session:\n start_time: {:?}\n sport: {}\n num_laps: {}",
//!                     ses.start_time.unix_timestamp(),
//!                     ses.sport,
//!                     ses.num_laps
//!                 );
//!             }
//!             _ => {}
//!         }
//!     }
//!    
//!     Ok(())
//!
//!     // # Output:
//!     //
//!     // file_header's data_size: 94080
//!     // messages count: 3611
//!     // file_id:
//!     //  file_type: activity
//!     // session:
//!     //  start_time: Some(1626815480)
//!     //  sport: stand_up_paddleboarding
//!     //  num_laps: 1
//! }
//!
//! ```
//!
//! ## Streaming Decoding
//!
//! `StreamDecoder` allows us to retrieve `DecoderEvent` enum for every `next()` method call. The enum can hold a value of either
//! `FileHeader`, `&MessageDefinition`, `&Message` or `CRC`, without unnecessary allocation. This way, users can have fine-grained
//! control to interact with the data efficiently. This is lazily evaluated, so users can decide when to stop without being
//! required to read the entire reader.
//!
//! The `next()` method may advance through chained FIT sequences. If exactly one sequence is desired, break manually when reaching
//! `DecoderEvent::Crc`.
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
//! These associated functions and methods are `const fn`, so we can use it to declare a static variable
//! as long as we wrap it with a lock, e.g. `Mutex`. This is useful on microcrontrollers where RAM is only hundred KBs.
//!
//! ####
//!
//! ## Encoding
//!
//! To make the encoding process easier to understand, we have generated code containing detailed information to guide
//! users through the encoding process, so users don't have to look back-and-forth into `Profile.xlsx` for specifications.
//!
//! ```
//! // rustyfit::profile::mesgdef
//!
//! # use rustyfit::profile::typedef;
//! pub struct FileId {
//!     pub r#type: typedef::File,
//!     pub manufacturer: typedef::Manufacturer,
//!     pub product: u16,
//!     /* ... */
//! }
//!
//! impl FileId {
//!     /// Value's type: `u8`; FitBaseType::ENUM; ProfileType::File
//!     pub const TYPE: u8 = 0;
//!     /// Value's type: `u16`; FitBaseType::UINT16; ProfileType::Manufacturer
//!     pub const MANUFACTURER: u8 = 1;
//!     /// Value's type: `u16`; FitBaseType::UINT16; ProfileType::Uint16
//!     pub const PRODUCT: u8 = 2;
//!     /* ... */
//! }
//! ```
//!
//! Now, let's encode FIT protocol data using low-level structures to illustrate the underlying mechanics of this library.
//!
//! ```
//! use embedded_io_adapters::std::FromStd;
//! use rustyfit::{Encoder, profile::{mesgdef, typedef}, proto::{FIT, Field, Message, Value}};
//! use std::{error::Error, fs::File, io::{BufWriter, Write}};
//!
//! fn main() -> Result<(), Box<dyn Error>> {
//!     let mut fit = FIT {
//!         messages: vec![
//!             Message {
//!                 num: typedef::MesgNum::FILE_ID,
//!                 fields: vec![
//!                     Field {
//!                         num: mesgdef::FileId::TYPE,
//!                         base_type: typedef::FitBaseType::ENUM,
//!                         value: Value::Uint8(typedef::File::ACTIVITY.0),
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::FileId::MANUFACTURER,
//!                         base_type: typedef::FitBaseType::UINT16,
//!                         value: Value::Uint16(typedef::Manufacturer::GARMIN.0),
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::FileId::PRODUCT,
//!                         base_type: typedef::FitBaseType::UINT16,
//!                         value: Value::Uint16(typedef::GarminProduct::FENIX8_SOLAR.0),
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
//!                         base_type: typedef::FitBaseType::UINT32,
//!                         value: Value::Uint32(100 * 100), // 100 m
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::Record::HEART_RATE,
//!                         base_type: typedef::FitBaseType::UINT8,
//!                         value: Value::Uint8(70), // 70 bpm
//!                         is_expanded: false,
//!                     },
//!                     Field {
//!                         num: mesgdef::Record::SPEED,
//!                         base_type: typedef::FitBaseType::UINT16,
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
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Encode with mesgdef
//!
//! For the most convenient approach, users can encode messages using the [mesgdef](crate::profile::mesgdef) module which
//! can be converted into low-level structures, `Message`. Same output, but more concise code.
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
//!                 file_id.r#type = typedef::File::ACTIVITY;
//!                 file_id.manufacturer = typedef::Manufacturer::GARMIN;
//!                 file_id.product = typedef::GarminProduct::FENIX8_SOLAR.0;
//!                 Message::from(file_id)
//!             },
//!             {
//!                 let mut record = mesgdef::Record::new();
//!                 record.distance = 100 * 100; // 100 m
//!                 record.heart_rate = 70; // 70 bpm
//!                 record.set_speed_scaled(2.0); // 2 m/s (helper methods provided)
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
//! The `finish()` method must be called to finalize the sequence. To make a chained FIT file, repeat the process using the same
//! `stream` instance.
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
//!         file_id.r#type = typedef::File::ACTIVITY;
//!         file_id.manufacturer = typedef::Manufacturer::GARMIN;
//!         file_id.product = typedef::GarminProduct::FENIX8_SOLAR.0;
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
//! These associated functions and methods are `const fn`, so we can use it to declare a static variable
//! as long as we wrap it with a lock, e.g. `Mutex`. This is useful on microcrontrollers where RAM is only hundred KBs.
//!
//! # Features
//!
//! No feature is enabled by default.
//!
//! ## Serde
//!
//! Enable this feature for converting from/into other formats e.g. JSON. Units may be converted, see [issue#77](https://github.com/muktihari/rustyfit/issues/77) for details.
//!
//! ```sh
//! cargo add rustyfit --features serde
//! ```
//!
//! Example:
//!
//! ```
//! # #[cfg(not(feature = "serde"))]
//! # fn main() {}
//! # #[cfg(feature = "serde")]
//! use rustyfit::{profile::{mesgdef, typedef}, proto::{Field, Message, Value}};
//!
//! # #[cfg(feature = "serde")]
//! fn main() {
//!     let mut record = mesgdef::Record::new();
//!     record.timestamp = typedef::DateTime::from_unix_timestamp(1781838455);
//!     record.position_lat = 424480360;
//!     record.position_long = -940295581;
//!     record.heart_rate = 70;
//!     record.distance = 50 * 100;
//!     record.activity_type = typedef::ActivityType::CYCLING; // typedef::ActivityType(2)
//!     record.unknown_fields = [Field {
//!         num: 254,
//!         base_type: typedef::FitBaseType::UINT8,
//!         value: Value::Uint8(10),
//!         is_expanded: false,
//!     }].into();
//!
//!     let s = serde_json::to_string_pretty(&record).unwrap();
//!     println!("# Serialized from mesgdef::Record:\n{}\n", s);
//!
//!     let mesg = Message::from(record);
//!     let s = serde_json::to_string_pretty(&mesg).unwrap();
//!     println!("# Serialized from proto::Message:\n{}\n", s);
//! }
//! ```
//!
//! Result:
//!
//! ```sh
//! # Serialized from mesgdef::Record:
//! {
//!   "timestamp": 1781838455,
//!   "position_lat": 35.579532757401466,
//!   "position_long": -78.81466512568295,
//!   "heart_rate": 70,
//!   "distance": 50.0,
//!   "activity_type": {
//!     "t": "cycling",
//!     "c": 2
//!   },
//!   "unknown_fields": [
//!     {
//!       "num": 254,
//!       "base_type": {
//!         "t": "uint8",
//!         "c": 2,
//!       },
//!       "value": {
//!         "t": "uint8",
//!         "c": 10
//!       },
//!       "is_expanded": false
//!     }
//!   ]
//! }
//!
//! # Serialized from proto::Message
//! {
//!   "num": {
//!     "t": "record",
//!     "c": 20
//!   },
//!   "fields": [
//!     {
//!       "num": 253,
//!       "name": "timestamp",
//!       "base_type": {
//!         "t": "uint32",
//!         "c": 134
//!       },
//!       "profile_type": "date_time",
//!       "value": {
//!         "t": "uint32",
//!         "c": 1150772855
//!       },
//!       "scale": 1.0,
//!       "offset": 0.0,
//!       "units": "s",
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 0,
//!       "name": "position_lat",
//!       "base_type": {
//!         "t": "sint32",
//!         "c": 133
//!       },
//!       "profile_type": "sint32",
//!       "value": {
//!         "t": "int32",
//!         "c": 424480360
//!       },
//!       "scale": 1.0,
//!       "offset": 0.0,
//!       "units": "semicircles",
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 1,
//!       "name": "position_long",
//!       "base_type": {
//!         "t": "sint32",
//!         "c": 133
//!       },
//!       "profile_type": "sint32",
//!       "value": {
//!         "t": "int32",
//!         "c": -940295581
//!       },
//!       "scale": 1.0,
//!       "offset": 0.0,
//!       "units": "semicircles",
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 3,
//!       "name": "heart_rate",
//!       "base_type": {
//!         "t": "uint8",
//!         "c": 2
//!       },
//!       "profile_type": "uint8",
//!       "value": {
//!         "t": "uint8",
//!         "c": 70
//!       },
//!       "scale": 1.0,
//!       "offset": 0.0,
//!       "units": "bpm",
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 5,
//!       "name": "distance",
//!       "base_type": {
//!         "t": "uint32",
//!         "c": 134
//!       },
//!       "profile_type": "uint32",
//!       "value": {
//!         "t": "uint32",
//!         "c": 5000
//!       },
//!       "scale": 100.0,
//!       "offset": 0.0,
//!       "units": "m",
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 42,
//!       "name": "activity_type",
//!       "base_type": {
//!         "t": "enum",
//!         "c": 0
//!       },
//!       "profile_type": "activity_type",
//!       "value": {
//!         "t": "uint8",
//!         "c": 2
//!       },
//!       "scale": 1.0,
//!       "offset": 0.0,
//!       "is_expanded": false
//!     },
//!     {
//!       "num": 254,
//!       "base_type": {
//!         "t": "uint8",
//!         "c": 2
//!       },
//!       "value": {
//!         "t": "uint8",
//!         "c": 10
//!       },
//!       "is_expanded": false
//!     }
//!   ],
//! }
//!
//! ```

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

/// The `profile` module represents FIT Global Profile containing types ([typedef](crate::profile::typedef)) and messages
/// ([mesgdef](crate::profile::mesgdef)) generated from Profile.xlsx. And also a [lookup](crate::profile::lookup) function
/// to find field references defined in Profile.xlsx.
pub mod profile;
/// The `proto` module provides FIT Protocol low level representation.
pub mod proto;
/// Semicircles/Degrees converter.
pub mod semconv;

mod crc16;
mod decoder;
mod encoder;
