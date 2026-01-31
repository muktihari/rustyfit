#![warn(missing_docs)]

use std::fmt;

use crate::{
    profile::{
        mesgdef,
        typedef::{FitBaseType, MesgNum},
    },
    proto::{Message, Value},
};

#[derive(Debug, Clone)]
pub enum MessageValidatorError {
    /// Message has no fields.
    NoFields,
    /// Field value integrity
    FieldValueIntegrity {
        field_index: usize,
        err: IntegrityError,
    },
    /// Developer field value integrity
    DeveloperFieldValueIntegrity {
        developer_field_index: usize,
        err: IntegrityError,
    },
    /// Missing developer data id
    MissingDeveloperDataId { developer_field_index: usize },
    /// Missing field description
    MissingFieldDescription { developer_field_index: usize },
}

impl fmt::Display for MessageValidatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::NoFields => write!(f, "no fields"),
            Self::FieldValueIntegrity { field_index, err } => write!(
                f,
                "value integrity: field index {}, err: {}",
                field_index, err
            ),
            Self::DeveloperFieldValueIntegrity {
                developer_field_index,
                err,
            } => write!(
                f,
                "value integrity: developer field index {}, err: {}",
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

#[derive(Debug, Clone)]
pub enum IntegrityError {
    /// Value and base type is not align
    ValueBaseTypeNotAlign {
        value: Value,
        base_type: FitBaseType,
    },
    /// Invalid UTF-8 string in a value.
    InvalidUTF8String(String),
    /// Value size exceed maximum limit (255 bytes)
    ValueSizeExceedLimit,
}

impl fmt::Display for IntegrityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self {
            Self::ValueBaseTypeNotAlign { value, base_type } => {
                write!(
                    f,
                    "value {:?} is not align with base type {}",
                    value, base_type
                )
            }
            Self::InvalidUTF8String(value) => write!(f, "invalid UTF-8 string: {:?}", value),
            Self::ValueSizeExceedLimit => write!(f, "value's size exceed limit"),
        }
    }
}

pub(super) struct MessageValidator {
    developer_data_index_seen: [u64; 4],
    field_descriptions: Vec<mesgdef::FieldDescription>,
}

impl MessageValidator {
    pub(super) fn new() -> Self {
        Self {
            developer_data_index_seen: [0u64; 4],
            field_descriptions: Vec::new(),
        }
    }

    pub(super) fn validate_message(
        &mut self,
        mesg: &mut Message,
    ) -> Result<(), MessageValidatorError> {
        let mut valid = 0usize;
        for i in 0..mesg.fields.len() {
            let field = &mesg.fields[i];
            if field.is_expanded {
                continue;
            }

            if let Err(err) = self.value_integrity(&field.value, field.profile_type.base_type()) {
                return Err(MessageValidatorError::FieldValueIntegrity {
                    field_index: i,
                    err,
                });
            }

            if valid == 255 {
                return Err(MessageValidatorError::FieldValueIntegrity {
                    field_index: i,
                    err: IntegrityError::ValueSizeExceedLimit,
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
                {
                    if let Value::Uint8(v) = field.value {
                        self.developer_data_index_seen[(v as usize) >> 6] |= 1 << (v & 63)
                    }
                }
            }
            MesgNum::FIELD_DESCRIPTION => self
                .field_descriptions
                .push(mesgdef::FieldDescription::from(mesg as &Message)),
            _ => {}
        };

        valid = 0;
        for i in 0..mesg.developer_fields.len() {
            let dev_field = &mesg.developer_fields[i];

            let x = dev_field.developer_data_index;
            if (self.developer_data_index_seen[(x as usize) >> 6] >> (x & 63)) & 1 == 0 {
                return Err(MessageValidatorError::MissingDeveloperDataId {
                    developer_field_index: i,
                });
            }

            match self.field_descriptions.iter().find(|v| {
                v.developer_data_index == dev_field.developer_data_index
                    && v.field_definition_number == dev_field.num
            }) {
                Some(v) => {
                    if !dev_field.value.is_valid(v.fit_base_type_id) {
                        continue;
                    }
                    if let Err(err) = self.value_integrity(&dev_field.value, v.fit_base_type_id) {
                        return Err(MessageValidatorError::DeveloperFieldValueIntegrity {
                            developer_field_index: i,
                            err,
                        });
                    }
                }
                None => {
                    return Err(MessageValidatorError::MissingFieldDescription {
                        developer_field_index: i,
                    });
                }
            };

            if valid == 255 {
                return Err(MessageValidatorError::DeveloperFieldValueIntegrity {
                    developer_field_index: i,
                    err: IntegrityError::ValueSizeExceedLimit,
                });
            }

            if i != valid {
                mesg.developer_fields.swap(i, valid);
            }

            valid += 1;
        }

        mesg.developer_fields.truncate(valid);

        if mesg.fields.is_empty() && mesg.developer_fields.is_empty() {
            return Err(MessageValidatorError::NoFields);
        }

        Ok(())
    }

    fn value_integrity(&self, value: &Value, base_type: FitBaseType) -> Result<(), IntegrityError> {
        if !value.is_align(base_type) {
            return Err(IntegrityError::ValueBaseTypeNotAlign {
                value: value.clone(),
                base_type,
            });
        }

        match value {
            Value::String(v) => {
                if std::str::from_utf8(v.as_bytes()).is_err() {
                    return Err(IntegrityError::InvalidUTF8String(v.clone()));
                }
            }
            Value::VecString(v) => {
                for x in v.iter() {
                    if std::str::from_utf8(x.as_bytes()).is_err() {
                        return Err(IntegrityError::InvalidUTF8String(x.clone()));
                    }
                }
            }
            _ => {}
        };

        let size = value.size();
        if size > 255 {
            return Err(IntegrityError::ValueSizeExceedLimit);
        }

        Ok(())
    }

    pub(super) fn reset(&mut self) {
        self.developer_data_index_seen.fill(0);
        self.field_descriptions.clear();
    }
}
