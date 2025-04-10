pub use decoder::{Decoder, DecoderBuilder, DecoderError, DecoderEvent};
pub use encoder::{
    Encoder, EncoderBuilder, EncoderError, EncoderMessageError, Endianness, HeaderOption,
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
