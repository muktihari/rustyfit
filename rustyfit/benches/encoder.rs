use std::{hint::black_box, io::Cursor};

use criterion::{Criterion, criterion_group, criterion_main};
use rustyfit::{Decoder, Encoder, EncoderBuilder, HeaderOption};

const TEST_FILE: &str = "tests/data/large.fit";

pub fn bench_encode(c: &mut Criterion) {
    let file_bytes = std::fs::read(TEST_FILE).unwrap();
    let mut dec = Decoder::new(Cursor::new(&file_bytes));
    let mut buf = Vec::<u8>::with_capacity(10_000 >> 10); // 10 MB, large enough to avoid realloc.

    while let Some(fit) = &mut dec.decode().unwrap() {
        c.bench_function("encode default", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = Encoder::new(black_box(cur));
                enc.encode(fit).unwrap();
                buf.clear();
            })
        });

        c.bench_function("encode normal interleave 15", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = EncoderBuilder::new(black_box(cur))
                    .header_option(HeaderOption::Normal(15))
                    .build();
                enc.encode(fit).unwrap();
                buf.clear();
            })
        });

        c.bench_function("encode compress interleave 3", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = EncoderBuilder::new(black_box(cur))
                    .header_option(HeaderOption::Compressed(3))
                    .build();
                enc.encode(fit).unwrap();
                buf.clear();
            })
        });
    }
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
