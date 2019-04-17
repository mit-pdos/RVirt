
use core::ptr;

// NOTE: you need to call machine_debug_init first if you want to use any of this
// TODO: remove that requirement

// this might re-init the ns16550a with respect to print.rs's initialization. I hope it's okay.
#[link_section = ".text.init"]
pub fn machine_debug_init() {
    unsafe {
        ptr::write_volatile(BASE_ADDRESS.offset(1), 0x00);
        ptr::write_volatile(BASE_ADDRESS.offset(3), 0x80);
        ptr::write_volatile(BASE_ADDRESS.offset(0), 0x03);
        ptr::write_volatile(BASE_ADDRESS.offset(1), 0x00);
        ptr::write_volatile(BASE_ADDRESS.offset(3), 0x03);
        ptr::write_volatile(BASE_ADDRESS.offset(2), 0xC7);
    }
}

// never returns but we can't convince rust of that, because unreachable_unchecked requires abort
#[link_section = ".text.init"]
pub fn machine_debug_abort(msg: &str) {
    machine_debug_mark_begin();
    machine_debug_puts(msg);
    machine_debug_puts("\n");
    machine_debug_mark_end();
    unsafe {
        asm!("1: j 1b\n");
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_assert(cond: bool, msg: &str) {
    if !cond {
        machine_debug_abort(msg);
    }
}

const BASE_ADDRESS: *mut u8 = 0x10000000 as *mut u8;

// assumes ns16550a
#[link_section = ".text.init"]
pub fn machine_debug_putchar(ch: u8) {
    unsafe {
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        loop {
            let result = ptr::read_volatile(BASE_ADDRESS.offset(5));
            if result & 0x20 == 0x20 {
                break
            }
        }
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
        ptr::write_volatile(BASE_ADDRESS, ch);
        core::sync::atomic::compiler_fence(core::sync::atomic::Ordering::SeqCst);
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_puts(s: &str) {
    let mut sreal = s;
    if s.as_ptr() as u64 >= 0xffffffff40000000 {
        unsafe {
            sreal = core::str::from_utf8_unchecked(core::slice::from_raw_parts(((s.as_ptr() as u64) - 0xffffffff40000000) as *const u8, s.len()));
        }
    }
    for byte in sreal.bytes() {
        machine_debug_putchar(byte);
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_puthex64(v: u64) {
    machine_debug_puts("0x");
    for i in 0 .. 16 {
        let digit = ((v >> (60 - i * 4)) & 0xF) as u8;
        if digit >= 0xA {
            machine_debug_putchar('A' as u8 + digit - 10);
        } else {
            machine_debug_putchar('0' as u8 + digit);
        }
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_putint_recur(v: u64) {
    if v > 0 {
        let digit = (v % 10) as u8;
        machine_debug_putint_recur(v / 10);
        machine_debug_putchar('0' as u8 + digit);
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_putint(v: u64) {
    if v == 0 {
        machine_debug_putchar('0' as u8);
    } else {
        machine_debug_putint_recur(v);
    }
}

#[link_section = ".text.init"]
pub fn machine_debug_mark_begin() {
    machine_debug_puts("\u{1b}[31m");
}

#[link_section = ".text.init"]
pub fn machine_debug_mark_end() {
    machine_debug_puts("\u{1b}[0m");
}
