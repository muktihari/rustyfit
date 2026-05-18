use criterion::{Criterion, criterion_group, criterion_main};
use embedded_io_adapters::std::FromStd;
use rustyfit::{Decoder, DecoderBuilder};
use std::{hint::black_box, io::Cursor};

const TEST_FILE: &str = "tests/data/large.fit";

pub fn bench_decode(c: &mut Criterion) {
    let file_bytes = std::fs::read(TEST_FILE).unwrap();

    c.bench_function("decode default", |b| {
        b.iter(|| {
            let mut dec = Decoder::new(black_box(FromStd::new(Cursor::new(&file_bytes))));
            dec.decode().unwrap();
        })
    });

    c.bench_function("decode no checksum no expand", |b| {
        b.iter(|| {
            let mut dec = DecoderBuilder::new(black_box(FromStd::new(Cursor::new(&file_bytes))))
                .checksum(false)
                .expand_components(false)
                .build();
            dec.decode().unwrap();
        })
    });

    c.bench_function("decode_with", |b| {
        b.iter(|| {
            let mut dec = Decoder::new(black_box(FromStd::new(Cursor::new(&file_bytes))));
            dec.decode_with(|_| {}).unwrap();
        })
    });

    c.bench_function("decode_with no checksum no expand", |b| {
        b.iter(|| {
            let mut dec = DecoderBuilder::new(black_box(FromStd::new(Cursor::new(&file_bytes))))
                .checksum(false)
                .expand_components(false)
                .build();
            dec.decode_with(|_| {}).unwrap();
        })
    });
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
