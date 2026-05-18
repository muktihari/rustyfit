#![no_std]

extern crate alloc;

pub use decoder::{
    Builder as DecoderBuilder, Decoder, Error as DecoderError, Event as DecoderEvent,
};
pub use encoder::{
    Builder as EncoderBuilder, Encoder, Endianness, Error as EncoderError, HeaderOption,
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
