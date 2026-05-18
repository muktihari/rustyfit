use crate::{profile::typedef::MesgNum, proto::Value};
use alloc::vec::Vec;

pub(super) struct Accumulator {
    values: Vec<AccuValue>,
}

impl Accumulator {
    pub(super) const fn new() -> Self {
        Self { values: Vec::new() }
    }

    pub(super) fn collect(&mut self, mesg_num: MesgNum, field_num: u8, value: &Value) {
        match value {
            Value::Uint8(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Int8(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Uint16(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Int16(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Uint32(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Int32(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Float32(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Float64(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Int64(v) => self.collect_u64(mesg_num, field_num, *v as u64),
            Value::Uint64(v) => self.collect_u64(mesg_num, field_num, *v),
            Value::VecInt8(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecUint8(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecInt16(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecUint16(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecInt32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecUint32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecFloat32(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecFloat64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecInt64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x as u64);
                }
            }
            Value::VecUint64(v) => {
                if let Some(&x) = v.last() {
                    self.collect_u64(mesg_num, field_num, x);
                }
            }
            _ => {}
        }
    }

    fn collect_u64(&mut self, mesg_num: MesgNum, field_num: u8, value: u64) {
        self.values
            .iter_mut()
            .find(|v| v.mesg_num == mesg_num && v.field_num == field_num)
            .map(|v| {
                v.value = value;
                v.last = value;
            })
            .unwrap_or_else(|| {
                self.values.push(AccuValue {
                    mesg_num,
                    field_num,
                    value,
                    last: value,
                });
            });
    }

    pub(super) fn accumulate(
        &mut self,
        mesg_num: MesgNum,
        field_num: u8,
        value: u64,
        bits: u8,
    ) -> u64 {
        self.values
            .iter_mut()
            .find(|v| v.mesg_num == mesg_num && v.field_num == field_num)
            .map(|v| {
                let mask: u64 = (1 << bits) - 1;
                v.value += (value.wrapping_sub(v.last)) & mask;
                v.last = value;
                v.value
            })
            .unwrap_or_else(|| {
                self.values.push(AccuValue {
                    mesg_num,
                    field_num,
                    value,
                    last: value,
                });
                value
            })
    }

    pub(super) fn reset(&mut self) {
        self.values.clear();
    }
}

#[cfg_attr(test, derive(PartialEq, Debug))]
struct AccuValue {
    mesg_num: MesgNum,
    field_num: u8,
    value: u64,
    last: u64,
}

#[cfg(test)]
mod tests {
    use crate::{
        decoder::{Accumulator, accumulator::AccuValue},
        profile::typedef::MesgNum,
        proto::Value,
    };
    use alloc::{string::String, vec, vec::Vec};

    #[test]
    fn test_collect() {
        let expected = vec![AccuValue {
            mesg_num: MesgNum(1),
            field_num: 1,
            value: 2,
            last: 2,
        }];

        let tt = [
            Value::Int8(2),
            Value::Uint8(2),
            Value::Int16(2),
            Value::Uint16(2),
            Value::Int32(2),
            Value::Uint32(2),
            Value::Float32(2.0),
            Value::Float64(2.0),
            Value::Int64(2),
            Value::Uint64(2),
            Value::VecInt8(vec![1, 2]),
            Value::VecUint8(vec![1, 2]),
            Value::VecInt16(vec![1, 2]),
            Value::VecUint16(vec![1, 2]),
            Value::VecInt32(vec![1, 2]),
            Value::VecUint32(vec![1, 2]),
            Value::VecFloat32(vec![1.0, 2.0]),
            Value::VecFloat64(vec![1.0, 2.0]),
            Value::VecInt64(vec![1, 2]),
            Value::VecUint64(vec![1, 2]),
        ];

        for tc in tt {
            let mut accumu = Accumulator::new();
            accumu.collect(MesgNum(1), 1, &tc);
            assert_eq!(expected, accumu.values, "case: {:?}", tc);
        }

        let tt = [
            Value::Invalid,
            Value::String(String::new()),
            Value::VecString(vec![]),
        ];

        for tc in tt {
            let mut accumu = Accumulator::new();
            accumu.collect(MesgNum(1), 1, &tc);
            assert_eq!(Vec::<AccuValue>::new(), accumu.values, "case: {:?}", tc);
        }
    }

    #[test]
    fn test_collect_64() {
        let mut accumu = Accumulator::new();

        accumu.collect_u64(MesgNum(0), 0, 10);
        assert_eq!(
            vec![AccuValue {
                mesg_num: MesgNum(0),
                field_num: 0,
                value: 10,
                last: 10
            }],
            accumu.values,
        );

        accumu.collect_u64(MesgNum(0), 0, 11);
        assert_eq!(
            vec![AccuValue {
                mesg_num: MesgNum(0),
                field_num: 0,
                value: 11,
                last: 11
            }],
            accumu.values,
        );

        accumu.collect_u64(MesgNum(0), 1, 11);
        assert_eq!(
            vec![
                AccuValue {
                    mesg_num: MesgNum(0),
                    field_num: 0,
                    value: 11,
                    last: 11
                },
                AccuValue {
                    mesg_num: MesgNum(0),
                    field_num: 1,
                    value: 11,
                    last: 11
                }
            ],
            accumu.values,
        );
    }

    #[test]
    fn test_accumulate() {
        let mut accumu = Accumulator::new();

        accumu.collect_u64(MesgNum(1), 1, 1);
        let val = accumu.accumulate(MesgNum(2), 2, 2, 8);
        assert_eq!(2, val, "accumulate non-existing value");

        let val = accumu.accumulate(MesgNum(1), 1, 10, 8);
        assert_eq!(10, val, "accumulate first value");
        assert_eq!(
            vec![
                AccuValue {
                    mesg_num: MesgNum(1),
                    field_num: 1,
                    value: 10,
                    last: 10,
                },
                AccuValue {
                    mesg_num: MesgNum(2),
                    field_num: 2,
                    value: 2,
                    last: 2,
                }
            ],
            accumu.values,
            "first value is updated, non-existing value is appended"
        );
    }

    #[test]
    fn reset() {
        let mut accumu = Accumulator::new();
        accumu.collect_u64(MesgNum(1), 1, 1);
        assert_eq!(1, accumu.values.len());
        accumu.reset();
        assert_eq!(0, accumu.values.len());
    }
}
