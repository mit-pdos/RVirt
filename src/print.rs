use core::fmt;
use spin::Mutex;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
pub mod uart {
    use core::ptr;

    const UART: *mut u8 = 0x10000000 as *mut u8;
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
}

// see: https://os.phil-opp.com/printing-to-screen
pub struct Writer { initialized: bool }
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
        WRITER.lock().write_fmt(format_args!($($arg)*)).unwrap();
    });
}
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}
