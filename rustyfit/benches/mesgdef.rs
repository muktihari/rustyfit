use criterion::{Criterion, criterion_group, criterion_main};
use rustyfit::{
    profile::{ProfileType, mesgdef::Record, typedef::MesgNum},
    proto::{Field, Message, Value},
};
use std::hint::black_box;

pub fn bench_from(c: &mut Criterion) {
    const FIELD: Field = Field {
        num: 0,
        profile_type: ProfileType(0),
        value: Value::Invalid,
        is_expanded: false,
    };

    let mesg = Message {
        num: MesgNum::RECORD,
        fields: vec![
            Field {
                num: Record::DISTANCE,
                profile_type: ProfileType::UINT32,
                value: Value::Uint32(1000),
                is_expanded: false,
            },
            Field { num: 255, ..FIELD },
            Field { num: 255, ..FIELD },
            Field { num: 255, ..FIELD },
            Field { num: 255, ..FIELD },
            Field { num: 255, ..FIELD },
        ],
        ..Default::default()
    };

    c.bench_function("mesgdef From<&Message>", |b| {
        b.iter(|| {
            let _ = Record::from(black_box(&mesg));
        })
    });
}

pub fn bench_new(c: &mut Criterion) {
    c.bench_function("new", |b| {
        b.iter(|| {
            let _ = Record::new();
        })
    });
}

criterion_group!(benches, bench_from, bench_new);
criterion_main!(benches);
