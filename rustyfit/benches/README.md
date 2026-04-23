## How to compare benchmarks

cargo bench --bench mesgdef -- --save-baseline old      # benchmark and save this as the baseline named "old"
cargo bench --bench mesgdef -- --baseline old           # benchmark and compare it with "old" baseline
