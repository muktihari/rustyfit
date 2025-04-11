#![warn(missing_docs)]

mod lru;
mod validator;

use crate::{
    crc16::Crc16,
    profile::{
        PROFILE_VERSION,
        typedef::{DateTime, MesgNum},
    },
    proto::*,
};
use lru::Lru;
use std::{
    cell::RefCell,
    fmt,
    io::{self, Seek, SeekFrom, Write},
    rc::Rc,
};
use validator::MessageValidator;

/// Encoder Error
#[derive(Debug, Clone)]
pub enum EncoderError {
    /// IO related error when reading from the Reader.
    /// 0: io error kind, 1: write byte position.
    Io(io::ErrorKind, i64),
    /// Empty messages, no data to be encoded.
    EmptyMessages,
    /// Protocol related error.
    /// 0: protocol error, 1: mesg index, 2: mesg number
    ProtocolValidation(ProtocolError, usize, MesgNum),
    /// 0: validator error, 1: error message.
    /// Message validation related error.
    MessageValidation(validator::MessageValidatorError, usize, MesgNum),
    /// 0: encode message error, 1: mesg index, 2: mesg number
    EncodeMessage(EncoderMessageError, usize, MesgNum),
}

impl fmt::Display for EncoderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            EncoderError::Io(kind, total_written) => {
                write!(f, "io error kind {}, total written {}", kind, total_written)
            }
            EncoderError::EmptyMessages => write!(f, "empty messages"),
            EncoderError::ProtocolValidation(err, i, mesg_num) => {
                write!(f, "{:?}: mesg index {}, mesg num {}", err, i, mesg_num)
            }
            EncoderError::MessageValidation(err, i, mesg_num) => {
                write!(f, "{:?}: mesg index {}, mesg num {}", err, i, mesg_num)
            }
            EncoderError::EncodeMessage(err, i, mesg_num) => {
                write!(f, "{:?}: mesg index {}, mesg num {}", err, i, mesg_num)
            }
        }
    }
}

/// Error related to encoding the message.
#[derive(Debug, Clone, Copy)]
pub enum EncoderMessageError {
    /// IO related error when reading from the Reader.
    /// 0: io error kind, 1: write byte position
    Io(io::ErrorKind, i64),
    /// Error occurs when marshaling a message.
    MarshalMessage(ProtocolError),
}

/// HeaderOption to pick when encoding FIT file, this will optimize
/// the size of the resulting FIT file.
///
/// Available options: Normal(0-15), Compressed(0-3).
///
/// Default: Normal(0)
#[derive(Clone, Copy, PartialEq)]
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

struct Options {
    protocol_version: ProtocolVersion,
    endianness: Endianness,
    header_option: HeaderOption,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            protocol_version: ProtocolVersion(0),
            endianness: Endianness::LittleEndian,
            header_option: HeaderOption::Normal(0),
        }
    }
}

/// Encoder for encoding FIT file.
pub struct Encoder<W: Write + Seek> {
    writer: W,
    n: i64,
    last_file_header_pos: i64,
    data_size: u32,
    crc16: Crc16,
    lru: Lru,
    mesg_def: MessageDefinition,
    buf_vector: Rc<RefCell<Vec<u8>>>,
    timestamp_reference: u32,
    options: Options,
    protocol_validator: ProtocolValidator,
    message_validator: MessageValidator,
}

impl<W: Write + Seek> Encoder<W> {
    /// Create new Encoder for encoding FIT file.
    /// For more options, use EncoderBuilder to build the Encoder.
    pub fn new(writer: W) -> Encoder<W> {
        EncoderBuilder::new(writer).build()
    }

    /// Encode the given `fit` to the writer.
    pub fn encode(&mut self, fit: &mut FIT) -> Result<(), EncoderError> {
        self.select_protocol_version(&mut fit.file_header);
        self.validate_messages(&mut fit.messages)?;

        self.encode_file_header(&mut fit.file_header)?;
        self.encode_messages(&mut fit.messages)?;

        fit.crc = self.crc16.sum16();
        self.encode_crc()?;

        self.update_file_header(&mut fit.file_header)?;
        self.reset();
        Ok(())
    }

    fn select_protocol_version(&mut self, file_header: &mut FileHeader) {
        if self.options.protocol_version != ProtocolVersion(0) {
            file_header.protocol_version = self.options.protocol_version;
        } else if file_header.protocol_version == ProtocolVersion(0) {
            file_header.protocol_version = ProtocolVersion::V1
        }
        self.protocol_validator.protocol_version = file_header.protocol_version;
    }

