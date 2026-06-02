#![warn(missing_docs)]

mod lru;
mod validator;

use crate::{
    crc16::Crc16,
    encoder::{lru::Lru, validator::MessageValidator},
    profile::{PROFILE_VERSION, typedef::DateTime},
    proto::*,
};
use alloc::vec::Vec;
use embedded_io::{Seek, SeekFrom, Write};
pub use validator::{FieldValidationError, MessageValidationError};

/// Encoder Error
#[derive(Debug)]
pub enum Error<E> {
    /// I/O related error when writing to the Writer.
    Io {
        /// I/O error
        err: E,
    },
    /// Empty messages, no data to be encoded.
    EmptyMessages,
    /// Message validation related error.
    MessageValidation {
        /// Message index
        mesg_index: usize,
        /// Message validation error
        err: MessageValidationError,
    },
}

impl<E> core::fmt::Display for Error<E>
where
    E: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::Io { err } => write!(f, "io error: {}", err),
            Self::EmptyMessages => write!(f, "messages is empty"),
            Self::MessageValidation { mesg_index, err } => {
                write!(f, "message validation: mesg_index {}: {}", mesg_index, err)
            }
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::Io { err }
    }
}

impl<E> core::error::Error for Error<E> where E: core::fmt::Debug + core::fmt::Display {}

/// HeaderOption to pick when encoding FIT file, this will optimize
/// the size of the resulting FIT file.
///
/// Available options: Normal(0-15), Compressed(0-3).
///
/// Default: Normal(0)
#[derive(Clone, Copy)]
pub enum HeaderOption {
    /// Set Normal Header with the number of maximum message definition interleave allowed.
    /// Valid value: 0-15;
    Normal(u8),
    /// Set Compressed Header with the number of maximum message definition interleave allowed.
    /// Valid value: 0-3;
    Compressed(u8),
}

/// Byte order used when encoding FIT file. Default: LittleEndian.
#[repr(u8)]
#[derive(Clone, Copy)]
pub enum Endianness {
    /// LittleEndian byte order.
    LittleEndian = 0,
    /// BigEndian byte order.
    BigEndian = 1,
}

#[derive(Clone, Copy)]
struct Options {
    protocol_version: ProtocolVersion,
    endianness: Endianness,
    header_option: HeaderOption,
}

/// Encoder for encoding FIT file.
pub struct Encoder {
    n: i64,
    data_size: u32,
    crc16: Crc16,
    lru: Lru,
    buf: Vec<u8>,
    timestamp_reference: u32,
    options: Options,
    message_validator: MessageValidator,
}

impl Encoder {
    /// Create new Encoder for encoding FIT file.
    /// For more options, use `Encoder::builder()` to build the Encoder.
    pub const fn new() -> Encoder {
        Builder::new().build()
    }

    /// Create new Encoder with options for encoding FIT file.
    pub const fn builder() -> Builder {
        Builder::new()
    }

