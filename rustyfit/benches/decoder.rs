use criterion::{Criterion, criterion_group, criterion_main};
use embedded_io_adapters::std::FromStd;
use rustyfit::{Decoder, StreamingIterator};
use std::{hint::black_box, io::Cursor, sync::Mutex};

const TEST_FILE: &str = "tests/data/large.fit";

static DECODER: Mutex<Decoder> = Mutex::new(Decoder::new());

pub fn bench_decode(c: &mut Criterion) {
    let file_bytes = std::fs::read(TEST_FILE).unwrap();

    c.bench_function("decode default", |b| {
        b.iter(|| {
            let mut dec = Decoder::new();
            let mut reader = black_box(FromStd::new(Cursor::new(&file_bytes)));
            dec.decode(&mut reader).unwrap();
        })
    });

    c.bench_function("decode default - static", |b| {
        b.iter(|| {
            let mut dec = DECODER.lock().unwrap();
            let mut reader = black_box(FromStd::new(Cursor::new(&file_bytes)));
            dec.decode(&mut reader).unwrap();
        })
    });

    c.bench_function("decode no checksum no expand", |b| {
        b.iter(|| {
            let mut dec = Decoder::builder()
                .checksum(false)
                .expand_components(false)
                .build();
            let mut reader = black_box(FromStd::new(Cursor::new(&file_bytes)));
            dec.decode(&mut reader).unwrap();
        })
    });

    c.bench_function("decode stream", |b| {
        b.iter(|| {
            let mut dec = Decoder::new();
            let mut reader = black_box(FromStd::new(Cursor::new(&file_bytes)));
            let mut stream = dec.stream(&mut reader);
            while let Some(event) = stream.next() {
                event.unwrap();
            }
        })
    });

    c.bench_function("decode stream no checksum no expand", |b| {
        b.iter(|| {
            let mut dec = Decoder::builder()
                .checksum(false)
                .expand_components(false)
                .build();
            let mut reader = black_box(FromStd::new(Cursor::new(&file_bytes)));
            let mut stream = dec.stream(&mut reader);
            while let Some(event) = stream.next() {
                event.unwrap();
            }
        })
    });
}

criterion_group!(benches, bench_decode);
criterion_main!(benches);
