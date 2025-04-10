#![warn(missing_docs)]

mod accumulator;
mod bits;

use crate::{
    crc16::Crc16,
    profile::{
        ProfileType, lookup, mesgdef,
        typedef::{FitBaseType, MesgNum},
    },
    proto::*,
};
use accumulator::Accumulator;
use bits::Bits;
use core::fmt;
use std::{
    io::{ErrorKind, Read},
    mem,
};

/// Decoder Error
#[derive(Debug, Clone, Copy)]
pub enum DecoderError {
    /// IO related error when reading from the Reader.
    /// 0: io error kind, 1: read byte position
    Io(ErrorKind, usize),
    /// File Header's size is not 12 or 14, or data_type is not `.FIT`.
    NotFITFile,
    /// Checksum mismatch either in File Header or in record data.
    /// 0: expected crc, 1: got crc
    ChecksumMismatch(u16, u16),
    /// Missing message definition for the current message data.
    /// 0: local message number
    MissingMessageDefinition(u8),
    /// Field definition's size should match exactly, or be a multiple of, the BaseType's size.
    /// 0: expected size, 1: got size, 2: base type
    BaseTypeSizeMismatch(u8, u8, FitBaseType),
}

impl fmt::Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            DecoderError::Io(kind, n) => write!(f, "io error kind {} at byte pos {}", kind, n),
            DecoderError::NotFITFile => write!(f, "not a FIT file"),
            DecoderError::ChecksumMismatch(expected, got) => {
                write!(f, "checksum mismatch, expected {} got {}", expected, got)
            }
            DecoderError::MissingMessageDefinition(local_mesg_num) => write!(
                f,
                "missing message definition for local message number {}",
                local_mesg_num
            ),
            DecoderError::BaseTypeSizeMismatch(expected, size, base_type) => write!(
                f,
                "size {} is less than expected {} for base type {}",
                size, expected, base_type
            ),
        }
    }
}

/// Event produced by decode_fn.
pub enum DecoderEvent<'a> {
    /// Message ref when the Decoder encounter a Message.
    Message(&'a Message),
    /// MessageDefinition ref when the Decoder encounter a MessageDefinition.
    MessageDefinition(&'a MessageDefinition),
}

struct Options {
    checksum: bool,
    expand_components: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            checksum: true,
            expand_components: true,
        }
    }
}

/// Decoder for decoding FIT file.
pub struct Decoder<R: Read> {
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
    field_descriptions: Vec<mesgdef::FieldDescription>,
    options: Options,
}

impl<R: Read> Decoder<R> {
    /// Create new Decoder for decoding FIT file.
    /// For more options, use DecoderBuilder to build the Decoder.
    pub fn new(reader: R) -> Self {
        DecoderBuilder::new(reader).build()
    }

    /// Decode return a single FIT sequence. If it's a chained FIT file, call this method multiple times.
    pub fn decode(&mut self) -> Result<FIT, DecoderError> {
        let file_header = self.decode_file_header()?;

        let mut messages = Vec::new();
        self.decode_message_fn(file_header.data_size, |event| match event {
            DecoderEvent::Message(mesg) => messages.push(mesg.clone()),
            DecoderEvent::MessageDefinition(_) => {}
        })?;

        let crc = self.decode_crc()?;
        self.reset();
        Ok(FIT {
            file_header,
            messages,
            crc,
        })
    }

    /// Similar to Decode but with a closure for retrieving DecoderEvent (Message or MessageDefinition)
    /// for the current FIT sequence.
    pub fn decode_fn<F>(&mut self, f: F) -> Result<(), DecoderError>
    where
        F: FnMut(DecoderEvent),
    {
        let file_header = self.decode_file_header()?;
        self.decode_message_fn(file_header.data_size, f)?;
        _ = self.decode_crc()?;
        self.reset();
        Ok(())
    }

