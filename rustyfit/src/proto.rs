#![warn(missing_docs)]

use core::fmt;

use crate::profile::{
    ProfileType,
    typedef::{FitBaseType, MesgNum},
};

/// Mask for determining if the message type is a message definition.
pub(crate) const MESG_DEFINITION_MASK: u8 = 0b01000000;

/// Mask for determining if the message type is a normal message data .
pub(crate) const MESG_NORMAL_HEADER_MASK: u8 = 0b00000000;

/// Mask for determining if the message type is a compressed timestamp message data.
pub(crate) const MESG_COMPRESSED_HEADER_MASK: u8 = 0b10000000;

/// Mask for determining if the message's header is a message definition or is it
/// actually a message data with compressed timestamp and multiple local message type.
///
/// For compressed timestamp, message data's header bit 5-6 is the local message type.
/// Bit 6 overlap with MESG_DEFINITION_MASK, it's a message definition only if Bit 7 is zero.
pub(crate) const MESG_HEADER_MASK: u8 = MESG_COMPRESSED_HEADER_MASK | MESG_DEFINITION_MASK;

/// Mask for mapping normal message data to the message definition.
pub(crate) const LOCAL_MESG_NUM_MASK: u8 = 0b00001111;

/// Mask for mapping compressed timestamp message data to the message definition. Used with CompressedBitShift.
pub(crate) const COMPRESSED_LOCAL_MESG_NUM_MASK: u8 = 0b01100000;

/// Mask for measuring time offset value from header. Compressed timestamp is using 5 least significant bits (lsb) of header
pub(crate) const COMPRESSED_TIME_MASK: u8 = 0b00011111;

/// Mask for determining if a message contains developer fields.
pub(crate) const DEV_DATA_MASK: u8 = 0b00100000;

/// Used for right-shifting the 5 least significant bits (lsb) of compressed time.
pub(crate) const COMPRESSED_BIT_SHIFT: u8 = 5;

/// Field's number for timestamp across all defined messages in the profile.
/// Exception: Course Point and Set message.
pub(crate) const FIELD_NUM_TIMESTAMP: u8 = 253;

/// FileHeader's data_type.
pub(crate) static DATA_TYPE: &str = ".FIT";

/// Defined errors returned from proto module.
#[derive(Debug, Clone, Copy)]
pub enum Error {
    /// Protocol Validator's error when Protocol Version 1.0 constains developer data.
    ProtocolViolationDeveloperData,
    /// Protocol Validator's error when Protocol Version 1.0 has base_type number more than byte number.
    ProtocolViolationUnsupportedBaseType(FitBaseType),
    /// Marshal error when Value is invalid.
    InvalidValue,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Error::ProtocolViolationDeveloperData => {
                write!(f, "protocol version 1.0 do not support developer data")
            }
            Error::ProtocolViolationUnsupportedBaseType(base_type) => {
                write!(f, "protocol version 1.0 do not support type {}", base_type)
            }
            Error::InvalidValue => write!(f, "invalid proto::Value"),
        }
    }
}

/// FIT is FIT protocol data representative.
#[derive(Default, Debug)]
pub struct FIT {
    /// File Header contains either 12 or 14 bytes
    pub file_header: FileHeader,
    /// Messages
    pub messages: Vec<Message>,
    /// Cyclic Redundancy Check 16-bit value to ensure the integrity of the messages.
    pub crc: u16,
}

/// FileHeader is a FIT's FileHeader with either 12 bytes size without CRC or a 14 bytes size with CRC,
/// while 14 bytes size is the preferred size.
#[derive(Debug, Default, Clone, Copy)]
pub struct FileHeader {
    /// File header size either 12 (legacy) or 14.
    pub size: u8,
    /// The FIT Protocol version which is being used to encode the FIT file.
    pub protocol_version: ProtocolVersion,
    /// The FIT Profile Version (associated with data defined in Global FIT Profile).
    pub profile_version: u16,
    /// The size of the messages in bytes (this field will be automatically updated by the encoder)
    pub data_size: u32,
    /// ".FIT" (an immutable string constant)
    pub data_type: &'static str,
    /// Cyclic Redundancy Check 16-bit value to ensure the integrity of the file header.
    /// (this field will be automatically updated by the encoder)
    pub crc: u16,
}

impl FileHeader {
    pub(crate) fn marshal_append(&self, vec: &mut Vec<u8>) {
        vec.push(self.size);
        vec.push(self.protocol_version.0);
        vec.extend_from_slice(&self.profile_version.to_le_bytes());
        vec.extend_from_slice(&self.data_size.to_le_bytes());
        vec.extend_from_slice(DATA_TYPE.as_bytes());
        if self.size >= 14 {
            vec.extend_from_slice(&self.crc.to_le_bytes());
        }
    }
}

/// MessageDefinition is the definition of the upcoming data messages.
#[derive(Debug, Default, Clone)]
pub struct MessageDefinition {
    /// The message definition header with mask 0b01000000.
    pub header: u8,
    /// Currently undetermined; the default value is 0.
    pub reserved: u8,
    /// The Byte Order to be used to decode the values of both this message definition and the upcoming message. (0: Little-Endian, 1: Big-Endian)
    pub arch: u8,
    /// Global Message Number defined by factory (retrieved from Profile.xslx). (endianness of this 2 Byte value is defined in the Architecture byte)
    pub mesg_num: MesgNum,
    /// List of the field definition
    pub field_definitions: Vec<FieldDefinition>,
    /// List of the developer field definition (only if Developer Data Flag is set in Header)
    pub developer_field_definitions: Vec<DeveloperFieldDefinition>,
}

/// FieldDefinition is the definition of the upcoming field within the message's structure.
#[derive(Debug, Default, Clone, Copy)]
pub struct FieldDefinition {
    /// The field definition number
    pub num: u8,
    /// The size of the upcoming value
    pub size: u8,
    /// The type of the upcoming value to be represented
    pub base_type: FitBaseType,
}

