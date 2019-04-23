
use crate::print;

// never returns but we can't convince rust of that, because unreachable_unchecked requires abort
#[link_section = ".text.init"]
#[no_mangle]
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
#[no_mangle]
pub fn machine_debug_assert(cond: bool, msg: &str) {
    if !cond {
        machine_debug_abort(msg);
    }
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_puts(s: &str) {
    let mut sreal = s;
    if s.as_ptr() as u64 >= 0xffffffff40000000 {
        unsafe {
            sreal = core::str::from_utf8_unchecked(core::slice::from_raw_parts(((s.as_ptr() as u64) - 0xffffffff40000000) as *const u8, s.len()));
        }
    }
    if let Some(mut writer) = print::mwriter() {
        for byte in sreal.bytes() {
            writer.mputchar(byte);
        }
    }
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_puthex64(v: u64) {
    machine_debug_puts("0x");
    if let Some(mut writer) = print::mwriter() {
        for i in 0 .. 16 {
            let digit = ((v >> (60 - i * 4)) & 0xF) as u8;
            if digit >= 0xA {
                writer.mputchar('A' as u8 + digit - 10);
            } else {
                writer.mputchar('0' as u8 + digit);
            }
        }
    }
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_putint_recur(writer: &mut print::UartWriter, v: u64) {
    if v > 0 {
        let digit = (v % 10) as u8;
        machine_debug_putint_recur(writer, v / 10);
        writer.mputchar('0' as u8 + digit);
    }
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_putint(v: u64) {
    if let Some(mut writer) = print::mwriter() {
        if v == 0 {
            writer.mputchar('0' as u8);
        } else {
            machine_debug_putint_recur(&mut writer, v);
        }
    }
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_mark_begin() {
    machine_debug_puts("\u{1b}[31m");
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_mark_end() {
    machine_debug_puts("\u{1b}[0m");
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_newline() {
    machine_debug_puts("\r\n");
}

#[link_section = ".text.init"]
#[no_mangle]
pub fn machine_debug_dump_state() {
    // for _ in 0..1000 {
    //     unsafe { asm!("nop" :::: "volatile"); }
    // }
    // unsafe {
    //     crate::print::UART_WRITER.force_unlock();
    // }
    machine_debug_puts("Dumping state...\r\n");
    machine_debug_puts("mepc=");
    machine_debug_puthex64(csrr!(mepc));
    machine_debug_newline();

    machine_debug_puts("mcause=");
    machine_debug_puthex64(csrr!(mcause));
    machine_debug_newline();

    machine_debug_puts("mtval=");
    machine_debug_puthex64(csrr!(mtval));
    machine_debug_newline();

    machine_debug_puts("mstatus=");
    machine_debug_puthex64(csrr!(mstatus));
    machine_debug_newline();

    //crate::pmp::debug_pmp();
    machine_debug_abort("Done.");
}

