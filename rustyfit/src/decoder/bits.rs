use crate::{profile::lookup, proto::Value};

const N: usize = ((lookup::MAX_COMPONENT_BITS / 8) + 7) / 8;

pub(super) struct Bits {
    store: [u64; N],
}

impl Bits {
    pub(super) fn new(value: &Value) -> Option<Self> {
        let mut bits = Bits { store: [0u64; N] };

        match value {
            Value::Int8(v) => bits.store[0] = *v as u64,
            Value::Uint8(v) => bits.store[0] = *v as u64,
            Value::Int16(v) => bits.store[0] = *v as u64,
            Value::Uint16(v) => bits.store[0] = *v as u64,
            Value::Int32(v) => bits.store[0] = *v as u64,
            Value::Uint32(v) => bits.store[0] = *v as u64,
            Value::Float32(v) => bits.store[0] = *v as u64,
            Value::Float64(v) => bits.store[0] = *v as u64,
            Value::Int64(v) => bits.store[0] = *v as u64,
            Value::Uint64(v) => bits.store[0] = *v,
            Value::VecInt8(v) => store_from_vec(&mut bits.store, v, 1),
            Value::VecUint8(v) => store_from_vec(&mut bits.store, v, 1),
            Value::VecInt16(v) => store_from_vec(&mut bits.store, v, 2),
            Value::VecUint16(v) => store_from_vec(&mut bits.store, v, 2),
            Value::VecInt32(v) => store_from_vec(&mut bits.store, v, 4),
            Value::VecUint32(v) => store_from_vec(&mut bits.store, v, 4),
            Value::VecFloat32(v) => store_from_vec(&mut bits.store, v, 4),
            Value::VecFloat64(v) => store_from_vec(&mut bits.store, v, 8),
            Value::VecInt64(v) => store_from_vec(&mut bits.store, v, 8),
            Value::VecUint64(v) => store_from_vec(&mut bits.store, v, 8),
            _ => {
                return None;
            }
        };
        Some(bits)
    }

    pub(super) fn pull(&mut self, bits: u8) -> u32 {
        let mask = (1u64 << bits) - 1;
        let val = (self.store[0] & mask) as u32;
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
        val
    }
}

trait AsU64 {
    fn as_u64(&self) -> u64;
}

macro_rules! impl_as_u64 {
    ($($type:ident),*) => {
        $(impl AsU64 for $type {
            fn as_u64(&self) -> u64 {
                *self as u64
            }
        })*
    };
}

impl_as_u64!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

fn store_from_vec<T: AsU64>(store: &mut [u64; N], v: &[T], size: u8) {
    let mut v = v;
    let mut pos = 0u8;
    let mut i = 0usize;
    while !v.is_empty() && i < N {
        store[i] |= (v[0].as_u64()) << (pos * 8);
        pos += size;
        if pos == 8 {
            i += 1;
            pos = 0;
        }
        v = &v[1..];
    }
}