/// FieldDefinition is the definition of the upcoming developer field within the message's structure.
#[derive(Debug, Default, Clone, Copy)]
pub struct DeveloperFieldDefinition {
    /// Maps to `field_definition_number` of a `field_description` message.
    pub num: u8,
    /// Size (in bytes) of the specified FIT message’s field
    pub size: u8,
    /// Maps to `developer_data_index` of a `developer_data_id` and a `field_description` messages.
    pub developer_data_index: u8,
}

/// Message is a FIT protocol message containing the data defined in the Message Definition
#[derive(Default, Debug, Clone)]
pub struct Message {
    /// Message Header serves to distinguish whether the message is a Normal Data or a Compressed Timestamp Data.
    /// Unlike MessageDefinition, Message's Header should not contain Developer Data Flag.
    pub header: u8,
    /// Global Message Number defined in Global FIT Profile, except number within range 0xFF00 - 0xFFFE are manufacturer specific number.
    pub num: MesgNum,
    /// List of Field
    pub fields: Vec<Field>,
    /// List of DeveloperField
    pub developer_fields: Vec<DeveloperField>,
}

impl Message {
    /// SubFieldSubstitution returns any sub-field that can substitute
    /// the properties interpretation of the parent Field (Dynamic Field).
    pub fn sub_field_substitution<'a>(
        &self,
        field_ref: &FieldReference<'a>,
    ) -> Option<&'a SubField<'a>> {
        for sub_field in field_ref.sub_fields {
            for sub_field_map in sub_field.maps {
                for field in &self.fields {
                    if field.num != sub_field_map.ref_field_num {
                        continue;
                    }
                    if field.value.cast_i64() == sub_field_map.ref_field_value {
                        return Some(sub_field);
                    }
                }
            }
        }
        None
    }

    pub(crate) fn marshal_append(&self, vec: &mut Vec<u8>, arch: u8) -> Result<(), Error> {
        vec.push(self.header);
        for field in &self.fields {
            field.value.marshal_append(vec, arch)?;
        }
        for dev_field in &self.developer_fields {
            dev_field.value.marshal_append(vec, arch)?;
        }
        Ok(())
    }
}

/// Field represents the full representation of a field, as specified in the Global FIT Profile.
#[derive(Default, Debug, Clone)]
pub struct Field {
    /// Defined in the Global FIT profile for the specified FIT message, otherwise
    /// its a manufaturer specific number (defined by manufacturer). (255 == invalid)
    pub num: u8,
    /// BaseType is the base of the ProfileType. E.g. ProfileType::DateTime -> FitBaseType::Uint32.
    pub profile_type: ProfileType,
    /// Value
    pub value: Value,
    /// A flag to detect whether this field is generated through component expansion.
    pub is_expanded: bool,
}

/// FieldReference acts as a representation of a field as defined in the Global FIT Profile.
#[derive(Debug, Clone, Copy)]
pub struct FieldReference<'a> {
    /// Defined in the Global FIT profile for the specified FIT message, otherwise
    /// its a manufaturer specific number (defined by manufacturer). (255 == invalid)
    pub num: u8,
    /// Defined in the Global FIT profile for the specified FIT message, otherwise
    /// its a manufaturer specific name (defined by manufacturer).
    pub name: &'a str,
    /// Type is defined type that serves as an abstraction layer above base types (primitive-types),
    /// e.g. DateTime is a time representation in uint32.
    pub base_type: FitBaseType,
    /// ProfileType represents more than just a type of Value. E.g. ProfileType::DateTime -> FitBaseType::Uint32.
    /// When an u32 value having ProfileType::DateTime as type, the value represents time.
    pub profile_type: ProfileType,
    /// Flag whether the value of this field is an array
    pub array: bool,
    /// Flag to indicate if the value of the field is accumulable.
    pub accumulate: bool,
    /// A scale or offset specified in the FIT profile for binary fields (sint/uint etc.) only.
    /// The binary quantity is divided by the scale factor and then the offset is subtracted. (default: 1)
    pub scale: f64,
    /// A scale or offset specified in the FIT profile for binary fields (sint/uint etc.) only.
    /// The binary quantity is divided by the scale factor and then the offset is subtracted. (default: 0)
    pub offset: f64,
    /// Units of the value, such as m (meter), m/s (meter per second), s (second), etc.
    pub units: &'a str,
    /// List of component
    pub components: &'a [Component],
    /// List of sub-field
    pub sub_fields: &'a [SubField<'a>],
}

/// DeveloperField is a way to add custom data fields to existing messages. Developer Data Fields can be added
/// to any message at runtime by providing a self-describing FieldDefinition messages prior to that message.
/// The combination of the DeveloperDataIndex and FieldDefinitionNumber create a unique id for each FieldDescription.
/// Developer Data Fields are also used by the Connect IQ FIT Contributor library, allowing Connect IQ apps
/// and data fields to include custom data in FIT Activity files during the recording of activities.
///
/// NOTE: If DeveloperField contains a valid NativeMesgNum and NativeFieldNum, the value should be treated as
/// native value (scale, offset, etc shall apply). [Added since protocol version 2.0]
#[derive(Debug, Clone)]
pub struct DeveloperField {
    /// Maps to `field_definition_number` of a `field_description` message.
    pub num: u8,
    /// Maps to `developer_data_index` of a `developer_data_id` and a `field_description` messages.
    pub developer_data_index: u8,
    /// Value
    pub value: Value,
}

/// Component is a way of compressing one or more fields into a bit field expressed in a single containing field.
/// The component can be expanded as a main Field in a Message or to update the value of the destination main Field.
#[derive(Debug)]
pub struct Component {
    /// Refer to Field's Number.
    pub field_num: u8,
    /// A flag whether this component should be accumulated.
    pub accumulate: bool,
    /// The size of data of this component in bits  
    pub bits: u8,
    /// Similar to FieldReference's scale, but for this component.
    pub scale: f64,
    /// Similar to FieldReference's offset, but for this component.
    pub offset: f64,
}

