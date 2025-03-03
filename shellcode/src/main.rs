#![no_std]
#![no_main]

use core::arch::asm;
use core::ffi::CStr;

unsafe fn syscall1(syscall: usize, arg1: usize) -> usize {
    let ret: usize;
    asm!(
    "syscall",
    in("x16") syscall,
    in("x0") arg1,
    lateout("x0") ret,
    lateout("x1") _,
    options(nostack),
    );
    ret
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

unsafe fn syscall3(syscall: usize, arg1: usize, arg2: usize, arg3: usize) -> usize {
    let ret: usize;
    asm!(
    "syscall",
    in("x16") syscall,
    in("x0") arg1,
    in("x1") arg2,
    in("x2") arg3,
    lateout("x0") ret,
    lateout("x1") _,
    options(nostack),
    );
    ret
}

const SYS_WRITE: usize = 4;
const SYS_EXIT: usize = 1;
const STDOUT: usize = 1;
static MESSAGE: &CStr = c"hello world";

pub extern "C" fn _start() -> () {
    unsafe {
        syscall3(
            SYS_WRITE,
            STDOUT,
            MESSAGE.as_ptr() as usize,
            MESSAGE.count_bytes(),
        );

        syscall1(SYS_EXIT, 0)
    };
}
