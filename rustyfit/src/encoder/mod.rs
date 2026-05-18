#![warn(missing_docs)]

mod lru;
mod validator;

use crate::{
    crc16::Crc16,
    encoder::lru::Lru,
    encoder::validator::{MessageValidator, MessageValidatorError},
    profile::{
        PROFILE_VERSION,
        typedef::{DateTime, MesgNum},
    },
    proto::{self, Error as ProtocolError, *},
};
use alloc::vec::Vec;
use embedded_io::{Seek, SeekFrom, Write};

/// Encoder Error
#[derive(Debug)]
pub enum Error<E> {
    /// I/O related error when writing to the Writer.
    Io {
        /// I/O error
        err: E,
        /// Total written
        n: i64,
    },
    /// Empty messages, no data to be encoded.
    EmptyMessages,
    /// Protocol violation related error.
    Protocol {
        /// Message index
        mesg_index: usize,
        /// Message number
        mesg_num: MesgNum,
        /// Protocol error
        err: ProtocolError,
    },
    /// Message validation related error.
    MessageValidation {
        /// Message index
        mesg_index: usize,
        /// Message number
        mesg_num: MesgNum,
        /// Message Validator error
        err: MessageValidatorError,
    },
}

impl<E> core::fmt::Display for Error<E>
where
    E: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::Io { err, n } => write!(f, "io: {}, n: {}", err, n),
            Self::EmptyMessages => write!(f, "messages is empty"),
            Self::Protocol {
                mesg_index,
                mesg_num,
                err,
            } => write!(f, "mesg: index: {}, num: {}: {}", mesg_index, mesg_num, err),

            Self::MessageValidation {
                mesg_index,
                mesg_num,
                err,
            } => write!(f, "mesg: index: {}, num: {}: {}", mesg_index, mesg_num, err),
        }
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
pub struct Encoder<W> {
    writer: W,
    n: i64,
    last_file_header_pos: i64,
    data_size: u32,
    crc16: Crc16,
    lru: Lru,
    buf: Vec<u8>,
    timestamp_reference: u32,
    options: Options,
    message_validator: MessageValidator,
}

impl<W: Write + Seek> Encoder<W> {
    /// Create new Encoder for encoding FIT file.
    /// For more options, use `Encoder::builder()` to build the Encoder.
    pub fn new(writer: W) -> Encoder<W> {
        Builder::new().build(writer)
    }

    /// Encode the given `fit` to the writer.
    pub fn encode(&mut self, fit: &mut FIT) -> Result<(), Error<W::Error>> {
        self.select_protocol_version(&mut fit.file_header);
        self.validate(fit)?;

        self.encode_file_header(&mut fit.file_header)?;
        self.encode_messages(&mut fit.messages)?;

        fit.crc = self.crc16.sum16();
        self.encode_crc()?;

        self.update_file_header(&mut fit.file_header)?;
        self.reset();
        Ok(())
    }

    fn select_protocol_version(&mut self, file_header: &mut FileHeader) {
        if self.options.protocol_version.0 != 0 {
            file_header.protocol_version = self.options.protocol_version;
        } else if file_header.protocol_version.0 == 0 {
            file_header.protocol_version = ProtocolVersion::V1
        }
    }

    fn validate(&mut self, fit: &mut FIT) -> Result<(), Error<W::Error>> {
        if fit.messages.is_empty() {
            return Err(Error::EmptyMessages);
        }
        let protocol_version = fit.file_header.protocol_version;
        for (i, mesg) in fit.messages.iter_mut().enumerate() {
            if let Err(err) = proto::validate_message(mesg, protocol_version) {
                return Err(Error::Protocol {
                    mesg_index: i,
                    mesg_num: mesg.num,
                    err,
                });
            }
        }
        for (i, mesg) in fit.messages.iter_mut().enumerate() {
            if let Err(err) = self.message_validator.validate_message(mesg) {
                return Err(Error::MessageValidation {
                    mesg_index: i,
                    mesg_num: mesg.num,
                    err,
                });
            }
        }
        Ok(())
    }

