#![no_std]
#![no_main]

extern crate alloc;

use alloc::format;
use rustyfit::{
    Decoder, DecoderEvent, StreamingIterator,
    profile::{mesgdef, typedef},
};
use spinning_top::Spinlock;
use talc::{DefaultBinning, source::Claim, sync::TalcLock};

#[global_allocator]
static A: TalcLock<spinning_top::RawSpinlock, Claim, DefaultBinning> = TalcLock::new(unsafe {
    const SIZE: usize = 40 * 1024; // 40 KB
    static mut HEAP: [u8; SIZE] = [0; SIZE];
    Claim::array(&raw mut HEAP)
});

static FIT_FILE: &[u8] = include_bytes!("sample.fit"); // Embed file

static DECODER: Spinlock<Decoder> = Spinlock::new(Decoder::new());

macro_rules! printf {
    ($($arg:tt)*) => {
        let msg = format!($($arg)*);
        let ret = sys_write(1, msg.as_ptr(), msg.len());
        if ret != msg.len() {
            sys_exit(1);
        }
    }
}

#[unsafe(no_mangle)]
fn _start() -> ! {
    printf!("Hello, world!\n");

    let mut dec = DECODER.lock();
    let mut stream = dec.stream(FIT_FILE);

    while let Some(event) = stream.next() {
        if let DecoderEvent::Message(v) = event.unwrap() {
            if v.num == typedef::MesgNum::SESSION {
                let ses = mesgdef::Session::from(v);
                let sport = ses.sport;
                let total_distance = ses.total_distance_scaled() / 1000.0;
                printf!("> sport: {sport:}, total_distance: {total_distance:.2} km\n");
            }
        }
    }

    printf!("Goodbye, world!\n");
    sys_exit(0);

    // # Output:
    // Hello, world!
    // > sport: running, total_distance: 9.40 km
    // Goodbye, world!
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    printf!("{}\n", _info);
    loop {}
}

// https://man7.org/linux/man-pages/man2/write.2.html
// ```c
// ssize_t write(int fd, const void buf[.count], size_t count);
// ```
//
// ref: https://zenn.dev/zulinx86/articles/rust-nostd-101
fn sys_write(fd: i32, buf: *const u8, count: usize) -> usize {
    unsafe {
        let ret: usize;
        core::arch::asm!(
            "syscall",
            in("rax") 1,
            in("rdi") fd,
            in("rsi") buf,
            in("rdx") count,
            lateout("rax") ret,
            out("rcx") _,
            out("r11") _,
        );
        ret
    }
}

// https://man7.org/linux/man-pages/man3/exit.3.html
// ```c
// [[noreturn]] void exit(int status);
// ```
//
// ref: https://zenn.dev/zulinx86/articles/rust-nostd-101
fn sys_exit(status: i32) -> ! {
    unsafe {
        core::arch::asm!(
            "syscall",
            in("rax") 60,
            in("rdi") status,
            options(noreturn)
        );
    }
}