    /// Creates a `Stream` from a mutably borrowed `Encoder` for streaming encoding of the given `writer`.
    pub fn stream<'a, W>(&'a mut self, writer: W) -> Stream<'a, W>
    where
        W: Write + Seek,
    {
        self.reset();

        Stream {
            writer,
            encoder: self,
            counter: 0,
        }
    }

    /// Encode the given `fit` to the writer.
    pub fn encode<W>(&mut self, mut writer: W, fit: &mut FIT) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        self.reset();

        self.select_protocol_version(&mut fit.file_header);
        self.validate::<W>(fit)?;

        self.encode_file_header(&mut writer, &mut fit.file_header)?;

        for mesg in &mut fit.messages {
            self.encode_message(&mut writer, mesg)?;
        }

        fit.crc = self.crc16.sum16();
        self.encode_crc(&mut writer)?;

        self.update_file_header(&mut writer, &mut fit.file_header)?;

        Ok(())
    }

    fn select_protocol_version(&mut self, file_header: &mut FileHeader) {
        if self.options.protocol_version.0 != 0 {
            file_header.protocol_version = self.options.protocol_version;
        } else if file_header.protocol_version.0 == 0 {
            file_header.protocol_version = ProtocolVersion::V1
        }
    }

    fn validate<W>(&mut self, fit: &mut FIT) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        if fit.messages.is_empty() {
            return Err(Error::EmptyMessages);
        }

        let protocol_version = fit.file_header.protocol_version;
        for (i, mesg) in fit.messages.iter_mut().enumerate() {
            if let Err(err) = self
                .message_validator
                .validate_message(mesg, protocol_version)
            {
                return Err(Error::MessageValidation { mesg_index: i, err });
            }
        }

        Ok(())
    }

    fn encode_file_header<W>(
        &mut self,
        writer: &mut W,
        file_header: &mut FileHeader,
    ) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        if file_header.size != 12 {
            file_header.size = 14;
        }

        if file_header.profile_version == 0 {
            file_header.profile_version = PROFILE_VERSION;
        }

        file_header.data_type = FileHeader::DATA_TYPE;

        self.buf.clear();
        write_file_header_to_vec(&mut self.buf, file_header);

        writer.write_all(&self.buf)?;
        self.n += self.buf.len() as i64;

        Ok(())
    }

    fn update_file_header<W>(
        &mut self,
        writer: &mut W,
        file_header: &mut FileHeader,
    ) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        file_header.data_size = self.data_size;

        self.buf.clear();
        write_file_header_to_vec(&mut self.buf, file_header);

        if file_header.size == 14 {
            self.crc16.write(&self.buf[..12]);
            file_header.crc = self.crc16.sum16();
            self.buf[12..14].copy_from_slice(&self.crc16.sum16().to_le_bytes());
            self.crc16.reset();
        }

        writer.seek(SeekFrom::Current(-self.n))?;

        writer.write_all(&self.buf)?;

        let n = self.buf.len() as i64;
        writer.seek(SeekFrom::Current(self.n - n))?;
        Ok(())
    }

    fn encode_message<W>(
        &mut self,
        writer: &mut W,
        mesg: &mut Message,
    ) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        mesg.header = Message::NORMAL_HEADER_MASK;

        if let HeaderOption::Compressed(_) = self.options.header_option {
            self.compress_timestamp_into_header(mesg);
        }

        self.buf.clear();

        // We write all at once because we need to store the whole message definition bytes
        // in Lru for redefining `local_mesg_num`. We can use `hash` to optimize this, but
        // we have to deal with collision. Currently, this is efficient enough. At most:
        //  - We only use 1537 bytes of buffer per write.
        //  - Lru can store up to 24592 bytes for 16 interleaves but it's practically impossible.
        write_message_definition_to_vec(&mut self.buf, mesg, self.options.endianness as u8);
        let (local_mesg_num, is_new_mesg_def) = self.lru.put(&self.buf);

        self.buf[0] |= local_mesg_num;
        if mesg.header & Message::COMPRESSED_HEADER_MASK == Message::COMPRESSED_HEADER_MASK {
            mesg.header |= local_mesg_num << Message::COMPRESSED_BIT_SHIFT;
        } else {
            mesg.header |= local_mesg_num;
        }

        if is_new_mesg_def {
            writer.write_all(&self.buf)?;
            self.crc16.write(&self.buf);
            self.n += self.buf.len() as i64;
            self.data_size += self.buf.len() as u32;
        }

        self.write_message_checksum(writer, mesg, self.options.endianness as u8)?;

        Ok(())
    }

    fn compress_timestamp_into_header(&mut self, mesg: &mut Message) {
        let mut timestamp = u32::MAX;
        if let Some(field) = mesg.fields.iter().find(|f| f.num == Field::TIMESTAMP)
            && let Value::Uint32(v) = field.value
        {
            timestamp = v;
        }

        if timestamp == u32::MAX || timestamp < DateTime::MIN.0 {
            return;
        }

        if timestamp.wrapping_sub(self.timestamp_reference) as u8 > Message::COMPRESSED_TIME_MASK {
            self.timestamp_reference = timestamp;
            return;
        }

        let time_offset = (timestamp & Message::COMPRESSED_TIME_MASK as u32) as u8;
        mesg.header = Message::COMPRESSED_HEADER_MASK | time_offset;
        mesg.fields.retain(|field| field.num != Field::TIMESTAMP);
    }

    /// Write message to the writer and calculate the checksum.
    /// This method writes one Value at a time. At most, we only use 255 bytes of buffer.
    fn write_message_checksum<W>(
        &mut self,
        writer: &mut W,
        mesg: &Message,
        arch: u8,
    ) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        writer.write_all(&[mesg.header])?;
        self.crc16.write(&[mesg.header]);

        self.n += 1;
        self.data_size += 1;

        for field in &mesg.fields {
            self.buf.clear();
            write_value_to_vec(&mut self.buf, &field.value, arch);

            writer.write_all(&self.buf)?;
            self.crc16.write(&self.buf);

            self.n += self.buf.len() as i64;
            self.data_size += self.buf.len() as u32;
        }

        for dev_field in &mesg.developer_fields {
            self.buf.clear();
            write_value_to_vec(&mut self.buf, &dev_field.value, arch);

            writer.write_all(&self.buf)?;
            self.crc16.write(&self.buf);

            self.n += self.buf.len() as i64;
            self.data_size += self.buf.len() as u32;
        }

        Ok(())
    }

    fn encode_crc<W>(&mut self, writer: &mut W) -> Result<(), Error<W::Error>>
    where
        W: Write + Seek,
    {
        let crc = self.crc16.sum16();
        writer.write_all(&crc.to_le_bytes())?;
        self.n += 2;
        self.crc16.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.n = 0;
        self.timestamp_reference = 0;
        self.data_size = 0;
        self.message_validator.reset();

        self.buf.clear();
        self.buf.reserve_exact(1537); // Never realloc. [5+1+(3*255)+1+(3*255) = 1537]

        self.lru.reset(
            match self.options.header_option {
                HeaderOption::Normal(interleave) => interleave.min(15) as usize,
                HeaderOption::Compressed(interleave) => interleave.min(3) as usize,
            } + 1,
        );
    }
}

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}