    fn encode_file_header(&mut self, file_header: &mut FileHeader) -> Result<(), Error<W::Error>> {
        self.last_file_header_pos = self.n;

        if file_header.size != 12 {
            file_header.size = 14;
        }

        if file_header.profile_version == 0 {
            file_header.profile_version = PROFILE_VERSION;
        }

        file_header.data_type = DATA_TYPE;
        file_header.crc = 0; // recalculated

        let buf = &mut self.buf;
        buf.clear();

        file_header.marshal_append(buf);

        if let Err(err) = self.writer.write_all(buf) {
            return Err(Error::Io { err, n: self.n });
        }
        self.n += buf.len() as i64;

        Ok(())
    }

    fn update_file_header(&mut self, file_header: &mut FileHeader) -> Result<(), Error<W::Error>> {
        file_header.data_size = self.data_size;

        let buf = &mut self.buf;
        buf.clear();

        file_header.marshal_append(buf);

        if file_header.size == 14 {
            self.crc16.write(&buf[..12]);
            file_header.crc = self.crc16.sum16();
            buf[12..14].copy_from_slice(&self.crc16.sum16().to_le_bytes());
            self.crc16.reset();
        }

        let size = self.n - self.last_file_header_pos;
        if let Err(err) = self.writer.seek(SeekFrom::Current(-size)) {
            return Err(Error::Io { err, n: self.n });
        }

        if let Err(err) = self.writer.write_all(&buf) {
            return Err(Error::Io { err, n: self.n });
        }

        let n = buf.len() as i64;
        if let Err(err) = self.writer.seek(SeekFrom::Current(size - n)) {
            return Err(Error::Io { err, n: self.n });
        };
        Ok(())
    }

    fn encode_messages(&mut self, messages: &mut [Message]) -> Result<(), Error<W::Error>> {
        for (i, mesg) in messages.iter_mut().enumerate() {
            if let Err(e) = self.encode_message(mesg) {
                match e {
                    Error::Io { err, .. } => return Err(Error::Io { err, n: self.n }),
                    Error::Protocol { err, .. } => {
                        return Err(Error::Protocol {
                            mesg_index: i,
                            mesg_num: mesg.num,
                            err,
                        });
                    }
                    _ => return Err(e),
                }
            }
        }
        Ok(())
    }

    fn encode_message(&mut self, mesg: &mut Message) -> Result<(), Error<W::Error>> {
        mesg.header = MESG_NORMAL_HEADER_MASK;

        if let HeaderOption::Compressed(_) = self.options.header_option {
            self.compress_timestamp_into_header(mesg);
        }

        let buf = &mut self.buf;
        buf.clear();

        marshal_append_message_definition(buf, mesg, self.options.endianness as u8);
        let (local_mesg_num, is_new_mesg_def) = self.lru.put(&buf);

        buf[0] |= local_mesg_num;
        if mesg.header & MESG_COMPRESSED_HEADER_MASK == MESG_COMPRESSED_HEADER_MASK {
            mesg.header |= local_mesg_num << COMPRESSED_BIT_SHIFT;
        } else {
            mesg.header |= local_mesg_num;
        }

        if is_new_mesg_def {
            if let Err(err) = self.writer.write_all(buf) {
                return Err(Error::Io { err, n: self.n });
            }
            self.n += buf.len() as i64;
            self.data_size += buf.len() as u32;
            self.crc16.write(&buf);
        }

        buf.clear();
        if let Err(err) = mesg.marshal_append(buf, self.options.endianness as u8) {
            return Err(Error::Protocol {
                mesg_index: 0,
                mesg_num: MesgNum(0),
                err,
            });
        }

        if let Err(err) = self.writer.write_all(buf) {
            return Err(Error::Io { err, n: self.n });
        }
        self.n += buf.len() as i64;
        self.data_size += buf.len() as u32;
        self.crc16.write(&buf);

        Ok(())
    }

