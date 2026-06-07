A sampe of #![no_std] program to decode a FIT file then print it into `stdout` on x86_64 linux machine.

To run this program:
```bash
# Go to this directory because I put `.cargo/config.toml` local to this program only
cd examples/no_std_decode

# Run the program
cargo run
```

Please note, this program generally works, but it only serves as a `Proof-of-Concept`. In this program, 
we use pure spinlock as a mutex which is considered harmful on multithreaded OS or on baremetal, while 
it's fine in WASM since it's a single-threaded without interrupts from peripheral devices.
Ref: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html

For baremetal, please use `critical section`'s Mutex instead (https://docs.rs/critical-section/1.2.0/critical_section/struct.Mutex.html) with a crate supplying its implementation (e.g. https://docs.rs/crate/rp2040-hal/0.12.0/source/src/critical_section_impl.rs).
Also use Heap Allocator which uses `critical section` as well, e.g. https://crates.io/crates/embedded-alloc
