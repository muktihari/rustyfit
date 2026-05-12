use crate::{profile::lookup, proto::Value};

const N: usize = ((lookup::MAX_COMPONENT_BITS / 8) + 7) / 8;

#[cfg_attr(test, derive(Debug, PartialEq))]
pub(super) struct Bits {
    store: [u64; N],
    size: u64,
}

impl Bits {
    pub(super) fn new(value: &Value) -> Option<Self> {
        let mut bits = Self {
            store: [0u64; N],
            size: 0,
        };

        match value {
            Value::Int8(v) => (bits.store[0], bits.size) = (*v as u64, 8),
            Value::Uint8(v) => (bits.store[0], bits.size) = (*v as u64, 8),
            Value::Int16(v) => (bits.store[0], bits.size) = (*v as u64, 16),
            Value::Uint16(v) => (bits.store[0], bits.size) = (*v as u64, 16),
            Value::Int32(v) => (bits.store[0], bits.size) = (*v as u64, 32),
            Value::Uint32(v) => (bits.store[0], bits.size) = (*v as u64, 32),
            Value::Float32(v) => (bits.store[0], bits.size) = (*v as u64, 32),
            Value::Float64(v) => (bits.store[0], bits.size) = (*v as u64, 64),
            Value::Int64(v) => (bits.store[0], bits.size) = (*v as u64, 64),
            Value::Uint64(v) => (bits.store[0], bits.size) = (*v, 64),
            Value::VecInt8(v) => bits_from_slice(&mut bits, v, 8),
            Value::VecUint8(v) => bits_from_slice(&mut bits, v, 8),
            Value::VecInt16(v) => bits_from_slice(&mut bits, v, 16),
            Value::VecUint16(v) => bits_from_slice(&mut bits, v, 16),
            Value::VecInt32(v) => bits_from_slice(&mut bits, v, 32),
            Value::VecUint32(v) => bits_from_slice(&mut bits, v, 32),
            Value::VecFloat32(v) => bits_from_slice(&mut bits, v, 32),
            Value::VecFloat64(v) => bits_from_slice(&mut bits, v, 64),
            Value::VecInt64(v) => bits_from_slice(&mut bits, v, 64),
            Value::VecUint64(v) => bits_from_slice(&mut bits, v, 64),
            _ => {
                return None;
            }
        };
        Some(bits)
    }

    pub(super) fn pull(&mut self, bits: u8) -> Option<u64> {
        if self.size == 0 {
            return None;
        }

        self.size = self.size.saturating_sub(bits as u64);

        let mask = (1u64 << bits) - 1;
        let val = self.store[0] & mask;
        self.store[0] >>= bits;

        for i in 1..self.store.len() {
            if self.store[i] == 0 {
                continue;
            }
            let hi = self.store[i] & mask;
            let lo = hi << (64 - bits);
            self.store[i - 1] |= lo;
            self.store[i] >>= bits;
        }

        Some(val)
    }
}

trait AsU64: Copy {
    fn as_u64(self) -> u64;
}

macro_rules! impl_as_u64 {
    ($($type:ident),*) => {
        $(impl AsU64 for $type {
            fn as_u64(self) -> u64 {
                self as u64
            }
        })*
    };
}

impl_as_u64!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

