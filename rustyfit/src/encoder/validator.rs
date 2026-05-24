#![warn(missing_docs)]

use crate::{
    profile::{
        mesgdef,
        typedef::{FitBaseType, MesgNum},
    },
    proto::{LocalFieldDescription, Message, ProtocolVersion, Value},
};
use alloc::vec::Vec;

/// Error occurs during message validation
#[derive(Debug)]
#[allow(missing_docs)]
pub enum MessageValidationError {
    /// Message has zero fields and zero developer fields.
    EmptyMessageData,
    /// Developer data is unsupported in protocol v1.
    UnsupportedDeveloperData,
    /// Some base types are unsupported in protocol v1.
    UnsupportedFitBaseType {
        field_index: usize,
        base_type: FitBaseType,
    },
    /// Field contains invalid data.
    FieldValidation {
        field_index: usize,
        err: FieldValidationError,
    },
    /// DeveloperField contains invalid data.
    DeveloperFieldValidation {
        developer_field_index: usize,
        err: FieldValidationError,
    },
    /// Developer fields must have prior DeveloperDataId message.
    MissingDeveloperDataId { developer_field_index: usize },
    /// Developer fields must have prior FieldDescription message.
    MissingFieldDescription { developer_field_index: usize },
}

impl core::fmt::Display for MessageValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::EmptyMessageData => {
                write!(f, "message has zero fields and zero developer fields")
            }
            Self::UnsupportedDeveloperData => {
                write!(f, "developer data is unsupported for protocol v1")
            }
            Self::UnsupportedFitBaseType {
                field_index,
                base_type,
            } => write!(
                f,
                "fit base type {} on field index {} is unsupported for protocol v1",
                base_type, field_index
            ),
            Self::FieldValidation { field_index, err } => {
                write!(f, "field validation: field index {}: {}", field_index, err)
            }
            Self::DeveloperFieldValidation {
                developer_field_index,
                err,
            } => write!(
                f,
                "developer field validation: developer field index {}: {}",
                developer_field_index, err
            ),
            Self::MissingDeveloperDataId {
                developer_field_index,
            } => write!(
                f,
                "missing developer data id for developer field index {}",
                developer_field_index
            ),
            Self::MissingFieldDescription {
                developer_field_index,
            } => write!(
                f,
                "missing field description for developer field index {}",
                developer_field_index
            ),
        }
    }
}

impl core::error::Error for MessageValidationError {}

/// Error occurs during value `fields` or `developer_fields` validation.
/// The context depends on where the validation takes place.
#[derive(Debug)]
#[allow(missing_docs)]
pub enum FieldValidationError {
    ValueTypeInvalid,
    StringValueInvalid,
    ValueSizeLimitExceeded,
    FieldLimitExceeded,
}

impl core::fmt::Display for FieldValidationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self {
            Self::ValueTypeInvalid => write!(f, "value type is invalid"),
            Self::StringValueInvalid => write!(f, "string value contains invalid UTF-8 characters"),
            Self::ValueSizeLimitExceeded => {
                write!(f, "value's size in bytes exceeds 255 bytes limit")
            }
            Self::FieldLimitExceeded => {
                write!(f, "fields or developer_fields exceeds 255 items limit")
            }
        }
    }
}

impl core::error::Error for FieldValidationError {}

pub(super) struct MessageValidator {
    developer_data_index_seen: [u64; 4],
    field_descriptions: Vec<LocalFieldDescription>,
}

impl MessageValidator {
    pub(super) const fn new() -> Self {
        Self {
            developer_data_index_seen: [0u64; 4],
            field_descriptions: Vec::new(),
        }
    }

