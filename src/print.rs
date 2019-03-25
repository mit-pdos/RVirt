use core::fmt;
use spin::Mutex;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
pub mod uart {
    use core::ptr;

    pub static mut UART: *mut u8 = 0x10000000 as *mut u8;
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

    pub fn getchar() -> Option<u8> {
        unsafe {
            if ptr::read_volatile(UART.offset(5)) & 0x01 != 0 {
                Some(ptr::read_volatile(UART))
            } else {
                None
            }
        }
    }
}

// see: https://os.phil-opp.com/printing-to-screen
pub struct Writer { initialized: bool }
impl Writer {
    fn putchar(&mut self, c: u8) {
        if !self.initialized {
            uart::enable();
            self.initialized = true;
        }
        uart::putchar(c);
    }
}
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if !self.initialized {
            uart::enable();
            self.initialized = true;
        }
        for byte in s.bytes() {
            uart::putchar(byte);
        }
        Ok(())
    }
}

pub static WRITER: Mutex<Writer> = Mutex::new(Writer {initialized: false});

macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::print::WRITER;
        let mut writer = WRITER.lock();
        writer.write_str("\u{1b}[33m").unwrap();
        writer.write_fmt(format_args!($($arg)*)).unwrap();
        writer.write_str("\u{1b}[0m").unwrap();
    });
}
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

pub fn guest_putchar(c: u8) {
    WRITER.lock().putchar(c);
}