    fn decode_file_header(&mut self) -> Result<FileHeader, DecoderError> {
        let mut arr = [0u8; 14];
        if let Err(err) = self.reader.read_exact(&mut arr[..1]) {
            return Err(DecoderError::Io(err.kind(), self.n));
        };
        self.n += 1;

        let n = arr[0];
        if n != 12 && n != 14 {
            return Err(DecoderError::NotFITFile);
        }

        if let Err(err) = self.reader.read_exact(&mut arr[1..n as usize]) {
            return Err(DecoderError::Io(err.kind(), self.n));
        }
        self.n += n as usize - 1;

        if &arr[8..12] != DATA_TYPE.as_bytes() {
            return Err(DecoderError::NotFITFile);
        }

        let crc = match n {
            14 => u16::from_le_bytes([arr[12], arr[13]]),
            _ => 0,
        };

        if n == 14 && crc != 0 {
            self.crc16.write(&arr[..12]);
            if self.options.checksum && crc != self.crc16.sum16() {
                return Err(DecoderError::ChecksumMismatch(crc, self.crc16.sum16()));
            }
            self.crc16.reset();
        }

        Ok(FileHeader {
            size: n,
            protocol_version: ProtocolVersion(arr[1]),
            profile_version: u16::from_le_bytes([arr[12], arr[13]]),
            data_size: u32::from_le_bytes(arr[4..8].try_into().unwrap()),
            data_type: DATA_TYPE,
            crc,
        })
    }

    /// Reads the exact number of bytes required to fill buf, increment n read bytes and calculate checksum.
    fn read_exact_inc(&mut self, buf: &mut [u8]) -> Result<(), DecoderError> {
        if let Err(err) = self.reader.read_exact(buf) {
            return Err(DecoderError::Io(err.kind(), self.n));
        };
        self.n += buf.len();
        self.cur += buf.len() as u32;
        if self.options.checksum {
            self.crc16.write(buf);
        }
        Ok(())
    }

    fn decode_message_fn<F>(&mut self, data_size: u32, mut f: F) -> Result<(), DecoderError>
    where
        F: FnMut(DecoderEvent),
    {
        let mut arr = [0u8; 1];

        while self.cur < data_size {
            self.read_exact_inc(&mut arr)?;

            let header = arr[0];
            if header & MESG_HEADER_MASK == MESG_DEFINITION_MASK {
                let local_num = (header & LOCAL_MESG_NUM_MASK) as usize;
                let mut mesg_def = mem::replace(&mut self.mesg_definitions[local_num], MESG_DEF);

                self.decode_message_definition(header, &mut mesg_def)?;

                f(DecoderEvent::MessageDefinition(&mesg_def));

                self.mesg_definitions[local_num] = mesg_def;
                continue;
            }

            let mut mesg = self.decode_message_data(header)?;

            f(DecoderEvent::Message(&mesg));

            mem::swap(&mut mesg.fields, &mut self.buf_fields);
            mem::swap(&mut mesg.developer_fields, &mut self.buf_developer_fields);
        }

        Ok(())
    }

    fn decode_message_definition(
        &mut self,
        header: u8,
        mesg_def: &mut MessageDefinition,
    ) -> Result<(), DecoderError> {
        let mut arr = [0u8; 765];

        self.read_exact_inc(&mut arr[..5])?;

        mesg_def.header = header;
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

        let mut buf = &arr[..n];
        while buf.len() >= 3 {
            mesg_def.field_definitions.push(FieldDefinition {
                num: buf[0],
                size: buf[1],
                base_type: FitBaseType(buf[2]),
            });
            buf = &buf[3..];
        }

        if header & DEV_DATA_MASK == DEV_DATA_MASK {
            self.read_exact_inc(&mut arr[..1])?;

            let n = arr[0] as usize * 3;
            self.read_exact_inc(&mut arr[..n])?;

            mesg_def.developer_field_definitions.reserve_exact(255);

            buf = &arr[..n];
            while buf.len() >= 3 {
                mesg_def
                    .developer_field_definitions
                    .push(DeveloperFieldDefinition {
                        num: buf[0],
                        size: buf[1],
                        developer_data_index: buf[2],
                    });
                buf = &buf[3..];
            }
        }

        Ok(())
    }