    pub(super) fn validate_message(
        &mut self,
        mesg: &mut Message,
        protocol_version: ProtocolVersion,
    ) -> Result<(), MessageValidationError> {
        // Some data are unsupported on protocol v1
        if protocol_version == ProtocolVersion::V1 {
            if !mesg.developer_fields.is_empty() {
                return Err(MessageValidationError::UnsupportedDeveloperData);
            }

            for (i, field) in mesg.fields.iter().enumerate() {
                if field.profile_type.base_type().0 & FitBaseType::NUM_MASK
                    > FitBaseType::BYTE.0 & FitBaseType::NUM_MASK
                {
                    return Err(MessageValidationError::UnsupportedFitBaseType {
                        base_type: field.profile_type.base_type(),
                        field_index: i,
                    });
                }
            }
        }

        let mut valid = 0usize;
        for i in 0..mesg.fields.len() {
            let field = &mesg.fields[i];
            if field.is_expanded {
                continue;
            }

            if let Err(err) = self.validate_field(&field.value, field.profile_type.base_type()) {
                return Err(MessageValidationError::FieldValidation {
                    field_index: i,
                    err,
                });
            }

            if valid == 255 {
                return Err(MessageValidationError::FieldValidation {
                    field_index: i,
                    err: FieldValidationError::FieldLimitExceeded,
                });
            }

            if i != valid {
                mesg.fields.swap(i, valid);
            }

            valid += 1;
        }

        mesg.fields.truncate(valid);

        match mesg.num {
            MesgNum::DEVELOPER_DATA_ID => {
                if let Some(field) = mesg
                    .fields
                    .iter()
                    .find(|field| field.num == mesgdef::DeveloperDataId::DEVELOPER_DATA_INDEX)
                    && let Value::Uint8(v) = field.value
                {
                    self.developer_data_index_seen[(v as usize) >> 6] |= 1 << (v & 63)
                }
            }
            MesgNum::FIELD_DESCRIPTION => self
                .field_descriptions
                .push(LocalFieldDescription::from(&*mesg)),
            _ => {}
        };

        for i in 0..mesg.developer_fields.len() {
            let dev_field = &mesg.developer_fields[i];

            let x = dev_field.developer_data_index;
            if (self.developer_data_index_seen[(x as usize) >> 6] >> (x & 63)) & 1 == 0 {
                return Err(MessageValidationError::MissingDeveloperDataId {
                    developer_field_index: i,
                });
            }

            match self.field_descriptions.iter().find(|v| {
                v.developer_data_index == dev_field.developer_data_index
                    && v.field_definition_number == dev_field.num
            }) {
                Some(v) => {
                    if let Err(err) = self.validate_field(&dev_field.value, v.fit_base_type_id) {
                        return Err(MessageValidationError::DeveloperFieldValidation {
                            developer_field_index: i,
                            err,
                        });
                    }
                }
                None => {
                    return Err(MessageValidationError::MissingFieldDescription {
                        developer_field_index: i,
                    });
                }
            };

            if i == 255 {
                return Err(MessageValidationError::DeveloperFieldValidation {
                    developer_field_index: i,
                    err: FieldValidationError::FieldLimitExceeded,
                });
            }
        }

        if mesg.fields.is_empty() && mesg.developer_fields.is_empty() {
            return Err(MessageValidationError::EmptyMessageData);
        }

        Ok(())
    }

    fn validate_field(
        &self,
        value: &Value,
        base_type: FitBaseType,
    ) -> Result<(), FieldValidationError> {
        if !value.is_align(base_type) {
            return Err(FieldValidationError::ValueTypeInvalid);
        }

        match value {
            Value::String(v) if core::str::from_utf8(v.as_bytes()).is_err() => {
                return Err(FieldValidationError::StringValueInvalid);
            }
            Value::VecString(v) => {
                for x in v.iter() {
                    if core::str::from_utf8(x.as_bytes()).is_err() {
                        return Err(FieldValidationError::StringValueInvalid);
                    }
                }
            }
            _ => {}
        };

        if value.size() > 255 {
            return Err(FieldValidationError::ValueSizeLimitExceeded);
        }

        Ok(())
    }

    pub(super) fn reset(&mut self) {
        self.developer_data_index_seen.fill(0);
        self.field_descriptions.clear();
    }
}
