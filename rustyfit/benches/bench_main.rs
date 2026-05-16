mod decoder;
mod encoder;
mod mesgdef;

use criterion::{Criterion, criterion_group, criterion_main};

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = decoder::bench_decode, encoder::bench_encode,  mesgdef::bench_from, mesgdef::bench_new
}

criterion_main!(benches);