fn write_file_header_to_vec(buf: &mut Vec<u8>, file_header: &FileHeader) {
    buf.push(file_header.size);
    buf.push(file_header.protocol_version.0);
    buf.extend_from_slice(&file_header.profile_version.to_le_bytes());
    buf.extend_from_slice(&file_header.data_size.to_le_bytes());
    buf.extend_from_slice(FileHeader::DATA_TYPE.as_bytes());
    if file_header.size == 14 {
        buf.extend_from_slice(&file_header.crc.to_le_bytes());
    }
}

fn write_message_definition_to_vec(buf: &mut Vec<u8>, mesg: &Message, arch: u8) {
    buf.extend_from_slice(&[
        Message::DEFINITION_MASK, // header
        0,                        // reserved
        arch,                     // architecture
    ]);

    buf.extend_from_slice(&match arch {
        0 => mesg.num.0.to_le_bytes(),
        _ => mesg.num.0.to_be_bytes(),
    });

    buf.push(mesg.fields.len() as u8);
    for field in &mesg.fields {
        buf.push(field.num);
        buf.push(field.value.size() as u8);
        buf.push(field.profile_type.base_type().0);
    }

    if mesg.developer_fields.is_empty() {
        return;
    }

    buf[0] |= Message::DEV_DATA_MASK;
    buf.push(mesg.developer_fields.len() as u8);
    for developer_field in &mesg.developer_fields {
        buf.push(developer_field.num);
        buf.push(developer_field.value.size() as u8);
        buf.push(developer_field.developer_data_index);
    }
}