/// SubField is a dynamic interpretation of the main Field in a Message when the SubFieldMap mapping match. See SubFieldMap's docs.
#[derive(Debug)]
pub struct SubField<'a> {
    /// Name
    pub name: &'a str,
    /// ProfileType
    pub profile_type: ProfileType,
    /// Scale
    pub scale: f64,
    /// Offset
    pub offset: f64,
    /// Units
    pub units: &'a str,
    /// List of SubFieldMap
    pub maps: &'a [SubFieldMap],
    /// List of Component
    pub components: &'a [Component],
}

/// SubFieldMap is the mapping between SubField and the corresponding main Field in a Message.
/// When any Field in a Message has Field.Num == RefFieldNum and Field.Value == RefFieldValue, then the SubField containing
/// this mapping can be interpreted as the main Field's properties (name, scale, type etc.)
#[derive(Debug)]
pub struct SubFieldMap {
    /// Mapping reference to targeted Field's Number.
    pub ref_field_num: u8,
    /// Mapping reference to targeted Field's Value.
    pub ref_field_value: i64,
}

/// Value representation of Message's Field.
#[allow(missing_docs)]
#[derive(Debug, Clone, Default)]
pub enum Value {
    #[default]
    Invalid,
    Int8(i8),
    Uint8(u8),
    Int16(i16),
    Uint16(u16),
    Int32(i32),
    Uint32(u32),
    String(String),
    Float32(f32),
    Float64(f64),
    Int64(i64),
    Uint64(u64),
    VecInt8(Vec<i8>),
    VecUint8(Vec<u8>),
    VecInt16(Vec<i16>),
    VecUint16(Vec<u16>),
    VecInt32(Vec<i32>),
    VecUint32(Vec<u32>),
    VecString(Vec<String>),
    VecFloat32(Vec<f32>),
    VecFloat64(Vec<f64>),
    VecInt64(Vec<i64>),
    VecUint64(Vec<u64>),
}