fn bits_from_slice<T: AsU64>(v: &mut Bits, s: &[T], bitsize: usize) {
    for i in 0..s.len() {
        let x = i * bitsize;
        let index = x >> 6;
        if index >= v.store.len() {
            break;
        }
        v.store[index] |= s[i].as_u64() << (x & 63);
        v.size += bitsize as u64;
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        decoder::{Bits, bits::N},
        proto::Value,
    };

    #[test]
    fn make_bits() {
        struct TestCase {
            value: Value,
            expected: Option<Bits>,
        }

        let tt = vec![
            TestCase {
                value: Value::Int8(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 8,
                }),
            },
            TestCase {
                value: Value::Uint8(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 8,
                }),
            },
            TestCase {
                value: Value::Int16(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 16,
                }),
            },
            TestCase {
                value: Value::Uint16(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 16,
                }),
            },
            TestCase {
                value: Value::Int32(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 32,
                }),
            },
            TestCase {
                value: Value::Uint32(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 32,
                }),
            },
            TestCase {
                value: Value::Float32(10.0),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 32,
                }),
            },
            TestCase {
                value: Value::Float64(10.0),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::Int64(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::Uint64(10),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::VecInt8(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 8);
                        tmp
                    },
                    size: 16,
                }),
            },
            TestCase {
                value: Value::VecUint8(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 8);
                        tmp
                    },
                    size: 16,
                }),
            },
            TestCase {
                value: Value::VecInt16(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 16);
                        tmp
                    },
                    size: 32,
                }),
            },
            TestCase {
                value: Value::VecUint16(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 16);
                        tmp
                    },
                    size: 32,
                }),
            },
            TestCase {
                value: Value::VecInt32(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 32);
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::VecUint32(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 32);
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::VecFloat32(vec![1.0, 2.0]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | (2 << 32);
                        tmp
                    },
                    size: 64,
                }),
            },
            TestCase {
                value: Value::VecFloat64(vec![1.0, 2.0]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1;
                        tmp[1] = 2;
                        tmp
                    },
                    size: 128,
                }),
            },
            TestCase {
                value: Value::VecInt64(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1;
                        tmp[1] = 2;
                        tmp
                    },
                    size: 128,
                }),
            },
            TestCase {
                value: Value::VecUint64(vec![1, 2]),
                expected: Some(Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1;
                        tmp[1] = 2;
                        tmp
                    },
                    size: 128,
                }),
            },
            TestCase {
                value: Value::VecUint8(vec![0; 255]),
                expected: Some(Bits {
                    store: [0u64; N],
                    size: 64 * (N as u64),
                }),
            },
        ];

        for tc in tt {
            let vbits = Bits::new(&tc.value);
            assert_eq!(tc.expected, vbits)
        }
    }

    #[test]
    fn pull() {
        struct Pull {
            bits: u8,
            value: Option<u64>,
            vbits: Bits,
        }

        struct TestCase {
            name: &'static str,
            vbits: Bits,
            pulls: Vec<Pull>,
        }

        let mut tt = vec![
            TestCase {
                name: "single value one pull",
                vbits: Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 10;
                        tmp
                    },
                    size: 8,
                },
                pulls: vec![Pull {
                    bits: 8,
                    value: Some(10),
                    vbits: Bits {
                        store: [0u64; N],
                        size: 0,
                    },
                }],
            },
            TestCase {
                name: "single value two pull",
                vbits: Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 1 | 2 << 8;
                        tmp
                    },
                    size: 16,
                },
                pulls: vec![
                    Pull {
                        bits: 8,
                        value: Some(1),
                        vbits: Bits {
                            store: {
                                let mut tmp = [0u64; N];
                                tmp[0] = 2;
                                tmp
                            },
                            size: 8,
                        },
                    },
                    Pull {
                        bits: 8,
                        value: Some(2),
                        vbits: Bits {
                            store: [0u64; N],
                            size: 0,
                        },
                    },
                ],
            },
            TestCase {
                name: "multiple value one pull",
                vbits: Bits {
                    store: {
                        let mut tmp = [0u64; N];
                        tmp[0] = 0x0000_0000_2701_0E08;
                        tmp[1] = 0x0000_0000_0000_FFFF;
                        tmp
                    },
                    size: 128,
                },
                pulls: vec![Pull {
                    bits: 8,
                    value: Some(0x08),
                    vbits: Bits {
                        store: {
                            let mut tmp = [0u64; N];
                            tmp[0] = 0xFF00_0000_0027_010E;
                            tmp[1] = 0x0000_0000_0000_00FF;
                            tmp
                        },
                        size: 120,
                    },
                }],
            },
            TestCase {
                name: "size is already zero",
                vbits: Bits {
                    store: [0u64; N],
                    size: 0,
                },
                pulls: vec![Pull {
                    bits: 8,
                    value: None,
                    vbits: Bits {
                        store: [0u64; N],
                        size: 0,
                    },
                }],
            },
            TestCase {
                name: "size 8 bits, pull 16 bits, return 8 bits",
                vbits: Bits {
                    store: [0u64; N],
                    size: 8,
                },
                pulls: vec![Pull {
                    bits: 16,
                    value: Some(0),
                    vbits: Bits {
                        store: [0u64; N],
                        size: 0,
                    },
                }],
            },
        ];

        for tc in &mut tt {
            for (i, pull) in &mut tc.pulls.iter_mut().enumerate() {
                let value = tc.vbits.pull(pull.bits);
                assert_eq!(pull.value, value, "{}: pulls[{}]", tc.name, i);
                assert_eq!(tc.vbits, pull.vbits, "{}: pills[{}]", tc.name, i);
            }
        }
    }
}
