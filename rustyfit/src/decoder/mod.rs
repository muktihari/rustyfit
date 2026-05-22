#![warn(missing_docs)]

mod accumulator;
mod bits;

use crate::{
    crc16::Crc16,
    decoder::{accumulator::Accumulator, bits::Bits},
    profile::{
        ProfileType, lookup,
        typedef::{FitBaseType, MesgNum},
    },
    proto::*,
};
use alloc::{vec, vec::Vec};
use core::mem;
use embedded_io::{Read, ReadExactError};

/// Decoder Error
#[derive(Debug)]
pub enum Error<E> {
    /// I/O related error when reading from the Reader.
    Io {
        /// I/O error
        err: E,
    },
    /// Unexpected EOF occurs during process.
    UnexpectedEof,
    /// File Header's size is not 12 or 14, or data_type is not `.FIT`.
    NotFITFile,
    /// Checksum mismatch either in File Header or in record data.
    ChecksumMismatch {
        /// Expected CRC retrieved from FIT File.
        found: u16,
        /// Actual CRC calculated by Decoder
        calculated: u16,
    },
    /// Missing message definition for the current message data.
    MissingMessageDefinition {
        /// Local Message Number
        local_mesg_num: u8,
    },
}

impl<E> core::fmt::Display for Error<E>
where
    E: core::fmt::Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::Io { err } => write!(f, "io error: {}", err),
            Self::UnexpectedEof => write!(f, "unexpected EOF"),
            Self::NotFITFile => write!(f, "not a FIT file"),
            Self::ChecksumMismatch { found, calculated } => {
                write!(
                    f,
                    "checksum mismatch, found {} calculated {}",
                    found, calculated
                )
            }
            Self::MissingMessageDefinition { local_mesg_num } => write!(
                f,
                "missing message definition for local message number {}",
                local_mesg_num
            ),
        }
    }
}

impl<E> From<E> for Error<E> {
    fn from(err: E) -> Self {
        Self::Io { err }
    }
}

impl<E> core::error::Error for Error<E> where E: core::fmt::Debug + core::fmt::Display {}