fn write_value_to_vec(w: &mut Vec<u8>, value: &Value, arch: u8) {
    match value {
        Value::Int8(v) => w.push(*v as u8),
        Value::Uint8(v) => w.push(*v),
        Value::Int16(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Uint16(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Int32(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Uint32(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::String(v) => {
            let b = v.as_bytes();
            w.extend_from_slice(b);
            if b.is_empty() || b[b.len() - 1] != 0 {
                w.push(0);
            }
        }
        Value::Float32(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Float64(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Int64(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::Uint64(v) => w.extend_from_slice(&match arch {
            0 => v.to_le_bytes(),
            _ => v.to_be_bytes(),
        }),
        Value::VecInt8(v) => w.extend(v.iter().map(|&x| x as u8)),
        Value::VecUint8(v) => w.extend(v.iter()),
        Value::VecInt16(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecUint16(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecInt32(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecUint32(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecString(v) => {
            for x in v {
                let b = x.as_bytes();
                w.extend_from_slice(b);
                if b.is_empty() || b[b.len() - 1] != 0 {
                    w.push(0);
                }
            }
        }
        Value::VecFloat32(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecFloat64(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecInt64(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        Value::VecUint64(v) => match arch {
            0 => w.extend(v.iter().flat_map(|x| x.to_le_bytes())),
            _ => w.extend(v.iter().flat_map(|x| x.to_be_bytes())),
        },
        _ => {} // SAFETY: Value validity has been checked by MessageValidator
    };
}

/// Build Encoder with some options.
pub struct Builder {
    options: Options,
}

impl Builder {
    /// Create new DecoderBuilder.
    pub const fn new() -> Builder {
        Self {
            options: Options {
                protocol_version: ProtocolVersion(0),
                endianness: Endianness::LittleEndian,
                header_option: HeaderOption::Normal(0),
            },
        }
    }

    /// Use this protocol version.
    /// Default: `file_header.protocol_version` or `ProtocolVersion::V1`
    pub const fn protocol_version(mut self, protocol_version: ProtocolVersion) -> Self {
        self.options.protocol_version = protocol_version;
        self
    }

    /// Use this endianness.
    /// Default: `Endianness::LittleEndian`
    pub const fn endianness(mut self, endianness: Endianness) -> Self {
        self.options.endianness = endianness;
        self
    }

    /// Use this header option.
    /// Default: `HeaderOption::Normal(0)`
    pub const fn header_option(mut self, header_option: HeaderOption) -> Self {
        self.options.header_option = header_option;
        self
    }

    /// Build Encoder based on given options (if any).
    pub const fn build(&self) -> Encoder {
        Encoder {
            n: 0,
            data_size: 0,
            crc16: Crc16::new(),
            lru: Lru::new(),
            buf: Vec::new(),
            timestamp_reference: 0,
            options: self.options,
            message_validator: MessageValidator::new(),
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

/// A `Stream` from a mutably borrowed `Encoder` for streaming encoding.
pub struct Stream<'a, W> {
    writer: W,
    encoder: &'a mut Encoder,
    counter: usize,
}

impl<'a, W: Write + Seek> Stream<'a, W> {
    /// Write message to the `writer`. When done writing all messages,
    /// call `finish()` to complete this FIT sequence.
    pub fn write_message(&mut self, mesg: &mut Message) -> Result<(), Error<W::Error>> {
        if self.counter == 0 {
            self.writer.write_all(&[0u8; 14])?; // Reserve 14 bytes for FileHeader
            self.encoder.n += 14;
        }

        if let Err(err) = self
            .encoder
            .message_validator
            .validate_message(mesg, self.encoder.options.protocol_version)
        {
            return Err(Error::MessageValidation {
                mesg_index: self.counter,
                err,
            });
        }

        self.encoder.encode_message(&mut self.writer, mesg)?;
        self.counter += 1;

        Ok(())
    }

    /// Finish the current FIT sequence by finalizing the correct FileHeader.
    ///
    /// Calling `write_message()` after this point will write message for the next
    /// FIT sequence on the same `writer`, and `finish()` MUST be called again.
    pub fn finish(&mut self) -> Result<(), Error<W::Error>> {
        if self.counter == 0 {
            return Err(Error::EmptyMessages);
        }

        self.encoder.encode_crc(&mut self.writer)?;

        let mut protocol_version = self.encoder.options.protocol_version;
        if protocol_version == ProtocolVersion(0) {
            protocol_version = ProtocolVersion::V1;
        }

        let file_header = FileHeader {
            size: 14,
            protocol_version,
            profile_version: PROFILE_VERSION,
            data_size: self.encoder.data_size,
            data_type: FileHeader::DATA_TYPE,
            crc: 0, // calculated
        };

        self.encoder.buf.clear();
        write_file_header_to_vec(&mut self.encoder.buf, &file_header);

        self.encoder.crc16.write(&self.encoder.buf[..12]);
        let crc = self.encoder.crc16.sum16();
        self.encoder.buf[12..14].copy_from_slice(&crc.to_le_bytes());
        self.encoder.crc16.reset();

        self.writer.seek(SeekFrom::Current(-self.encoder.n))?;
        self.writer.write_all(&self.encoder.buf)?;
        self.writer.seek(SeekFrom::Current(self.encoder.n - 14))?;

        self.counter = 0;
        self.encoder.reset();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{
        Encoder,
        encoder::write_value_to_vec,
        profile::{mesgdef, typedef},
        proto::{FIT, FileHeader, Message, ProtocolVersion, Value},
    };
    use alloc::{borrow::ToOwned, vec, vec::Vec};
    use embedded_io::{ErrorKind, ErrorType, Seek, Write};
    use embedded_io_adapters::std::FromStd;

    #[test]
    fn compress_timestamp_into_header() {
        let meter: u32 = 100;

        let mut mesgs = vec![
            {
                let mut file_id = mesgdef::FileId::new();
                file_id.manufacturer = typedef::Manufacturer::DEVELOPMENT;
                Message::from(file_id)
            },
            {
                let mut rec = mesgdef::Record::new();
                rec.timestamp = typedef::DateTime(0);
                rec.distance = 100 * meter;
                Message::from(rec)
            },
            {
                let mut rec = mesgdef::Record::new();
                rec.timestamp = typedef::DateTime(1062594924);
                rec.distance = 200 * meter;
                Message::from(rec)
            },
            {
                let mut rec = mesgdef::Record::new();
                rec.timestamp = typedef::DateTime(1062594925);
                rec.distance = 300 * meter;
                Message::from(rec)
            },
        ];

        let expected = vec![
            {
                let mut file_id = mesgdef::FileId::new();
                file_id.manufacturer = typedef::Manufacturer::DEVELOPMENT;
                Message::from(file_id) // Nothing's changed since no timestamp
            },
            {
                let mut rec = mesgdef::Record::new();
                rec.timestamp = typedef::DateTime(0); // Keep since timestamp <= DateTime::MIN
                rec.distance = 100 * meter;
                Message::from(rec)
            },
            {
                let mut rec = mesgdef::Record::new();
                rec.timestamp = typedef::DateTime(1062594924); // Keep since first timestamp 
                rec.distance = 200 * meter;
                Message::from(rec)
            },
            {
                let mut rec = mesgdef::Record::new();
                // Timestamp is compressed into header since roll over < 32
                rec.distance = 300 * meter;
                let mut mesg = Message::from(rec);
                mesg.header |= Message::COMPRESSED_HEADER_MASK
                    | (1062594925 & Message::COMPRESSED_TIME_MASK as u32) as u8;
                mesg
            },
        ];

        let mut enc = Encoder::new();
        for mesg in mesgs.iter_mut() {
            enc.compress_timestamp_into_header(mesg);
        }

        assert_eq!(mesgs, expected);
    }

    #[test]
    fn test_write_value_to_vec() {
        struct Case {
            value: Value,
            expected: Vec<u8>,
            arch: u8,
        }

        let tt = [
            Case {
                value: Value::Int8(1),
                expected: 1i8.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Uint8(2),
                expected: 2u8.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Int16(3),
                expected: 3i16.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Uint16(4),
                expected: 4i16.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Int32(5),
                expected: 5i32.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Uint32(6),
                expected: 6u32.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Int64(7),
                expected: 7i64.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Uint64(8),
                expected: 8u64.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::String("FIT".to_owned()),
                expected: "FIT\x00".as_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::String("".to_owned()),
                expected: "\x00".as_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Float32(9.0),
                expected: 9.0f32.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::Float64(10.0),
                expected: 10.0f64.to_le_bytes().to_vec(),
                arch: 0,
            },
            Case {
                value: Value::VecInt8(vec![1, 1]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(1u8.to_le_bytes());
                    v.extend(1i8.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecUint8(vec![2, 2]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(2u8.to_le_bytes());
                    v.extend(2u8.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecInt16(vec![3, 3]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(3i16.to_le_bytes());
                    v.extend(3i16.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecUint16(vec![4, 4]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(4u16.to_le_bytes());
                    v.extend(4u16.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecInt32(vec![5, 5]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(5i32.to_le_bytes());
                    v.extend(5i32.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecUint32(vec![6, 6]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(6u32.to_le_bytes());
                    v.extend(6u32.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecInt64(vec![7, 7]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(7i64.to_le_bytes());
                    v.extend(7i64.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecUint64(vec![8, 8]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(8u64.to_le_bytes());
                    v.extend(8u64.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecString(vec!["FIT".to_owned(), "SDK".to_owned()]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend("FIT\x00".as_bytes());
                    v.extend("SDK\x00".as_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecString(vec!["".to_owned(), "SDK\x00".to_owned()]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend("\x00".as_bytes());
                    v.extend("SDK\x00".as_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecFloat32(vec![9.0, 9.0]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(9.0f32.to_le_bytes());
                    v.extend(9.0f32.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::VecFloat64(vec![10.0, 10.0]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(10.0f64.to_le_bytes());
                    v.extend(10.0f64.to_le_bytes());
                    v
                },
                arch: 0,
            },
            Case {
                value: Value::Int8(1),
                expected: 1i8.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Uint8(2),
                expected: 2u8.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Int16(3),
                expected: 3i16.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Uint16(4),
                expected: 4i16.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Int32(5),
                expected: 5i32.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Uint32(6),
                expected: 6u32.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Int64(7),
                expected: 7i64.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Uint64(8),
                expected: 8u64.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Float32(9.0),
                expected: 9.0f32.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::Float64(10.0),
                expected: 10.0f64.to_be_bytes().to_vec(),
                arch: 1,
            },
            Case {
                value: Value::VecInt8(vec![1, 1]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(1u8.to_be_bytes());
                    v.extend(1i8.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecUint8(vec![2, 2]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(2u8.to_be_bytes());
                    v.extend(2u8.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecInt16(vec![3, 3]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(3i16.to_be_bytes());
                    v.extend(3i16.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecUint16(vec![4, 4]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(4u16.to_be_bytes());
                    v.extend(4u16.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecInt32(vec![5, 5]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(5i32.to_be_bytes());
                    v.extend(5i32.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecUint32(vec![6, 6]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(6u32.to_be_bytes());
                    v.extend(6u32.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecInt64(vec![7, 7]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(7i64.to_be_bytes());
                    v.extend(7i64.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecUint64(vec![8, 8]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(8u64.to_be_bytes());
                    v.extend(8u64.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecFloat32(vec![9.0, 9.0]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(9.0f32.to_be_bytes());
                    v.extend(9.0f32.to_be_bytes());
                    v
                },
                arch: 1,
            },
            Case {
                value: Value::VecFloat64(vec![10.0, 10.0]),
                expected: {
                    let mut v: Vec<u8> = Vec::new();
                    v.extend(10.0f64.to_be_bytes());
                    v.extend(10.0f64.to_be_bytes());
                    v
                },
                arch: 1,
            },
        ];

        let mut buf: Vec<u8> = Vec::new();
        for tc in tt {
            buf.clear();
            write_value_to_vec(&mut buf, &tc.value, tc.arch);
            assert_eq!(tc.expected, buf, "input: {:?}", tc.value)
        }
    }

    struct WriteSeeker {
        buf: Vec<u8>,
    }

    impl ErrorType for WriteSeeker {
        type Error = ErrorKind;
    }

    impl Write for WriteSeeker {
        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            Ok(self.buf.write(buf).unwrap())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Seek for WriteSeeker {
        fn seek(&mut self, pos: embedded_io::SeekFrom) -> Result<u64, Self::Error> {
            match pos {
                embedded_io::SeekFrom::Current(v) => {
                    unsafe { self.buf.set_len((self.buf.len() as i64 + v) as usize) };
                    return Ok(v.wrapping_abs() as u64);
                }
                _ => panic!("only support SeekFrom::Current"),
            }
        }
    }

    #[test]
    fn test_update_file_header() {
        let mut ws = WriteSeeker {
            buf: vec![14, 16, 213, 82, 0, 0, 0, 0, 46, 70, 73, 84, 0, 0, 64],
        };
        let n = ws.buf.len();

        let mut enc = Encoder::new();
        enc.n = n as i64;
        enc.data_size = 1;

        let mut file_header = FileHeader {
            size: 14,
            protocol_version: ProtocolVersion::V1, // [16]
            profile_version: 21205,                // [212, 82]
            data_size: 0,                          // [1, 0, 0, 0] updated
            data_type: FileHeader::DATA_TYPE,      // [46, 70, 73, 84]
            crc: 0,                                // [83, 147] updated
        };

        enc.update_file_header(&mut ws, &mut file_header).unwrap();
        let pos = ws.buf.len();

        assert_eq!(
            &ws.buf,
            &[14, 16, 213, 82, 1, 0, 0, 0, 46, 70, 73, 84, 83, 147, 64],
            "should write at index 0, and data_size and crc should be updated"
        );

        assert_eq!(pos, n, "should put offset back to its original position");
    }

    #[test]
    fn test_compare_encoder_and_stream_result() {
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

        let mut enc_storage = Vec::<u8>::new();
        let mut enc_writer = FromStd::new(Cursor::new(&mut enc_storage));

        let mut enc = Encoder::new();
        enc.encode(&mut enc_writer, &mut fit).unwrap();

        let mut stream_storage = Vec::<u8>::new();
        let mut stream_writer = FromStd::new(Cursor::new(&mut stream_storage));

        let mut enc2 = Encoder::new();
        let mut stream = enc2.stream(&mut stream_writer);

        for mut mesg in fit.messages.iter_mut() {
            stream.write_message(&mut mesg).unwrap();
        }
        stream.finish().unwrap();

        assert_eq!(
            enc_storage, stream_storage,
            "Encoder and Stream should produce same result"
        );
    }
}
