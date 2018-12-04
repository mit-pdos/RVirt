use core::fmt;

// see: https://os.phil-opp.com/printing-to-screen

pub struct Writer;
impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            crate::uart::putchar(byte);
        }
        Ok(())
    }
}

macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::print::Writer;
        Writer.write_fmt(format_args!($($arg)*)).unwrap();
    });
}
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}
