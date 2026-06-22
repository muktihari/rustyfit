use criterion::{Criterion, criterion_group, criterion_main};
use rustyfit::{
    profile::{
        mesgdef::Record,
        typedef::{FitBaseType, MesgNum},
    },
    proto::{Field, Message, Value},
};
use std::hint::black_box;

pub fn bench_from(c: &mut Criterion) {
    const FIELD: Field = Field {
        num: 0,
        base_type: FitBaseType(u8::MAX),
        value: Value::Invalid,
        is_expanded: false,
    };

    let mesg = Message {
        num: MesgNum::RECORD,
        fields: vec![
            Field {
                num: Record::DISTANCE,
                base_type: FitBaseType::UINT32,
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

    c.bench_function("mesgdef From<Record>", |b| {
        b.iter(|| {
            let mut record = Record::new();
            record.distance = 1000;
            record.speed = 1000;
            record.power = 1000;
            record.temperature = 29;
            record.position_lat = 0;
            record.position_long = 0;
            record.altitude = 6000;

            let mesg = Message::from(record);
            assert_eq!(mesg.fields.len(), 7);
            assert_eq!(mesg.fields.capacity(), 7);
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