    fn compress_timestamp_into_header(&mut self, mesg: &mut Message) {
        let timestamp = mesg
            .fields
            .iter()
            .find(|field| field.num == FIELD_NUM_TIMESTAMP)
            .map(|field| field.value.as_u32())
            .unwrap_or(u32::MAX);

        if timestamp == u32::MAX || timestamp < DateTime::MIN.0 {
            return;
        }

        if timestamp.wrapping_sub(self.timestamp_reference) as u8 > COMPRESSED_TIME_MASK {
            self.timestamp_reference = timestamp;
            return;
        }

        let time_offset = (timestamp & COMPRESSED_TIME_MASK as u32) as u8;
        mesg.header = MESG_COMPRESSED_HEADER_MASK | time_offset;
        mesg.fields.retain(|field| field.num != FIELD_NUM_TIMESTAMP);
    }

    fn encode_crc(&mut self) -> Result<(), Error<W::Error>> {
        let crc = self.crc16.sum16().to_le_bytes();
        if let Err(err) = self.writer.write_all(&crc) {
            return Err(Error::Io { err, n: self.n });
        }
        self.n += 2;
        self.crc16.reset();
        Ok(())
    }

    fn reset(&mut self) {
        self.timestamp_reference = 0;
        self.data_size = 0;
        self.lru.reset();
        self.message_validator.reset();
    }
}

fn marshal_append_message_definition(buf: &mut Vec<u8>, mesg: &Message, arch: u8) {
    buf.extend_from_slice(&[
        MESG_DEFINITION_MASK, // header
        0,                    // reserved
        arch,                 // architecture
    ]);

    buf.extend_from_slice(&match arch {
        0 => mesg.num.0.to_le_bytes(),
        _ => mesg.num.0.to_be_bytes(),
    });

    buf.push(mesg.fields.len() as u8);
    for field in &mesg.fields {
        buf.push(field.num);
        buf.push(field.value.size() as u8);
        buf.push(field.profile_type.base_type().0)
    }

    if mesg.developer_fields.is_empty() {
        return;
    }

    buf[0] |= DEV_DATA_MASK;
    buf.push(mesg.developer_fields.len() as u8);
    for developer_field in &mesg.developer_fields {
        buf.push(developer_field.num);
        buf.push(developer_field.value.size() as u8);
        buf.push(developer_field.developer_data_index);
    }
}

impl Encoder<()> {
    /// Create new Encoder with options for encoding FIT file.
    pub const fn builder() -> Builder {
        Builder::new()
    }
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
    pub fn build<W: Write + Seek>(&self, writer: W) -> Encoder<W> {
        Encoder {
            writer: writer,
            n: 0,
            last_file_header_pos: 0,
            data_size: 0,
            crc16: Crc16::new(),
            lru: Lru::new(
                match self.options.header_option {
                    HeaderOption::Normal(interleave) => interleave.min(15) as usize,
                    HeaderOption::Compressed(interleave) => interleave.min(3) as usize,
                } + 1,
            ),
            buf: Vec::with_capacity(1536),
            timestamp_reference: 0,
            options: self.options,
            message_validator: MessageValidator::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Encoder,
        profile::{mesgdef, typedef},
        proto::{COMPRESSED_TIME_MASK, MESG_COMPRESSED_HEADER_MASK, Message},
    };
    use alloc::vec;
    use embedded_io::{ErrorKind, ErrorType, Seek, Write};

    struct Sink;

    impl ErrorType for Sink {
        type Error = ErrorKind;
    }

    impl Write for Sink {
        fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    impl Seek for Sink {
        fn seek(&mut self, _: embedded_io::SeekFrom) -> Result<u64, Self::Error> {
            Ok(0)
        }
    }

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
                mesg.header |=
                    MESG_COMPRESSED_HEADER_MASK | (1062594925 & COMPRESSED_TIME_MASK as u32) as u8;
                mesg
            },
        ];

        let mut enc = Encoder::new(Sink {});
        for mesg in mesgs.iter_mut() {
            enc.compress_timestamp_into_header(mesg);
        }

        assert_eq!(mesgs, expected);
    }
}
