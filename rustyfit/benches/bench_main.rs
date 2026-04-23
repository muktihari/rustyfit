use criterion::{Criterion, criterion_group, criterion_main};

mod decoder;
mod encoder;
mod mesgdef;

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = decoder::bench_decode, encoder::bench_encode, mesgdef::bench_from
}

criterion_main!(benches);
