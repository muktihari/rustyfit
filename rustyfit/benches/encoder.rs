use criterion::{Criterion, criterion_group, criterion_main};
use embedded_io_adapters::std::FromStd;
use rustyfit::{Decoder, Encoder, HeaderOption};
use std::{hint::black_box, io::Cursor, sync::Mutex};

const TEST_FILE: &str = "tests/data/large.fit";

static ENCODER: Mutex<Encoder> = Mutex::new(Encoder::new());

pub fn bench_encode(c: &mut Criterion) {
    let mut dec = Decoder::new();
    let file_bytes = std::fs::read(TEST_FILE).unwrap();
    let mut reader = FromStd::new(Cursor::new(&file_bytes));

    let mut buf = Vec::<u8>::with_capacity(5000 * 1024); // 5 MB, large enough to avoid realloc.

    while let Some(fit) = &mut dec.decode(&mut reader).unwrap() {
        c.bench_function("encode default", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = Encoder::new();
                let mut writer = black_box(FromStd::new(cur));
                enc.encode(&mut writer, fit).unwrap();
                buf.clear();
            })
        });

        c.bench_function("encode default - static", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = ENCODER.lock().unwrap();
                let mut writer = black_box(FromStd::new(cur));
                enc.encode(&mut writer, fit).unwrap();
                buf.clear();
            })
        });

        c.bench_function("encode normal interleave 15", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = Encoder::builder()
                    .header_option(HeaderOption::Normal(15))
                    .build();
                let mut writer = black_box(FromStd::new(cur));
                enc.encode(&mut writer, fit).unwrap();
                buf.clear();
            })
        });

        c.bench_function("encode compress interleave 3", |b| {
            b.iter(|| {
                let cur = Cursor::new(&mut buf);
                let mut enc = Encoder::builder()
                    .header_option(HeaderOption::Compressed(3))
                    .build();
                let mut writer = black_box(FromStd::new(cur));
                enc.encode(&mut writer, fit).unwrap();
                buf.clear();
            })
        });
    }
}

criterion_group!(benches, bench_encode);
criterion_main!(benches);