    fn decode_message_data(&mut self, header: u8) -> Result<Message, DecoderError> {
        let local_num = match header & COMPRESSED_TIME_MASK {
            COMPRESSED_TIME_MASK => {
                (header & COMPRESSED_LOCAL_MESG_NUM_MASK) >> COMPRESSED_BIT_SHIFT
            }
            _ => header,
        } & LOCAL_MESG_NUM_MASK;

        let mesg_def = mem::replace(&mut self.mesg_definitions[local_num as usize], MESG_DEF);
        if mesg_def.field_definitions.is_empty() && mesg_def.developer_field_definitions.is_empty()
        {
            return Err(DecoderError::MissingMessageDefinition(local_num));
        }

        let mut mesg = Message {
            header,
            num: mesg_def.mesg_num,
            fields: mem::take(&mut self.buf_fields),
            developer_fields: mem::take(&mut self.buf_developer_fields),
        };

        mesg.fields.clear();
        mesg.developer_fields.clear();

        if header & MESG_COMPRESSED_HEADER_MASK == MESG_COMPRESSED_HEADER_MASK {
            let time_offset = header & COMPRESSED_TIME_MASK;
            self.timestamp += ((time_offset - self.last_time_offset) & COMPRESSED_TIME_MASK) as u32;
            self.last_time_offset = time_offset;

            mesg.fields.push(Field {
                num: 253,
                base_type: FitBaseType::UINT32,
                profile_type: ProfileType::UINT32,
                is_expanded: false,
                value: Value::Uint32(self.timestamp),
            });
        }

        self.decode_fields(&mut mesg, &mesg_def)?;

        self.decode_developer_fields(&mut mesg, &mesg_def)?;

        self.mesg_definitions[local_num as usize] = mesg_def;

        // Developer Data Lookup, currently we allow missing developer_data_id
        if mesg.num == MesgNum::FIELD_DESCRIPTION {
            self.field_descriptions
                .push(mesgdef::FieldDescription::from(&mesg));
        }

        if !self.options.expand_components {
            return Ok(mesg);
        }

        // Now that all fields has been decoded, we need to expand all components and accumulate the accumulable values.
        for i in 0..mesg.fields.len() {
            let field = &mesg.fields[i];
            if !field.value.is_valid(field.base_type) {
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
                self.expand_components(&mut mesg, bits, components);
            };
        }

        Ok(mesg)
    }

    fn decode_fields(
        &mut self,
        mesg: &mut Message,
        mesg_def: &MessageDefinition,
    ) -> Result<(), DecoderError> {
        let mut arr = [0u8; 255];

        for field_def in &mesg_def.field_definitions {
            let buf = &mut arr[..field_def.size as usize];
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
                    profile_type = ProfileType((base_type.0 & FitBaseType::NUM_MASK) as u16);
                    accumulate = false;
                    array = match base_type {
                        FitBaseType::STRING => strcount(buf) > 1,
                        _ => {
                            field_def.size > base_type.size()
                                && field_def.size % base_type.size() == 0
                        }
                    }
                }
            };

            let value = Value::unmarshal(buf, array, base_type, mesg_def.arch);

            if num == FIELD_NUM_TIMESTAMP && base_type == FitBaseType::UINT32 {
                if let Value::Uint32(v) = value {
                    self.timestamp = v;
                    self.last_time_offset = v as u8 & COMPRESSED_TIME_MASK;
                }
            }

            if accumulate {
                self.accumulator.collect(mesg.num, num, &value);
            }

