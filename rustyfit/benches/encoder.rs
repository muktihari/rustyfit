use std::io::Cursor;

use criterion::{Criterion, black_box};
use rustyfit::{Decoder, Encoder, EncoderBuilder, HeaderOption};

const TEST_FILE: &str = "tests/data/large.fit";

pub fn bench_encode(c: &mut Criterion) {
    let file_bytes = std::fs::read(TEST_FILE).unwrap();
    let mut dec = Decoder::new(Cursor::new(&file_bytes));
    let mut fit = dec.decode().unwrap();

    let mut buf = Vec::<u8>::with_capacity(10_000 >> 10); // 10 MB, large enough to avoid realloc.

    c.bench_function("encode default", |b| {
        b.iter(|| {
            let cur = Cursor::new(&mut buf);
            let mut enc = Encoder::new(black_box(cur));
            enc.encode(&mut fit).unwrap();
        })
    });

    c.bench_function("encode normal interleave 15", |b| {
        b.iter(|| {
            let cur = Cursor::new(&mut buf);
            let mut enc = EncoderBuilder::new(black_box(cur))
                .header_option(HeaderOption::Normal(15))
                .build();
            enc.encode(&mut fit).unwrap();
        })
    });

    c.bench_function("encode compress interleave 3", |b| {
        b.iter(|| {
            let cur = Cursor::new(&mut buf);
            let mut enc = EncoderBuilder::new(black_box(cur))
                .header_option(HeaderOption::Compressed(3))
                .build();
            enc.encode(&mut fit).unwrap();
        })
    });
}