    fn validate_messages(&mut self, messages: &mut [Message]) -> Result<(), EncoderError> {
        if messages.is_empty() {
            return Err(EncoderError::EmptyMessages);
        }
        for (i, mesg) in messages.iter_mut().enumerate() {
            if let Err(err) = self.protocol_validator.validate_message(mesg) {
                return Err(EncoderError::ProtocolValidation(err, i, mesg.num));
            }
        }
        for (i, mesg) in messages.iter_mut().enumerate() {
            if let Err(err) = self.message_validator.validate_message(mesg) {
                return Err(EncoderError::MessageValidation(err, i, mesg.num));
            }
        }
        Ok(())
    }

    fn encode_file_header(&mut self, file_header: &mut FileHeader) -> Result<(), EncoderError> {
        self.last_file_header_pos = self.n;

        if file_header.size != 12 {
            file_header.size = 14;
        }

        if file_header.profile_version == 0 {
            file_header.profile_version = PROFILE_VERSION;
        }

        file_header.data_type = DATA_TYPE;
        file_header.crc = 0; // recalculated

        let mut buf = self.buf_vector.borrow_mut();
        buf.clear();

        file_header.marshal_append(&mut buf);

        if let Err(err) = self.writer.write_all(&buf) {
            return Err(EncoderError::Io(err.kind(), self.n));
        }
        self.n += buf.len() as i64;

        Ok(())
    }

    fn update_file_header(&mut self, file_header: &mut FileHeader) -> Result<(), EncoderError> {
        file_header.data_size = self.data_size;

        let mut buf_mut = self.buf_vector.borrow_mut();
        buf_mut.clear();

        file_header.marshal_append(&mut buf_mut);

        if file_header.size == 14 {
            self.crc16.write(&buf_mut[..12]);
            file_header.crc = self.crc16.sum16();
            buf_mut[12..14].copy_from_slice(&self.crc16.sum16().to_le_bytes());
            self.crc16.reset();
        }

        let size = self.n - self.last_file_header_pos;
        if let Err(err) = self.writer.seek(SeekFrom::Current(-size)) {
            return Err(EncoderError::Io(err.kind(), self.n));
        }

        if let Err(err) = self.writer.write_all(&buf_mut) {
            return Err(EncoderError::Io(err.kind(), self.last_file_header_pos));
        }

        let n = buf_mut.len() as i64;
        if let Err(err) = self.writer.seek(SeekFrom::Current(size - n)) {
            return Err(EncoderError::Io(err.kind(), self.n));
        };
        Ok(())
    }

    fn encode_messages(&mut self, messages: &mut [Message]) -> Result<(), EncoderError> {
        for (i, mesg) in messages.iter_mut().enumerate() {
            if let Err(err) = self.encode_message(mesg) {
                return Err(EncoderError::EncodeMessage(err, i, mesg.num));
            }
        }
        Ok(())
    }

    fn encode_message(&mut self, mesg: &mut Message) -> Result<(), EncoderMessageError> {
        mesg.header = MESG_NORMAL_HEADER_MASK;

        let mut compressed = false;
        if let HeaderOption::Compressed(_) = self.options.header_option {
            compressed = self.compress_timestamp_into_header(mesg);
        }

        let buf_rc = self.buf_vector.clone();
        let mut buf = buf_rc.borrow_mut();
        buf.clear();

        self.create_message_definition(mesg);
        let mesg_def = &self.mesg_def;

        mesg_def.marshal_append(&mut buf);

        let (local_mesg_num, is_new_mesg_def) = self.lru.put(&buf);

        buf[0] |= local_mesg_num;
        if compressed {
            mesg.header |= local_mesg_num << COMPRESSED_BIT_SHIFT;
        } else {
            mesg.header |= local_mesg_num;
        }

        if is_new_mesg_def {
            if let Err(err) = self.writer.write_all(&buf) {
                return Err(EncoderMessageError::Io(err.kind(), self.n));
            }
            self.n += buf.len() as i64;
            self.data_size += buf.len() as u32;
            self.crc16.write(&buf);
        }

        buf.clear();
        if let Err(err) = mesg.marshal_append(&mut buf, mesg_def.arch) {
            return Err(EncoderMessageError::MarshalMessage(err));
        }

        if let Err(err) = self.writer.write_all(&buf) {
            return Err(EncoderMessageError::Io(err.kind(), self.n));
        }
        self.n += buf.len() as i64;
        self.data_size += buf.len() as u32;
        self.crc16.write(&buf);

        Ok(())
    }