            mesg.fields.push(Field {
                num,
                base_type,
                profile_type,
                is_expanded: false,
                value,
            });
        }

        Ok(())
    }

    fn expand_components(&mut self, mesg: &mut Message, bits: &mut Bits, components: &[Component]) {
        for component in components {
            let field_ref = match lookup::field_reference(mesg.num, component.field_num) {
                Some(v) => v,
                None => continue,
            };

            let mut field = Field {
                num: component.field_num,
                base_type: field_ref.base_type,
                profile_type: field_ref.profile_type,
                is_expanded: true,
                value: Value::Invalid,
            };

            let mut val = bits.pull(component.bits);
            if val == 0 && components.len() > 1 {
                break;
            }

            if component.accumulate {
                val = self
                    .accumulator
                    .accumulate(mesg.num, field.num, val, component.bits)
            }

            let scaled_val = val as f64 / component.scale - component.offset;
            val = ((scaled_val + field_ref.offset) * field_ref.scale) as u32;
            let value = convert_u32_to_value(val, field_ref.base_type);

            let mut should_append = true;
            let mut field_mut = &mut field;
            for v in &mut mesg.fields {
                if v.num == field_mut.num {
                    field_mut = v;
                    should_append = false;
                    break;
                }
            }

            if field_ref.array {
                push_value_to_vec(&mut field_mut.value, &value);
            } else {
                field_mut.value = value;
            }

            if should_append {
                mesg.fields.push(field);
            }

            let components = match mesg.sub_field_substitution(&field_ref) {
                Some(sub_field) => sub_field.components,
                None => field_ref.components,
            };

            if components.is_empty() {
                continue;
            }

            let value = convert_u32_to_value(val, field_ref.base_type);
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
    ) -> Result<(), DecoderError> {
        let mut arr = [0u8; 255];

        for dev_field_def in &mesg_def.developer_field_definitions {
            let buf = &mut arr[..dev_field_def.size as usize];
            self.read_exact_inc(buf)?;

            let mut field_desc: Option<&mesgdef::FieldDescription> = None;
            for f in &self.field_descriptions {
                if f.developer_data_index != dev_field_def.developer_data_index {
                    continue;
                }
                if f.field_definition_number != dev_field_def.num {
                    continue;
                }
                field_desc = Some(f);
                break;
            }

            let field_desc = match field_desc {
                Some(field_desc) => field_desc,
                None => continue, // Currently we ignore missing field_description
            };

            let base_type = field_desc.fit_base_type_id;
            if dev_field_def.size < base_type.size() {
                return Err(DecoderError::BaseTypeSizeMismatch(
                    base_type.size(),
                    dev_field_def.size,
                    base_type,
                ));
            }

            let size = dev_field_def.size;
            let array = match base_type {
                FitBaseType::STRING => strcount(buf) > 1,
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

    fn decode_crc(&mut self) -> Result<u16, DecoderError> {
        let mut arr = [0u8; 2];
        if let Err(err) = self.reader.read_exact(&mut arr) {
            return Err(DecoderError::Io(err.kind(), self.n));
        };
        self.n += arr.len();

        let crc = u16::from_le_bytes(arr);
        if self.options.checksum && crc != self.crc16.sum16() {
            return Err(DecoderError::ChecksumMismatch(crc, self.crc16.sum16()));
        }
        Ok(crc)
    }

    fn reset(&mut self) {
        self.cur = 0;
        self.crc16.reset();
        for mesg_def in &mut self.mesg_definitions {
            mesg_def.field_definitions.clear();
            mesg_def.developer_field_definitions.clear();
        }
        self.accumulator.reset();
        self.timestamp = 0;
        self.last_time_offset = 0;
        self.field_descriptions.clear();
    }
}

fn convert_u32_to_value(val: u32, base_type: FitBaseType) -> Value {
    match base_type {
        FitBaseType::SINT8 => Value::Int8(val as i8),
        FitBaseType::ENUM | FitBaseType::BYTE | FitBaseType::UINT8 | FitBaseType::UINT8Z => {
            Value::Uint8(val as u8)
        }
        FitBaseType::SINT16 => Value::Int16(val as i16),
        FitBaseType::UINT16 | FitBaseType::UINT16Z => Value::Uint16(val as u16),
        FitBaseType::SINT32 => Value::Int32(val as i32),
        FitBaseType::UINT32 | FitBaseType::UINT32Z => Value::Uint32(val),
        FitBaseType::FLOAT32 => Value::Float32(val as f32),
        FitBaseType::FLOAT64 => Value::Float64(val as f64),
        FitBaseType::SINT64 => Value::Int64(val as i64),
        FitBaseType::UINT64 | FitBaseType::UINT64Z => Value::Uint64(val as u64),
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

/// Build Decoder with some options.
pub struct DecoderBuilder<R: Read> {
    reader: R,
    options: Options,
}

impl<R: Read> DecoderBuilder<R> {
    /// Create new DecoderBuilder.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            options: Options::default(),
        }
    }

    /// Toggle for checksum calculation (default: `true`).
    /// If you want to retrieve the data regardless its integrity, set this to `false`.
    pub fn checksum(mut self, v: bool) -> Self {
        self.options.checksum = v;
        self
    }

    /// Toggle for field's components expansion (default: `true`).
    pub fn expand_components(mut self, v: bool) -> Self {
        self.options.expand_components = v;
        self
    }

    /// Build Decoder based on given options (if any).
    pub fn build(self) -> Decoder<R> {
        Decoder {
            reader: self.reader,
            n: 0,
            cur: 0,
            crc16: Crc16::new(),
            mesg_definitions: [MESG_DEF; 16],
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

const MESG_DEF: MessageDefinition = MessageDefinition {
    header: 0,
    reserved: 0,
    arch: 0,
    mesg_num: MesgNum(0),
    field_definitions: Vec::new(),
    developer_field_definitions: Vec::new(),
};