/// Event is FIT segments encountered by the Decoder.
#[derive(Debug)]
pub enum Event<'a> {
    /// Returned when the Decoder encounter FileHeader.
    FileHeader(&'a FileHeader),
    /// Returned when the Decoder encounter MessageDefinition.
    MessageDefinition(&'a MessageDefinition),
    /// Returned when the Decoder encounter Message.
    Message(&'a Message),
    /// Returned when the Decoder encounter File's CRC.
    Crc(&'a u16),
}

#[derive(Clone, Copy)]
struct Options {
    checksum: bool,
    expand_components: bool,
}

/// Decoder for decoding FIT file.
pub struct Decoder<R> {
    reader: R,
    n: usize,
    cur: u32,
    crc16: Crc16,
    mesg_definitions: [MessageDefinition; 16],
    accumulator: Accumulator,
    timestamp: u32,
    last_time_offset: u8,
    buf_fields: Vec<Field>,
    buf_developer_fields: Vec<DeveloperField>,
    field_descriptions: Vec<LocalFieldDescription>,
    options: Options,
}

impl<R: Read> Decoder<R> {
    /// Create new Decoder for decoding FIT file.
    /// For more options, use `Decoder::builder()` to build the Decoder.
    pub fn new(reader: R) -> Self {
        Builder::new().build(reader)
    }

    /// Decode return a single FIT sequence. If it's a chained FIT file, call this method multiple times.
    /// First call will return either Ok(Some(fit)) or Err(err), never Ok(None).
    /// On next call, it may return Ok(None) to indicate that no more FIT sequence in the file.
    pub fn decode(&mut self) -> Result<Option<FIT>, Error<R::Error>> {
        let file_header = match self.decode_file_header()? {
            Some(file_header) => file_header,
            None => return Ok(None),
        };

        let mut messages = Vec::new();
        self.decode_message_with(file_header.data_size, |event| {
            if let Event::Message(mesg) = event {
                messages.push(mesg.clone())
            }
        })?;

        let crc = self.decode_crc()?;
        self.reset();
        Ok(Some(FIT {
            file_header,
            messages,
            crc,
        }))
    }

    /// Similar to Decode but with a closure for retrieving DecoderEvent for the current FIT sequence.
    pub fn decode_with<F>(&mut self, mut f: F) -> Result<bool, Error<R::Error>>
    where
        F: FnMut(Event),
    {
        let file_header = match self.decode_file_header()? {
            Some(file_header) => file_header,
            None => return Ok(false),
        };
        f(Event::FileHeader(&file_header));
        self.decode_message_with(file_header.data_size, &mut f)?;
        let crc = self.decode_crc()?;
        f(Event::Crc(&crc));
        self.reset();
        Ok(true)
    }

    fn decode_file_header(&mut self) -> Result<Option<FileHeader>, Error<R::Error>> {
        let mut arr = [0u8; 14];
        if let Err(err) = self.reader.read_exact(&mut arr[..1]) {
            match err {
                ReadExactError::UnexpectedEof => {
                    if self.n > 0 {
                        return Ok(None);
                    }
                    return Err(Error::UnexpectedEof);
                }
                ReadExactError::Other(err) => return Err(Error::Io { err }),
            }
        };
        self.n += 1;

        let n = arr[0] as usize;
        if n != 12 && n != 14 {
            return Err(Error::NotFITFile);
        }

        if let Err(err) = self.reader.read_exact(&mut arr[1..n]) {
            return Err(match err {
                ReadExactError::UnexpectedEof => Error::UnexpectedEof,
                ReadExactError::Other(err) => Error::Io { err },
            });
        }
        self.n += n - 1;

        if &arr[8..12] != FileHeader::DATA_TYPE.as_bytes() {
            return Err(Error::NotFITFile);
        }

        let crc = match n {
            14 => u16::from_le_bytes([arr[12], arr[13]]),
            _ => 0,
        };

        if n == 14 && crc != 0 {
            self.crc16.write(&arr[..12]);
            if self.options.checksum && crc != self.crc16.sum16() {
                return Err(Error::ChecksumMismatch {
                    found: crc,
                    calculated: self.crc16.sum16(),
                });
            }
            self.crc16.reset();
        }

        Ok(Some(FileHeader {
            size: n as u8,
            protocol_version: ProtocolVersion(arr[1]),
            profile_version: u16::from_le_bytes([arr[12], arr[13]]),
            data_size: u32::from_le_bytes(arr[4..8].try_into().unwrap()),
            data_type: FileHeader::DATA_TYPE,
            crc,
        }))
    }

    /// Reads the exact number of bytes required to fill buf, increment n read bytes and calculate checksum.
    fn read_exact_inc(&mut self, buf: &mut [u8]) -> Result<(), Error<R::Error>> {
        if let Err(err) = self.reader.read_exact(buf) {
            return Err(match err {
                ReadExactError::UnexpectedEof => Error::UnexpectedEof,
                ReadExactError::Other(err) => Error::Io { err },
            });
        };
        self.n += buf.len();
        self.cur += buf.len() as u32;
        if self.options.checksum {
            self.crc16.write(buf);
        }
        Ok(())
    }

    fn decode_message_with<F>(&mut self, data_size: u32, mut f: F) -> Result<(), Error<R::Error>>
    where
        F: FnMut(Event),
    {
        let mut arr = [0u8; 1];

        while self.cur < data_size {
            self.read_exact_inc(&mut arr)?;

            let header = arr[0];

            if header & Message::HEADER_MASK == Message::DEFINITION_MASK {
                let local_mesg_num = (header & Message::LOCAL_NUM_MASK) as usize;
                let mut mesg_def = mem::take(&mut self.mesg_definitions[local_mesg_num]);
                mesg_def.header = header;

                self.decode_message_definition(&mut mesg_def)?;

                f(Event::MessageDefinition(&mesg_def));

                self.mesg_definitions[local_mesg_num] = mesg_def;
                continue;
            }

            let local_mesg_num = match header & Message::COMPRESSED_HEADER_MASK {
                Message::COMPRESSED_HEADER_MASK => {
                    (header & Message::COMPRESSED_LOCAL_NUM_MASK) >> Message::COMPRESSED_BIT_SHIFT
                }
                _ => header,
            } & Message::LOCAL_NUM_MASK;

            let mesg_def = mem::take(&mut self.mesg_definitions[local_mesg_num as usize]);
            if mesg_def.header == 0 {
                return Err(Error::MissingMessageDefinition { local_mesg_num });
            }

            self.buf_fields.clear();
            self.buf_developer_fields.clear();

            let mut mesg = Message {
                header,
                num: mesg_def.mesg_num,
                fields: mem::take(&mut self.buf_fields),
                developer_fields: mem::take(&mut self.buf_developer_fields),
            };

            self.decode_message_data(&mut mesg, &mesg_def)?;

            self.mesg_definitions[local_mesg_num as usize] = mesg_def;

            f(Event::Message(&mesg));

            self.buf_fields = mesg.fields;
            self.buf_developer_fields = mesg.developer_fields;
        }

        Ok(())
    }

    fn decode_message_definition(
        &mut self,
        mesg_def: &mut MessageDefinition,
    ) -> Result<(), Error<R::Error>> {
        let mut arr = [0u8; 765];
        self.read_exact_inc(&mut arr[..5])?;

        mesg_def.reserved = arr[0];
        mesg_def.arch = arr[1];
        mesg_def.mesg_num = MesgNum(match mesg_def.arch {
            0 => u16::from_le_bytes([arr[2], arr[3]]),
            _ => u16::from_be_bytes([arr[2], arr[3]]),
        });
        mesg_def.field_definitions.clear();
        mesg_def.developer_field_definitions.clear();

        let n = arr[4] as usize * 3;
        self.read_exact_inc(&mut arr[..n])?;

        mesg_def.field_definitions.reserve_exact(255);

        mesg_def
            .field_definitions
            .extend(arr[..n].chunks_exact(3).map(|b| FieldDefinition {
                num: b[0],
                size: b[1],
                base_type: FitBaseType(b[2]),
            }));

        if mesg_def.header & Message::DEV_DATA_MASK == Message::DEV_DATA_MASK {
            self.read_exact_inc(&mut arr[..1])?;

            let n = arr[0] as usize * 3;
            self.read_exact_inc(&mut arr[..n])?;

            mesg_def.developer_field_definitions.reserve_exact(255);

            mesg_def
                .developer_field_definitions
                .extend(arr[..n].chunks_exact(3).map(|b| DeveloperFieldDefinition {
                    num: b[0],
                    size: b[1],
                    developer_data_index: b[2],
                }));
        }

        Ok(())
    }

    fn decode_message_data(
        &mut self,
        mesg: &mut Message,
        mesg_def: &MessageDefinition,
    ) -> Result<(), Error<R::Error>> {
        if mesg.header & Message::COMPRESSED_HEADER_MASK == Message::COMPRESSED_HEADER_MASK {
            let time_offset = mesg.header & Message::COMPRESSED_TIME_MASK;
            self.timestamp = self.timestamp.wrapping_add(
                (time_offset.wrapping_sub(self.last_time_offset) & Message::COMPRESSED_TIME_MASK)
                    as u32,
            );
            self.last_time_offset = time_offset;

            mesg.fields.push(Field {
                num: Field::TIMESTAMP,
                profile_type: ProfileType::DATE_TIME,
                is_expanded: false,
                value: Value::Uint32(self.timestamp),
            });
        }

        self.decode_fields(mesg, &mesg_def)?;

        self.decode_developer_fields(mesg, &mesg_def)?;

        // Developer Data Lookup, currently we allow missing developer_data_id
        if mesg.num == MesgNum::FIELD_DESCRIPTION {
            self.field_descriptions
                .push(LocalFieldDescription::from(&*mesg));
        }

        if !self.options.expand_components {
            return Ok(());
        }

        // Now that all fields has been decoded, we need to expand all components and accumulate the accumulable values.
        for i in 0..mesg.fields.len() {
            let field = &mesg.fields[i];
            if !field.value.is_valid(field.profile_type.base_type()) {
                continue;
            }
            let field_ref = match lookup::field_reference(mesg.num, field.num) {
                Some(field_ref) => field_ref,
                None => continue,
            };
            let components = match mesg.sub_field_substitution(&field_ref) {
                Some(sub_field) => sub_field.components,
                None => field_ref.components,
            };
            if components.is_empty() {
                continue;
            }
            if let Some(bits) = &mut Bits::new(&field.value) {
                self.expand_components(mesg, bits, components);
            };
        }

        Ok(())
    }

    fn decode_fields(
        &mut self,
        mesg: &mut Message,
        mesg_def: &MessageDefinition,
    ) -> Result<(), Error<R::Error>> {
        let mut arr = [0u8; 255];

        for field_def in &mesg_def.field_definitions {
            let mut buf = &mut arr[..field_def.size as usize];
            self.read_exact_inc(buf)?;

            let num = field_def.num;
            let base_type: FitBaseType;
            let profile_type: ProfileType;
            let accumulate: bool;
            let array: bool;

            match lookup::field_reference(mesg_def.mesg_num, num) {
                Some(field_ref) => {
                    base_type = field_ref.base_type;
                    profile_type = field_ref.profile_type;
                    accumulate = field_ref.accumulate;
                    array = field_ref.array;
                }
                None => {
                    base_type = field_def.base_type;
                    profile_type = ProfileType::from(field_def.base_type);
                    accumulate = false;
                    array = match base_type {
                        FitBaseType::STRING => Value::strcount(buf) > 1,
                        _ => {
                            field_def.size > base_type.size()
                                && field_def.size % base_type.size() == 0
                        }
                    }
                }
            };

            if field_def.size < base_type.size() {
                buf = slice_buffer_to_match_type_size(
                    &mut arr,
                    mesg_def.arch,
                    field_def.size as usize,
                    base_type.size() as usize,
                );
            }

            let value = Value::unmarshal(buf, array, base_type, mesg_def.arch);

            if num == Field::TIMESTAMP && base_type == FitBaseType::UINT32 {
                if let Value::Uint32(v) = value {
                    self.timestamp = v;
                    self.last_time_offset = v as u8 & Message::COMPRESSED_TIME_MASK;
                }
            }

            if accumulate {
                self.accumulator.collect(mesg.num, num, &value);
            }

            mesg.fields.push(Field {
                num,
                profile_type,
                is_expanded: false,
                value,
            });
        }

        Ok(())
    }

    fn expand_components(&mut self, mesg: &mut Message, bits: &mut Bits, components: &[Component]) {
        for component in components {
            let mut val = match bits.pull(component.bits) {
                Some(v) => v,
                None => break,
            };

            let field_num = component.field_num;
            if component.accumulate {
                val = self
                    .accumulator
                    .accumulate(mesg.num, field_num, val, component.bits);
            }

            let field_ref = match lookup::field_reference(mesg.num, field_num) {
                Some(v) => v,
                None => continue,
            };

            let scaled_val = val as f64 / component.scale - component.offset;
            val = ((scaled_val + field_ref.offset) * field_ref.scale) as u64;
            let value = convert_u64_to_value(val, field_ref.base_type);

            match mesg.fields.iter_mut().find(|v| v.num == field_num) {
                Some(v) => {
                    if field_ref.array {
                        push_value_to_vec(&mut v.value, &value);
                    } else {
                        v.value = value;
                    }
                }
                None => {
                    mesg.fields.push(Field {
                        num: field_num,
                        profile_type: field_ref.profile_type,
                        is_expanded: true,
                        value: if field_ref.array {
                            let mut vec_value = Value::Invalid;
                            push_value_to_vec(&mut vec_value, &value);
                            vec_value
                        } else {
                            value
                        },
                    });
                }
            };

            let components = match mesg.sub_field_substitution(&field_ref) {
                Some(sub_field) => sub_field.components,
                None => field_ref.components,
            };

            if components.is_empty() {
                continue;
            }

            let value = convert_u64_to_value(val, field_ref.base_type);
            if !value.is_valid(field_ref.base_type) {
                continue;
            }

            if let Some(bits) = &mut Bits::new(&value) {
                self.expand_components(mesg, bits, components);
            };
        }
    }

    fn decode_developer_fields(
        &mut self,
        mesg: &mut Message,
        mesg_def: &MessageDefinition,
    ) -> Result<(), Error<R::Error>> {
        let mut arr = [0u8; 255];

        for dev_field_def in &mesg_def.developer_field_definitions {
            let mut buf = &mut arr[..dev_field_def.size as usize];
            self.read_exact_inc(buf)?;

            let field_desc = match self.field_descriptions.iter().find(|v| {
                v.developer_data_index == dev_field_def.developer_data_index
                    && v.field_definition_number == dev_field_def.num
            }) {
                Some(field_desc) => field_desc,
                None => continue, // Currently we ignore missing field_description
            };

            let base_type = field_desc.fit_base_type_id;
            if dev_field_def.size < base_type.size() {
                buf = slice_buffer_to_match_type_size(
                    &mut arr,
                    mesg_def.arch,
                    dev_field_def.size as usize,
                    base_type.size() as usize,
                );
            }

            let size = dev_field_def.size;
            let array = match base_type {
                FitBaseType::STRING => Value::strcount(buf) > 1,
                _ => size > base_type.size() && size % base_type.size() == 0,
            };

            let value = Value::unmarshal(buf, array, base_type, mesg_def.arch);

            mesg.developer_fields.push(DeveloperField {
                num: dev_field_def.num,
                developer_data_index: dev_field_def.developer_data_index,
                value,
            });
        }

        Ok(())
    }

    fn decode_crc(&mut self) -> Result<u16, Error<R::Error>> {
        let mut arr = [0u8; 2];
        if let Err(err) = self.reader.read_exact(&mut arr) {
            return Err(match err {
                ReadExactError::UnexpectedEof => Error::UnexpectedEof,
                ReadExactError::Other(err) => Error::Io { err },
            });
        };
        self.n += arr.len();

        let crc = u16::from_le_bytes(arr);
        if self.options.checksum && crc != self.crc16.sum16() {
            return Err(Error::ChecksumMismatch {
                found: crc,
                calculated: self.crc16.sum16(),
            });
        }
        Ok(crc)
    }

    fn reset(&mut self) {
        self.cur = 0;
        self.crc16.reset();
        for mesg_def in &mut self.mesg_definitions {
            mesg_def.header = 0;
        }
        self.accumulator.reset();
        self.timestamp = 0;
        self.last_time_offset = 0;
        self.field_descriptions.clear();
    }
}

fn slice_buffer_to_match_type_size(
    arr: &mut [u8; 255],
    arch: u8,
    current_len: usize,
    target_len: usize,
) -> &mut [u8] {
    if arch == 0 {
        arr[current_len..target_len].fill(0);
        &mut arr[..target_len]
    } else {
        arr.copy_within(..current_len, target_len - current_len);
        arr[..target_len - current_len].fill(0);
        &mut arr[..target_len]
    }
}

fn convert_u64_to_value(val: u64, base_type: FitBaseType) -> Value {
    match base_type {
        FitBaseType::SINT8 => Value::Int8(val as i8),
        FitBaseType::ENUM | FitBaseType::BYTE | FitBaseType::UINT8 | FitBaseType::UINT8Z => {
            Value::Uint8(val as u8)
        }
        FitBaseType::SINT16 => Value::Int16(val as i16),
        FitBaseType::UINT16 | FitBaseType::UINT16Z => Value::Uint16(val as u16),
        FitBaseType::SINT32 => Value::Int32(val as i32),
        FitBaseType::UINT32 | FitBaseType::UINT32Z => Value::Uint32(val as u32),
        FitBaseType::FLOAT32 => Value::Float32(val as f32),
        FitBaseType::FLOAT64 => Value::Float64(val as f64),
        FitBaseType::SINT64 => Value::Int64(val as i64),
        FitBaseType::UINT64 | FitBaseType::UINT64Z => Value::Uint64(val),
        _ => Value::Invalid,
    }
}

fn push_value_to_vec(vec_value: &mut Value, value: &Value) {
    match value {
        Value::Uint8(v) => match vec_value {
            Value::VecUint8(vs) => vs.push(*v),
            _ => *vec_value = Value::VecUint8(vec![*v]),
        },
        Value::Int8(v) => match vec_value {
            Value::VecInt8(vs) => vs.push(*v),
            _ => *vec_value = Value::VecInt8(vec![*v]),
        },
        Value::Uint16(v) => match vec_value {
            Value::VecUint16(vs) => vs.push(*v),
            _ => *vec_value = Value::VecUint16(vec![*v]),
        },
        Value::Int16(v) => match vec_value {
            Value::VecInt16(vs) => vs.push(*v),
            _ => *vec_value = Value::VecInt16(vec![*v]),
        },
        Value::Uint32(v) => match vec_value {
            Value::VecUint32(vs) => vs.push(*v),
            _ => *vec_value = Value::VecUint32(vec![*v]),
        },
        Value::Int32(v) => match vec_value {
            Value::VecInt32(vs) => vs.push(*v),
            _ => *vec_value = Value::VecInt32(vec![*v]),
        },
        Value::Float32(v) => match vec_value {
            Value::VecFloat32(vs) => vs.push(*v),
            _ => *vec_value = Value::VecFloat32(vec![*v]),
        },
        Value::Float64(v) => match vec_value {
            Value::VecFloat64(vs) => vs.push(*v),
            _ => *vec_value = Value::VecFloat64(vec![*v]),
        },
        Value::Int64(v) => match vec_value {
            Value::VecInt64(vs) => vs.push(*v),
            _ => *vec_value = Value::VecInt64(vec![*v]),
        },
        Value::Uint64(v) => match vec_value {
            Value::VecUint64(vs) => vs.push(*v),
            _ => *vec_value = Value::VecUint64(vec![*v]),
        },
        _ => {}
    }
}

impl Decoder<()> {
    /// Create new Decoder with options for decoding FIT file.
    pub const fn builder() -> Builder {
        Builder::new()
    }
}

/// Build Decoder with some options.
pub struct Builder {
    options: Options,
}

impl Builder {
    /// Create new DecoderBuilder.
    pub const fn new() -> Self {
        Self {
            options: Options {
                checksum: true,
                expand_components: true,
            },
        }
    }

    /// Toggle for checksum calculation (default: `true`).
    /// If you want to retrieve the data regardless its integrity, set this to `false`.
    pub const fn checksum(mut self, v: bool) -> Self {
        self.options.checksum = v;
        self
    }

    /// Toggle for field's components expansion (default: `true`).
    pub const fn expand_components(mut self, v: bool) -> Self {
        self.options.expand_components = v;
        self
    }

    /// Build Decoder based on given options (if any).
    pub fn build<R: Read>(&self, reader: R) -> Decoder<R> {
        Decoder {
            reader: reader,
            n: 0,
            cur: 0,
            crc16: Crc16::new(),
            mesg_definitions: [const {
                MessageDefinition {
                    header: 0,
                    reserved: 0,
                    arch: 0,
                    mesg_num: MesgNum(0),
                    field_definitions: Vec::new(),
                    developer_field_definitions: Vec::new(),
                }
            }; 16],
            accumulator: Accumulator::new(),
            timestamp: 0,
            last_time_offset: 0,
            buf_fields: Vec::with_capacity(255),
            buf_developer_fields: Vec::with_capacity(255),
            field_descriptions: Vec::new(),
            options: self.options,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{decoder::convert_u64_to_value, profile::typedef::FitBaseType, proto::Value};

    #[test]
    fn test_convert_u64_to_value() {
        let input = 1u64;

        let tt = [
            (FitBaseType::SINT8, Value::Int8(1)),
            (FitBaseType::ENUM, Value::Uint8(1)),
            (FitBaseType::BYTE, Value::Uint8(1)),
            (FitBaseType::UINT8, Value::Uint8(1)),
            (FitBaseType::UINT8Z, Value::Uint8(1)),
            (FitBaseType::SINT16, Value::Int16(1)),
            (FitBaseType::UINT16, Value::Uint16(1)),
            (FitBaseType::UINT16Z, Value::Uint16(1)),
            (FitBaseType::SINT32, Value::Int32(1)),
            (FitBaseType::UINT32, Value::Uint32(1)),
            (FitBaseType::UINT32Z, Value::Uint32(1)),
            (FitBaseType::FLOAT32, Value::Float32(1.0)),
            (FitBaseType::FLOAT64, Value::Float64(1.0)),
            (FitBaseType::SINT64, Value::Int64(1)),
            (FitBaseType::UINT64, Value::Uint64(1)),
            (FitBaseType::UINT64Z, Value::Uint64(1)),
        ];

        for tc in tt {
            let val = convert_u64_to_value(input, tc.0);
            assert_eq!(tc.1, val, "input: {:?}", tc);
        }
    }
}