    fn compress_timestamp_into_header(&mut self, mesg: &mut Message) -> bool {
        let mut timestamp = u32::MAX;
        for field in &mesg.fields {
            if field.num == FIELD_NUM_TIMESTAMP {
                if let Value::Uint32(v) = &field.value {
                    timestamp = *v
                }
                break;
            }
        }

        if timestamp == u32::MAX || timestamp < DateTime::MIN.0 {
            return false;
        }

        if (timestamp - self.timestamp_reference) as u8 > COMPRESSED_TIME_MASK {
            self.timestamp_reference = timestamp;
            return false;
        }

        let time_offset = (timestamp & COMPRESSED_TIME_MASK as u32) as u8;
        mesg.header = MESG_COMPRESSED_HEADER_MASK | time_offset;
        for (i, field) in mesg.fields.iter().enumerate() {
            if field.num == FIELD_NUM_TIMESTAMP {
                mesg.fields.remove(i);
                break;
            }
        }
        true
    }

    fn create_message_definition(&mut self, mesg: &Message) {
        let mesg_def = &mut self.mesg_def;

        mesg_def.header = MESG_DEFINITION_MASK;
        mesg_def.reserved = 0;
        mesg_def.arch = self.options.endianness as u8;
        mesg_def.mesg_num = mesg.num;
        mesg_def.field_definitions.clear();
        mesg_def.developer_field_definitions.clear();

        for field in &mesg.fields {
            mesg_def.field_definitions.push(FieldDefinition {
                num: field.num,
                size: field.value.size() as u8,
                base_type: field.profile_type.base_type(),
            });
        }

        if mesg.developer_fields.is_empty() {
            return;
        }

        mesg_def.header |= DEV_DATA_MASK;
        for developer_field in &mesg.developer_fields {
            mesg_def
                .developer_field_definitions
                .push(DeveloperFieldDefinition {
                    num: developer_field.num,
                    size: developer_field.value.size() as u8,
                    developer_data_index: developer_field.developer_data_index,
                });
        }
    }

    fn encode_crc(&mut self) -> Result<(), EncoderError> {
        let crc = self.crc16.sum16().to_le_bytes();
        if let Err(err) = self.writer.write_all(&crc) {
            return Err(EncoderError::Io(err.kind(), self.n));
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

/// Build Encoder with some options.
pub struct EncoderBuilder<W: Write + Seek> {
    writer: W,
    options: Options,
    omit_invalid_value: bool,
}

impl<W: Write + Seek> EncoderBuilder<W> {
    /// Create new DecoderBuilder.
    pub fn new(writer: W) -> EncoderBuilder<W> {
        Self {
            writer,
            options: Options::default(),
            omit_invalid_value: true,
        }
    }

    /// Use this protocol version.
    /// Default: `file_header.protocol_version` or `ProtocolVersion::V1`
    pub fn protocol_version(mut self, protocol_version: ProtocolVersion) -> Self {
        self.options.protocol_version = protocol_version;
        self
    }

    /// Use this endianness.
    /// Default: `Endianness::LittleEndian`
    pub fn endianness(mut self, endianness: Endianness) -> Self {
        self.options.endianness = endianness;
        self
    }

    /// Use this header option.
    /// Default: `HeaderOption::Normal(0)`
    pub fn header_option(mut self, header_option: HeaderOption) -> Self {
        self.options.header_option = header_option;
        self
    }

    /// By default, the encoder will omit invalid values to create a more compact file.
    /// For example, an u16 having value 0xFFFF will be omitted.
    ///
    /// Set this to `false` if you want to encode the value as it is.
    pub fn omit_invalid_value(mut self, flag: bool) -> Self {
        self.omit_invalid_value = flag;
        self
    }

    /// Build Encoder based on given options (if any).
    pub fn build(self) -> Encoder<W> {
        Encoder {
            writer: self.writer,
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
            mesg_def: MessageDefinition {
                field_definitions: Vec::with_capacity(255),
                developer_field_definitions: Vec::with_capacity(255),
                ..Default::default()
            },
            buf_vector: Rc::new(RefCell::new(Vec::with_capacity(1536))),
            timestamp_reference: 0,
            options: self.options,
            protocol_validator: ProtocolValidator::default(),
            message_validator: MessageValidator::new(self.omit_invalid_value),
        }
    }
}