/// Value that can hold FIT's value.
impl Value {
    /// Unmarshal bytes into Value.
    //
    // This is hotpath. We intentionally avoid using chunks_exact() iterator, as
    // it introduces a slight performance overhead compared to using a standard loop.
    pub(crate) fn unmarshal(buf: &[u8], array: bool, base_type: FitBaseType, arch: u8) -> Value {
        match base_type {
            FitBaseType::SINT8 => match array {
                true => Value::VecInt8({
                    let mut vals: Vec<i8> = Vec::with_capacity(buf.len());
                    for &v in buf {
                        vals.push(v as i8);
                    }
                    vals
                }),
                false => Value::Int8(buf[0] as i8),
            },
            FitBaseType::ENUM | FitBaseType::BYTE | FitBaseType::UINT8 | FitBaseType::UINT8Z => {
                match array {
                    true => Value::VecUint8(buf.to_vec()),
                    false => Value::Uint8(buf[0]),
                }
            }
            FitBaseType::SINT16 => match array {
                true => Value::VecInt16({
                    let mut vals: Vec<i16> = Vec::with_capacity(buf.len() / 2);
                    let mut buf = buf;
                    while buf.len() >= 2 {
                        vals.push(match arch {
                            0 => i16::from_le_bytes(buf[..2].try_into().unwrap()),
                            _ => i16::from_be_bytes(buf[..2].try_into().unwrap()),
                        });
                        buf = &buf[2..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Int16(i16::from_le_bytes(buf[..2].try_into().unwrap())),
                    _ => Value::Int16(i16::from_be_bytes(buf[..2].try_into().unwrap())),
                },
            },
            FitBaseType::UINT16 | FitBaseType::UINT16Z => match array {
                true => Value::VecUint16({
                    let mut vals: Vec<u16> = Vec::with_capacity(buf.len() / 2);
                    let mut buf = buf;
                    while buf.len() >= 2 {
                        vals.push(match arch {
                            0 => u16::from_le_bytes(buf[..2].try_into().unwrap()),
                            _ => u16::from_be_bytes(buf[..2].try_into().unwrap()),
                        });
                        buf = &buf[2..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Uint16(u16::from_le_bytes(buf[..2].try_into().unwrap())),
                    _ => Value::Uint16(u16::from_be_bytes(buf[..2].try_into().unwrap())),
                },
            },
            FitBaseType::SINT32 => match array {
                true => Value::VecInt32({
                    let mut vals: Vec<i32> = Vec::with_capacity(buf.len() / 4);
                    let mut buf = buf;
                    while buf.len() >= 4 {
                        vals.push(match arch {
                            0 => i32::from_le_bytes(buf[..4].try_into().unwrap()),
                            _ => i32::from_be_bytes(buf[..4].try_into().unwrap()),
                        });
                        buf = &buf[4..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Int32(i32::from_le_bytes(buf[..4].try_into().unwrap())),
                    _ => Value::Int32(i32::from_be_bytes(buf[..4].try_into().unwrap())),
                },
            },
            FitBaseType::UINT32 | FitBaseType::UINT32Z => match array {
                true => Value::VecUint32({
                    let mut vals: Vec<u32> = Vec::with_capacity(buf.len() / 4);
                    let mut buf = buf;
                    while buf.len() >= 4 {
                        vals.push(match arch {
                            0 => u32::from_le_bytes(buf[..4].try_into().unwrap()),
                            _ => u32::from_be_bytes(buf[..4].try_into().unwrap()),
                        });
                        buf = &buf[4..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Uint32(u32::from_le_bytes(buf[..4].try_into().unwrap())),
                    _ => Value::Uint32(u32::from_be_bytes(buf[..4].try_into().unwrap())),
                },
            },
            FitBaseType::STRING => match array {
                true => Value::String(String::from_utf8_lossy(buf).to_string()),
                false => Value::VecString({
                    let mut vals = Vec::with_capacity(strcount(buf) as usize);
                    let mut last = 0usize;
                    for (i, &v) in buf.iter().enumerate() {
                        if v != 0 {
                            continue;
                        }
                        if last != i {
                            vals.push(String::from_utf8_lossy(&buf[last..i]).into_owned());
                        }
                        last = i + 1
                    }
                    vals
                }),
            },
            FitBaseType::FLOAT32 => match array {
                true => Value::VecFloat32({
                    let mut vals: Vec<f32> = Vec::with_capacity(buf.len() / 4);
                    let mut buf = buf;
                    while buf.len() >= 4 {
                        vals.push(match arch {
                            0 => f32::from_le_bytes(buf[..4].try_into().unwrap()),
                            _ => f32::from_be_bytes(buf[..4].try_into().unwrap()),
                        });
                        buf = &buf[4..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Float32(f32::from_le_bytes(buf[..4].try_into().unwrap())),
                    _ => Value::Float32(f32::from_be_bytes(buf[..4].try_into().unwrap())),
                },
            },
            FitBaseType::FLOAT64 => match array {
                true => Value::VecFloat64({
                    let mut vals: Vec<f64> = Vec::with_capacity(buf.len() / 8);
                    let mut buf = buf;
                    while buf.len() >= 2 {
                        vals.push(match arch {
                            0 => f64::from_le_bytes(buf[..8].try_into().unwrap()),
                            _ => f64::from_be_bytes(buf[..8].try_into().unwrap()),
                        });
                        buf = &buf[8..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Float64(f64::from_le_bytes(buf[..8].try_into().unwrap())),
                    _ => Value::Float64(f64::from_be_bytes(buf[..8].try_into().unwrap())),
                },
            },
            FitBaseType::SINT64 => match array {
                true => Value::VecInt64({
                    let mut vals: Vec<i64> = Vec::with_capacity(buf.len() / 8);
                    let mut buf = buf;
                    while buf.len() >= 8 {
                        vals.push(match arch {
                            0 => i64::from_le_bytes(buf[..8].try_into().unwrap()),
                            _ => i64::from_be_bytes(buf[..8].try_into().unwrap()),
                        });
                        buf = &buf[8..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Int64(i64::from_le_bytes(buf[..8].try_into().unwrap())),
                    _ => Value::Int64(i64::from_be_bytes(buf[..8].try_into().unwrap())),
                },
            },
            FitBaseType::UINT64 | FitBaseType::UINT64Z => match array {
                true => Value::VecUint64({
                    let mut vals: Vec<u64> = Vec::with_capacity(buf.len() / 8);
                    let mut buf = buf;
                    while buf.len() >= 8 {
                        vals.push(match arch {
                            0 => u64::from_le_bytes(buf[..8].try_into().unwrap()),
                            _ => u64::from_be_bytes(buf[..8].try_into().unwrap()),
                        });
                        buf = &buf[8..];
                    }
                    vals
                }),
                false => match arch {
                    0 => Value::Uint64(u64::from_le_bytes(buf[..8].try_into().unwrap())),
                    _ => Value::Uint64(u64::from_be_bytes(buf[..8].try_into().unwrap())),
                },
            },
            _ => Value::Invalid,
        }
    }

    pub(crate) fn marshal_append(&self, vec: &mut Vec<u8>, arch: u8) -> Result<(), Error> {
        match self {
            Value::Int8(v) => vec.push(*v as u8),
            Value::Uint8(v) => vec.push(*v),
            Value::Int16(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Uint16(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Int32(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Uint32(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::String(v) => {
                let b = v.as_bytes();
                vec.extend_from_slice(b);
                if v.is_empty() || b[b.len() - 1] != 0 {
                    vec.push(0);
                }
            }
            Value::Float32(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Float64(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Int64(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::Uint64(v) => vec.extend_from_slice(&match arch {
                0 => v.to_le_bytes(),
                _ => v.to_be_bytes(),
            }),
            Value::VecInt8(v) => {
                for &x in v {
                    vec.push(x as u8);
                }
            }
            Value::VecUint8(v) => {
                for &x in v {
                    vec.push(x);
                }
            }
            Value::VecInt16(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecUint16(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecInt32(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecUint32(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecString(v) => {
                for x in v {
                    let b = x.as_bytes();
                    vec.extend_from_slice(b);
                    if v.is_empty() || b[b.len() - 1] != 0 {
                        vec.push(0);
                    }
                }
            }
            Value::VecFloat32(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecFloat64(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecInt64(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::VecUint64(v) => match arch {
                0 => {
                    for x in v {
                        vec.extend_from_slice(&x.to_le_bytes());
                    }
                }
                _ => {
                    for x in v {
                        vec.extend_from_slice(&x.to_be_bytes());
                    }
                }
            },
            Value::Invalid => return Err(Error::InvalidValue),
        };
        Ok(())
    }

    /// cast_i64 converts any integer value into i64. If the value is not an integer, this returns i64::MAX.
    pub(crate) fn cast_i64(&self) -> i64 {
        match self {
            Value::Uint8(v) => *v as i64,
            Value::Int8(v) => *v as i64,
            Value::Int16(v) => *v as i64,
            Value::Uint16(v) => *v as i64,
            Value::Int32(v) => *v as i64,
            Value::Uint32(v) => *v as i64,
            Value::Int64(v) => *v,
            Value::Uint64(v) => *v as i64,
            _ => i64::MAX,
        }
    }

    /// Returns i8 or its invalid value when it's not an i8 value.
    pub fn as_i8(&self) -> i8 {
        if let Value::Int8(v) = self {
            return *v;
        }
        i8::MAX
    }

    /// Returns u8 or its invalid value when it's not an u8 value.
    pub fn as_u8(&self) -> u8 {
        if let Value::Uint8(v) = self {
            return *v;
        }
        u8::MAX
    }

    /// Returns u8z or its invalid value when it's not an u8z value.
    pub fn as_u8z(&self) -> u8 {
        if let Value::Uint8(v) = self {
            return *v;
        }
        0
    }

    /// Returns i16 or its invalid value when it's not an i16 value.
    pub fn as_i16(&self) -> i16 {
        if let Value::Int16(v) = self {
            return *v;
        }
        i16::MAX
    }

    /// Returns u16 or its invalid value when it's not an u16 value.
    pub fn as_u16(&self) -> u16 {
        if let Value::Uint16(v) = self {
            return *v;
        }
        u16::MAX
    }

    /// Returns u16z or its invalid value when it's not an u16z value.
    pub fn as_u16z(&self) -> u16 {
        if let Value::Uint16(v) = self {
            return *v;
        }
        0
    }

    /// Returns i32 or its invalid value when it's not an i32 value.
    pub fn as_i32(&self) -> i32 {
        if let Value::Int32(v) = self {
            return *v;
        }
        i32::MAX
    }

    /// Returns u32 or its invalid value when it's not an u32 value.
    pub fn as_u32(&self) -> u32 {
        if let Value::Uint32(v) = self {
            return *v;
        }
        u32::MAX
    }

    /// Returns u32z or its invalid value when it's not an u32z value.
    pub fn as_u32z(&self) -> u32 {
        if let Value::Uint32(v) = self {
            return *v;
        }
        0
    }

    /// Returns string (clone) or empty string when it's not a string.
    pub fn as_string(&self) -> String {
        if let Value::String(v) = self {
            return v.clone();
        }
        String::new()
    }

    /// Returns f32 or its invalid value when it's not a f32 value.
    pub fn as_f32(&self) -> f32 {
        if let Value::Float32(v) = self {
            return *v;
        }
        f32::MAX
    }

    /// Returns f64 or its invalid value when it's not a f64 value.
    pub fn as_f64(&self) -> f64 {
        if let Value::Float64(v) = self {
            return *v;
        }
        f64::MAX
    }

    /// Returns i64 or its invalid value when it's not an i64 value.
    pub fn as_i64(&self) -> i64 {
        if let Value::Int64(v) = self {
            return *v;
        }
        i64::MAX
    }

    /// Returns u64 or its invalid value when it's not an u64 value.
    pub fn as_u64(&self) -> u64 {
        if let Value::Uint64(v) = self {
            return *v;
        }
        u64::MAX
    }

    /// Returns u64z or its invalid value when it's not an u64z value.
    pub fn as_u64z(&self) -> u64 {
        if let Value::Uint64(v) = self {
            return *v;
        }
        0
    }

    /// Returns Vec<i8> (clone) or empty vector when it's not a Vec<i8> value.
    pub fn as_vec_i8(&self) -> Vec<i8> {
        if let Value::VecInt8(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<u8> (clone) or empty vector when it's not a Vec<u8> value.
    pub fn as_vec_u8(&self) -> Vec<u8> {
        if let Value::VecUint8(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<i16> (clone) or empty vector when it's not a Vec<i16> value.
    pub fn as_vec_i16(&self) -> Vec<i16> {
        if let Value::VecInt16(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<u16> (clone) or empty vector when it's not a Vec<u16> value.
    pub fn as_vec_u16(&self) -> Vec<u16> {
        if let Value::VecUint16(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<i32> (clone) or empty vector when it's not a Vec<i32> value.
    pub fn as_vec_i32(&self) -> Vec<i32> {
        if let Value::VecInt32(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<u32> (clone) or empty vector when it's not a Vec<u32> value.
    pub fn as_vec_u32(&self) -> Vec<u32> {
        if let Value::VecUint32(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<String> (clone) or empty vector when it's not a Vec<String> value.
    pub fn as_vec_string(&self) -> Vec<String> {
        if let Value::VecString(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<f32> (clone) or empty vector when it's not a Vec<f32> value.
    pub fn as_vec_f32(&self) -> Vec<f32> {
        if let Value::VecFloat32(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<f64> (clone) or empty vector when it's not a Vec<f64> value.
    pub fn as_vec_f64(&self) -> Vec<f64> {
        if let Value::VecFloat64(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<i64> (clone) or empty vector when it's not a Vec<i64> value.
    pub fn as_vec_i64(&self) -> Vec<i64> {
        if let Value::VecInt64(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Returns Vec<u64> (clone) or empty vector when it's not a Vec<u64> value.
    pub fn as_vec_u64(&self) -> Vec<u64> {
        if let Value::VecUint64(v) = self {
            return v.clone();
        }
        Vec::new()
    }

    /// Checks whether Value holds any representation of invalid value based on base_type.
    /// An array is invalid when all elements represent invalid value.
    pub(crate) fn is_valid(&self, base_type: FitBaseType) -> bool {
        match self {
            Value::Int8(v) => *v != i8::MAX,
            Value::Uint8(v) => match base_type {
                FitBaseType::UINT8 | FitBaseType::ENUM | FitBaseType::BYTE => *v != u8::MAX,
                FitBaseType::UINT8Z => *v != 0,
                _ => false,
            },
            Value::Int16(v) => *v != i16::MAX,
            Value::Uint16(v) => match base_type {
                FitBaseType::UINT16 => *v != u16::MAX,
                FitBaseType::UINT16Z => *v != 0,
                _ => false,
            },
            Value::Int32(v) => *v != i32::MAX,
            Value::Uint32(v) => match base_type {
                FitBaseType::UINT32 => *v != u32::MAX,
                FitBaseType::UINT32Z => *v != 0,
                _ => false,
            },
            Value::String(v) => !v.is_empty() && v.as_str() != "\x00",
            Value::Float32(v) => f32::to_bits(*v) != u32::MAX,
            Value::Float64(v) => f64::to_bits(*v) != u64::MAX,
            Value::Int64(v) => *v != i64::MAX,
            Value::Uint64(v) => match base_type {
                FitBaseType::UINT64 => *v != u64::MAX,
                FitBaseType::UINT64Z => *v != 0,
                _ => false,
            },
            Value::VecInt8(v) => {
                for &x in v {
                    if x != i8::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecUint8(v) => match base_type {
                FitBaseType::UINT8 => {
                    for &x in v {
                        if x != u8::MAX {
                            return true;
                        }
                    }
                    false
                }
                FitBaseType::UINT8Z => {
                    for &x in v {
                        if x != u8::MIN {
                            return true;
                        }
                    }
                    false
                }
                _ => false,
            },
            Value::VecInt16(v) => {
                for &x in v {
                    if x != i16::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecUint16(v) => match base_type {
                FitBaseType::UINT16 => {
                    for &x in v {
                        if x != u16::MAX {
                            return true;
                        }
                    }
                    false
                }
                FitBaseType::UINT16Z => {
                    for &x in v {
                        if x != u16::MIN {
                            return true;
                        }
                    }
                    false
                }
                _ => false,
            },
            Value::VecInt32(v) => {
                for &x in v {
                    if x != i32::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecUint32(v) => match base_type {
                FitBaseType::UINT32 => {
                    for &x in v {
                        if x != u32::MAX {
                            return true;
                        }
                    }
                    false
                }
                FitBaseType::UINT32Z => {
                    for &x in v {
                        if x != u32::MIN {
                            return true;
                        }
                    }
                    false
                }
                _ => false,
            },
            Value::VecString(v) => {
                for x in v {
                    if !x.is_empty() && x.as_str() != "\x00" {
                        return true;
                    }
                }
                false
            }
            Value::VecFloat32(v) => {
                for &x in v {
                    if x.to_bits() != u32::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecFloat64(v) => {
                for &x in v {
                    if x.to_bits() != u64::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecInt64(v) => {
                for &x in v {
                    if x != i64::MAX {
                        return true;
                    }
                }
                false
            }
            Value::VecUint64(v) => match base_type {
                FitBaseType::UINT64 => {
                    for &x in v {
                        if x != u64::MAX {
                            return true;
                        }
                    }
                    false
                }
                FitBaseType::UINT64Z => {
                    for &x in v {
                        if x != u64::MIN {
                            return true;
                        }
                    }
                    false
                }
                _ => false,
            },
            Value::Invalid => false,
        }
    }

    /// Checks whether Value's type is align with given basetype.
    pub(crate) fn is_align(&self, base_type: FitBaseType) -> bool {
        match self {
            Value::Int8(_) => base_type == FitBaseType::SINT8,
            Value::Uint8(_) => {
                base_type == FitBaseType::ENUM
                    || base_type == FitBaseType::UINT8
                    || base_type == FitBaseType::UINT8Z
                    || base_type == FitBaseType::BYTE
            }
            Value::Int16(_) => base_type == FitBaseType::SINT16,
            Value::Uint16(_) => {
                base_type == FitBaseType::UINT16 || base_type == FitBaseType::UINT16Z
            }
            Value::Int32(_) => base_type == FitBaseType::SINT32,
            Value::Uint32(_) => {
                base_type == FitBaseType::UINT32 || base_type == FitBaseType::UINT32Z
            }
            Value::String(_) => base_type == FitBaseType::STRING,
            Value::Float32(_) => base_type == FitBaseType::FLOAT32,
            Value::Float64(_) => base_type == FitBaseType::FLOAT64,
            Value::Int64(_) => base_type == FitBaseType::SINT64,
            Value::Uint64(_) => {
                base_type == FitBaseType::UINT64 || base_type == FitBaseType::UINT64Z
            }
            Value::VecInt8(_) => base_type == FitBaseType::SINT8,
            Value::VecUint8(_) => {
                base_type == FitBaseType::ENUM
                    || base_type == FitBaseType::UINT8
                    || base_type == FitBaseType::UINT8Z
                    || base_type == FitBaseType::BYTE
            }
            Value::VecInt16(_) => base_type == FitBaseType::SINT16,
            Value::VecUint16(_) => {
                base_type == FitBaseType::UINT16 || base_type == FitBaseType::UINT16Z
            }
            Value::VecInt32(_) => base_type == FitBaseType::SINT32,
            Value::VecUint32(_) => {
                base_type == FitBaseType::UINT32 || base_type == FitBaseType::UINT32Z
            }
            Value::VecString(_) => base_type == FitBaseType::STRING,
            Value::VecFloat32(_) => base_type == FitBaseType::FLOAT32,
            Value::VecFloat64(_) => base_type == FitBaseType::FLOAT64,
            Value::VecInt64(_) => base_type == FitBaseType::SINT64,
            Value::VecUint64(_) => {
                base_type == FitBaseType::UINT64 || base_type == FitBaseType::UINT64Z
            }
            _ => false,
        }
    }

    /// Returns the size of Value in binary from. For every string in Value,
    /// if the last index of the string is not '\x00', size += 1.
    pub(crate) fn size(&self) -> usize {
        match self {
            Value::Int8(_) | Value::Uint8(_) => 1,
            Value::Int16(_) | Value::Uint16(_) => 2,
            Value::Int32(_) | Value::Uint32(_) | Value::Float32(_) => 4,
            Value::Int64(_) | Value::Uint64(_) | Value::Float64(_) => 8,
            Value::String(v) => {
                let n = v.len();
                if n == 0 || v.as_bytes()[n - 1] != 0 {
                    return n + 1;
                }
                n
            }
            Value::VecInt8(v) => v.len(),
            Value::VecUint8(v) => v.len(),
            Value::VecInt16(v) => v.len() * 2,
            Value::VecUint16(v) => v.len() * 2,
            Value::VecInt32(v) => v.len() * 4,
            Value::VecUint32(v) => v.len() * 4,
            Value::VecFloat32(v) => v.len() * 4,
            Value::VecFloat64(v) => v.len() * 8,
            Value::VecInt64(v) => v.len() * 8,
            Value::VecUint64(v) => v.len() * 8,
            Value::VecString(v) => {
                let mut size = 0usize;
                for x in v {
                    let mut n = x.len();
                    if n == 0 || x.as_bytes()[n - 1] != 0 {
                        n += 1;
                    }
                    size += n;
                }
                size
            }
            Value::Invalid => 0,
        }
    }
}

/// Protocol Version representation.
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct ProtocolVersion(pub u8);

#[allow(missing_docs)]
impl ProtocolVersion {
    pub const V1: ProtocolVersion = ProtocolVersion(1 << 4);
    pub const V2: ProtocolVersion = ProtocolVersion(2 << 4);

    /// Returns major version.
    pub fn major(self) -> u8 {
        self.0 >> 4
    }

    /// Returns minor version.
    pub fn minor(self) -> u8 {
        self.0 | ((1 << 4) - 1)
    }
}

/// Validates whether the message contains unsupported data for the targeted protocol version.
pub(crate) fn validate_message(
    mesg: &Message,
    protocol_version: ProtocolVersion,
) -> Result<(), Error> {
    if protocol_version == ProtocolVersion::V1 {
        if !mesg.developer_fields.is_empty() {
            return Err(Error::ProtocolViolationDeveloperData);
        }
        for field in &mesg.fields {
            if field.profile_type.base_type().0 & FitBaseType::NUM_MASK
                > FitBaseType::BYTE.0 & FitBaseType::NUM_MASK
            {
                return Err(Error::ProtocolViolationUnsupportedBaseType(
                    field.profile_type.base_type(),
                ));
            }
        }
    }
    Ok(())
}

/// Counts how many valid utf-8 string in s.
pub(crate) fn strcount(s: &[u8]) -> u8 {
    let mut last = 0usize;
    let mut size = 0u8;
    for (i, &v) in s.iter().enumerate() {
        if v != 0 {
            continue;
        }
        if last != i {
            size += 1
        }
        last = i + 1
    }
    if size == 0 && !s.is_empty() {
        return 1; // Allow string without utf-8 null termination.
    }
    size
}

#[cfg(test)]
mod tests {
    use crate::{
        profile::typedef::FitBaseType,
        proto::{Value, strcount},
    };

    #[test]
    fn test_strcount() {
        #[derive(Default, Clone, Copy)]
        struct Case {
            input: &'static str,
            expected: u8,
        }

        let tt = [
            Case {
                input: "Open Water",
                expected: 1,
            },
            Case {
                input: "Open Water\x00",
                expected: 1,
            },
            Case {
                input: "Open Water\x00\x00",
                expected: 1,
            },
            Case {
                input: "Open\x00Water\x00",
                expected: 2,
            },
        ];

        for (i, tc) in tt.iter().enumerate() {
            let v = strcount(tc.input.as_bytes());
            assert_eq!(v, tc.expected, "index {} input \"{}\"", i, tc.input);
        }
    }

    #[test]
    fn test_value_is_valid() {
        struct Case {
            value: Value,
            base_type: FitBaseType,
            is_valid: bool,
        }

        let tt = [
            Case {
                value: Value::Int8(i8::MIN),
                base_type: FitBaseType::SINT8,
                is_valid: true,
            },
            Case {
                value: Value::Int8(i8::MAX),
                base_type: FitBaseType::SINT8,
                is_valid: false,
            },
            Case {
                value: Value::Uint8(u8::MIN),
                base_type: FitBaseType::UINT8,
                is_valid: true,
            },
            Case {
                value: Value::Uint8(u8::MAX),
                base_type: FitBaseType::UINT8,
                is_valid: false,
            },
            Case {
                value: Value::Uint8(u8::MIN),
                base_type: FitBaseType::UINT8Z,
                is_valid: false,
            },
            Case {
                value: Value::Uint8(u8::MAX),
                base_type: FitBaseType::UINT8Z,
                is_valid: true,
            },
            Case {
                value: Value::Int16(i16::MIN),
                base_type: FitBaseType::SINT16,
                is_valid: true,
            },
            Case {
                value: Value::Int16(i16::MAX),
                base_type: FitBaseType::SINT16,
                is_valid: false,
            },
            Case {
                value: Value::Uint16(u16::MIN),
                base_type: FitBaseType::UINT16,
                is_valid: true,
            },
            Case {
                value: Value::Uint16(u16::MAX),
                base_type: FitBaseType::UINT16,
                is_valid: false,
            },
            Case {
                value: Value::Uint16(u16::MIN),
                base_type: FitBaseType::UINT16Z,
                is_valid: false,
            },
            Case {
                value: Value::Uint16(u16::MAX),
                base_type: FitBaseType::UINT16Z,
                is_valid: true,
            },
            Case {
                value: Value::Int32(i32::MIN),
                base_type: FitBaseType::SINT32,
                is_valid: true,
            },
            Case {
                value: Value::Int32(i32::MAX),
                base_type: FitBaseType::SINT32,
                is_valid: false,
            },
            Case {
                value: Value::Uint32(u32::MIN),
                base_type: FitBaseType::UINT32,
                is_valid: true,
            },
            Case {
                value: Value::Uint32(u32::MAX),
                base_type: FitBaseType::UINT32,
                is_valid: false,
            },
            Case {
                value: Value::Uint32(u32::MIN),
                base_type: FitBaseType::UINT32Z,
                is_valid: false,
            },
            Case {
                value: Value::Uint32(u32::MAX),
                base_type: FitBaseType::UINT32Z,
                is_valid: true,
            },
            Case {
                value: Value::Float32(f32::MIN),
                base_type: FitBaseType::FLOAT32,
                is_valid: true,
            },
            Case {
                value: Value::Float32(f32::from_bits(u32::MAX)),
                base_type: FitBaseType::FLOAT32,
                is_valid: false,
            },
            Case {
                value: Value::Float64(f64::MIN),
                base_type: FitBaseType::FLOAT64,
                is_valid: true,
            },
            Case {
                value: Value::Float64(f64::from_bits(u64::MAX)),
                base_type: FitBaseType::FLOAT64,
                is_valid: false,
            },
            Case {
                value: Value::Int64(i64::MIN),
                base_type: FitBaseType::SINT64,
                is_valid: true,
            },
            Case {
                value: Value::Int64(i64::MAX),
                base_type: FitBaseType::SINT64,
                is_valid: false,
            },
            Case {
                value: Value::Uint64(u64::MIN),
                base_type: FitBaseType::UINT64,
                is_valid: true,
            },
            Case {
                value: Value::Uint64(u64::MAX),
                base_type: FitBaseType::UINT64,
                is_valid: false,
            },
            Case {
                value: Value::Uint64(u64::MIN),
                base_type: FitBaseType::UINT64Z,
                is_valid: false,
            },
            Case {
                value: Value::Uint64(u64::MAX),
                base_type: FitBaseType::UINT64Z,
                is_valid: true,
            },
            Case {
                value: Value::String("rustyfit".to_string()),
                base_type: FitBaseType::STRING,
                is_valid: true,
            },
            Case {
                value: Value::String("".to_string()),
                base_type: FitBaseType::STRING,
                is_valid: false,
            },
            Case {
                value: Value::String("\x00".to_string()),
                base_type: FitBaseType::STRING,
                is_valid: false,
            },
            Case {
                value: Value::VecInt8(vec![0i8, 1i8]),
                base_type: FitBaseType::SINT8,
                is_valid: true,
            },
            Case {
                value: Value::VecInt8(vec![i8::MAX, i8::MAX]),
                base_type: FitBaseType::SINT8,
                is_valid: false,
            },
            Case {
                value: Value::VecUint8(vec![0u8, 1u8]),
                base_type: FitBaseType::UINT8,
                is_valid: true,
            },
            Case {
                value: Value::VecUint8(vec![0u8, 1u8]),
                base_type: FitBaseType::UINT8Z,
                is_valid: true,
            },
            Case {
                value: Value::VecUint8(vec![u8::MAX, u8::MAX]),
                base_type: FitBaseType::UINT8,
                is_valid: false,
            },
            Case {
                value: Value::VecUint8(vec![u8::MIN, u8::MIN]),
                base_type: FitBaseType::UINT8Z,
                is_valid: false,
            },
            Case {
                value: Value::VecInt16(vec![0i16, 1i16]),
                base_type: FitBaseType::SINT16,
                is_valid: true,
            },
            Case {
                value: Value::VecInt16(vec![i16::MAX, i16::MAX]),
                base_type: FitBaseType::SINT16,
                is_valid: false,
            },
            Case {
                value: Value::VecUint16(vec![0u16, 1u16]),
                base_type: FitBaseType::UINT16,
                is_valid: true,
            },
            Case {
                value: Value::VecUint16(vec![0u16, 1u16]),
                base_type: FitBaseType::UINT16Z,
                is_valid: true,
            },
            Case {
                value: Value::VecUint16(vec![u16::MAX, u16::MAX]),
                base_type: FitBaseType::UINT16,
                is_valid: false,
            },
            Case {
                value: Value::VecUint16(vec![u16::MIN, u16::MIN]),
                base_type: FitBaseType::UINT16Z,
                is_valid: false,
            },
            Case {
                value: Value::VecInt32(vec![0i32, 1i32]),
                base_type: FitBaseType::SINT32,
                is_valid: true,
            },
            Case {
                value: Value::VecInt32(vec![i32::MAX, i32::MAX]),
                base_type: FitBaseType::SINT32,
                is_valid: false,
            },
            Case {
                value: Value::VecUint32(vec![0u32, 1u32]),
                base_type: FitBaseType::UINT32,
                is_valid: true,
            },
            Case {
                value: Value::VecUint32(vec![0u32, 1u32]),
                base_type: FitBaseType::UINT32Z,
                is_valid: true,
            },
            Case {
                value: Value::VecUint32(vec![u32::MAX, u32::MAX]),
                base_type: FitBaseType::UINT32,
                is_valid: false,
            },
            Case {
                value: Value::VecUint32(vec![u32::MIN, u32::MIN]),
                base_type: FitBaseType::UINT32Z,
                is_valid: false,
            },
            Case {
                value: Value::VecFloat32(vec![0f32, 1f32]),
                base_type: FitBaseType::FLOAT32,
                is_valid: true,
            },
            Case {
                value: Value::VecFloat32(vec![f32::from_bits(u32::MAX), f32::from_bits(u32::MAX)]),
                base_type: FitBaseType::FLOAT32,
                is_valid: false,
            },
            Case {
                value: Value::VecFloat64(vec![0f64, 1f64]),
                base_type: FitBaseType::FLOAT64,
                is_valid: true,
            },
            Case {
                value: Value::VecFloat64(vec![f64::from_bits(u64::MAX), f64::from_bits(u64::MAX)]),
                base_type: FitBaseType::FLOAT64,
                is_valid: false,
            },
            Case {
                value: Value::VecInt64(vec![0i64, 1i64]),
                base_type: FitBaseType::SINT64,
                is_valid: true,
            },
            Case {
                value: Value::VecInt64(vec![i64::MAX, i64::MAX]),
                base_type: FitBaseType::SINT64,
                is_valid: false,
            },
            Case {
                value: Value::VecUint64(vec![0u64, 1u64]),
                base_type: FitBaseType::UINT64,
                is_valid: true,
            },
            Case {
                value: Value::VecUint64(vec![0u64, 1u64]),
                base_type: FitBaseType::UINT64Z,
                is_valid: true,
            },
            Case {
                value: Value::VecUint64(vec![u64::MAX, u64::MAX]),
                base_type: FitBaseType::UINT64,
                is_valid: false,
            },
            Case {
                value: Value::VecUint64(vec![u64::MIN, u64::MIN]),
                base_type: FitBaseType::UINT64Z,
                is_valid: false,
            },
            Case {
                value: Value::VecString(vec!["rustyfit".to_string(), "rustyfit".to_string()]),
                base_type: FitBaseType::STRING,
                is_valid: true,
            },
            Case {
                value: Value::VecString(vec!["\x00".to_string(), "\x00".to_string()]),
                base_type: FitBaseType::STRING,
                is_valid: false,
            },
        ];

        for (i, tc) in tt.iter().enumerate() {
            let is_valid = tc.value.is_valid(tc.base_type);
            assert_eq!(
                tc.is_valid, is_valid,
                "{}: {:?} | {}",
                i, tc.value, tc.base_type,
            );
        }
    }
}
