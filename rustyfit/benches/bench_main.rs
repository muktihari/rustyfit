use criterion::{Criterion, criterion_group, criterion_main};

mod decoder;
mod encoder;

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = decoder::bench_decode, encoder::bench_encode,
}

criterion_main!(benches);
