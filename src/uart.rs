use core::ptr;

const UART: *mut u8 = 0x10000000 as *mut u8;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
pub fn enable() {
    unsafe { ptr::write_volatile(UART.offset(1), 0x00) }
    unsafe { ptr::write_volatile(UART.offset(3), 0x80) }
    unsafe { ptr::write_volatile(UART.offset(0), 0x03) }
    unsafe { ptr::write_volatile(UART.offset(1), 0x00) }
    unsafe { ptr::write_volatile(UART.offset(3), 0x03) }
    unsafe { ptr::write_volatile(UART.offset(2), 0xC7) }
}

pub fn putchar(ch: u8) {
    unsafe {
        while ptr::read_volatile(UART.offset(5)) & 0x20 == 0 {
            // do nothing
        }

        ptr::write_volatile(UART, ch)
    }
}
