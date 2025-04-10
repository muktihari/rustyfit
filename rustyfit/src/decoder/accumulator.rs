use crate::{profile::typedef::MesgNum, proto::Value};

pub(super) struct Accumulator {
    values: Vec<AccuValue>,
}

impl Accumulator {
    pub(super) const fn new() -> Self {
        Self { values: Vec::new() }
    }

    fn collect_u32(&mut self, mesg_num: MesgNum, field_num: u8, value: u32) {
        for v in &mut self.values {
            if v.mesg_num == mesg_num && v.field_num == field_num {
                v.value = value;
                v.last = value;
                break;
            }
        }
        self.values.push(AccuValue {
            mesg_num,
            field_num,
            value,
            last: value,
        });
    }

    pub(super) fn collect(&mut self, mesg_num: MesgNum, field_num: u8, value: &Value) {
        match value {
            Value::Uint8(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Int8(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Uint16(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Int16(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Uint32(v) => self.collect_u32(mesg_num, field_num, *v),
            Value::Int32(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Float32(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Float64(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Int64(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::Uint64(v) => self.collect_u32(mesg_num, field_num, *v as u32),
            Value::VecInt8(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecUint8(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecInt16(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecUint16(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecInt32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecUint32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x);
                }
            }
            Value::VecFloat32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecFloat64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecInt64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            Value::VecUint64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u32(mesg_num, field_num, x as u32);
                }
            }
            _ => {}
        }
    }

    pub(super) fn accumulate(
        &mut self,
        mesg_num: MesgNum,
        field_num: u8,
        value: u32,
        bits: u8,
    ) -> u32 {
        for v in &mut self.values {
            if v.mesg_num == mesg_num && v.field_num == field_num {
                let mask: u32 = (1 << bits) - 1;
                v.value += (value.wrapping_sub(v.last)) & mask;
                v.last = value;
                return v.value;
            }
        }
        self.values.push(AccuValue {
            mesg_num,
            field_num,
            value,
            last: value,
        });
        value
    }

    pub(super) fn reset(&mut self) {
        self.values.clear();
    }
}

struct AccuValue {
    mesg_num: MesgNum,
    field_num: u8,
    value: u32,
    last: u32,
}
