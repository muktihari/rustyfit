#![no_std]

extern crate alloc;

pub use decoder::{
    Builder as DecoderBuilder, Decoder, Error as DecoderError, Event as DecoderEvent,
};
pub use encoder::{
    Builder as EncoderBuilder, Encoder, Endianness, Error as EncoderError, HeaderOption,
};

/// FIT Global Profile representation (generated from Profile.xlsx)
pub mod profile;
/// FIT Protocol representation
pub mod proto;
/// Semicircles/Degrees converter.
pub mod semconv;

mod crc16;
mod decoder;
mod encoder;
