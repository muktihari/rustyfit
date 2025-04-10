use crate::{
    profile::{
        mesgdef,
        typedef::{FitBaseType, MesgNum},
    },
    proto::{Message, Value},
};

#[derive(Debug, Clone)]
pub enum MessageValidatorError {
    ValueIntegrity(IntegrityError, String),
    MissingDeveloperDataId(String),
    MissingFieldDescription(String),
    NoValidFields,
}

#[derive(Debug, Clone)]
pub enum IntegrityError {
    ValueBaseTypeNotAlign(String),
    InvalidUTF8String(String),
    ValueSizeExceedLimit,
}

pub(super) struct MessageValidator {
    omit_invalid_value: bool,
    developer_data_indexes: Vec<u8>,
    field_descriptions: Vec<mesgdef::FieldDescription>,
}

impl MessageValidator {
    pub(super) fn new(omit_invalid_value: bool) -> Self {
        Self {
            omit_invalid_value,
            developer_data_indexes: Vec::new(),
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

            if self.omit_invalid_value && !field.value.is_valid(field.base_type) {
                continue;
            }

            if let Err(err) = self.value_integrity(&field.value, field.base_type) {
                return Err(MessageValidatorError::ValueIntegrity(
                    err,
                    format!("fields[{}], num: {}", i, field.num),
                ));
            }

            if valid == 255 {
                return Err(MessageValidatorError::ValueIntegrity(
                    IntegrityError::ValueSizeExceedLimit,
                    format!("fields[{}], num: {}", i, field.num),
                ));
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
                        self.developer_data_indexes.push(v);
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

            let mut ok = false;
            for &developer_data_index in &self.developer_data_indexes {
                if developer_data_index == dev_field.developer_data_index {
                    ok = true;
                    break;
                }
            }

            if !ok {
                return Err(MessageValidatorError::MissingDeveloperDataId(format!(
                    "developer_fields[{}], num: {}",
                    i, dev_field.num
                )));
            }

            let mut field_desc: Option<&mesgdef::FieldDescription> = None;
            for v in &self.field_descriptions {
                if v.developer_data_index == dev_field.developer_data_index
                    && v.field_definition_number == dev_field.num
                {
                    field_desc = Some(v);
                    break;
                }
            }

            match field_desc {
                Some(v) => {
                    if !dev_field.value.is_valid(v.fit_base_type_id) {
                        continue;
                    }
                    if let Err(err) = self.value_integrity(&dev_field.value, v.fit_base_type_id) {
                        return Err(MessageValidatorError::ValueIntegrity(
                            err,
                            format!("developer_fields[{}], num: {}", i, dev_field.num),
                        ));
                    }
                }
                None => {
                    return Err(MessageValidatorError::MissingFieldDescription(format!(
                        "developer_fields[{}], num: {}",
                        i, dev_field.num
                    )));
                }
            };

            if valid == 255 {
                return Err(MessageValidatorError::ValueIntegrity(
                    IntegrityError::ValueSizeExceedLimit,
                    format!("developer_fields[{}], num: {}", i, dev_field.num),
                ));
            }

            if i != valid {
                mesg.developer_fields.swap(i, valid);
            }

            valid += 1;
        }

        mesg.developer_fields.truncate(valid);

        if mesg.fields.is_empty() && mesg.developer_fields.is_empty() {
            return Err(MessageValidatorError::NoValidFields);
        }

        Ok(())
    }

    fn value_integrity(&self, value: &Value, base_type: FitBaseType) -> Result<(), IntegrityError> {
        if !value.is_align(base_type) {
            return Err(IntegrityError::ValueBaseTypeNotAlign(format!(
                "value {:?} is not align with base_type '{}'",
                value, base_type
            )));
        }

        match value {
            Value::String(v) => {
                if std::str::from_utf8(v.as_bytes()).is_err() {
                    return Err(IntegrityError::InvalidUTF8String(format!(
                        "\"{}\" is not a valid utf-8 string",
                        v
                    )));
                }
            }
            Value::VecString(v) => {
                for (i, x) in v.iter().enumerate() {
                    if std::str::from_utf8(x.as_bytes()).is_err() {
                        return Err(IntegrityError::InvalidUTF8String(format!(
                            "[{}] \"{}\" is not a valid utf-8 string",
                            i, x
                        )));
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
        self.field_descriptions.clear();
    }
}
