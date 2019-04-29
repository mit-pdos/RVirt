use core::{fmt, ptr};
use spin::MutexGuard;
use crate::statics::SHARED_STATICS;
use crate::fdt::UartType;
use crate::pmap;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
// see: https://os.phil-opp.com/printing-to-screen

pub enum UartWriterInner {
    Ns16550a { initialized: bool },
    SiFive,
}

pub struct UartWriter {
    pub pa: u64,
    pub va: Option<u64>,
    pub inner: UartWriterInner,
}

impl UartWriterInner {
    #[inline(always)]
    unsafe fn initialize_ns16550a(base_address: *mut u8) {
        ptr::write_volatile(base_address.offset(1), 0x00);
        ptr::write_volatile(base_address.offset(3), 0x80);
        ptr::write_volatile(base_address.offset(0), 0x03);
        ptr::write_volatile(base_address.offset(1), 0x00);
        ptr::write_volatile(base_address.offset(3), 0x03);
        ptr::write_volatile(base_address.offset(2), 0xC7);
    }

    #[inline(always)]
    fn putchar(&mut self, base_address: u64, ch: u8) {
        unsafe {
            match *self {
                UartWriterInner::Ns16550a { ref mut initialized } => {
                    let base_address = base_address as *mut u8;
                    if !*initialized {
                        Self::initialize_ns16550a(base_address);
                        *initialized = true;
                    }

                    while ptr::read_volatile(base_address.offset(5)) & 0x20 == 0 {
                        // do nothing
                    }
                    ptr::write_volatile(base_address, ch)
                }
                UartWriterInner::SiFive => {
                    let base_address = base_address as *mut u32;
                    while ptr::read_volatile(base_address) & 0x80000000 != 0 {
                        // do nothing
                    }
                    ptr::write_volatile(base_address, ch as u32)
                }
            }
        }
    }

    #[inline(always)]
    fn getchar(&mut self, base_address: u64) -> Option<u8> {
        unsafe {
            match *self {
                UartWriterInner::Ns16550a { ref mut initialized } => {
                    let base_address = base_address as *mut u8;
                    if !*initialized {
                        Self::initialize_ns16550a(base_address);
                        *initialized = true;
                    }

                    if ptr::read_volatile(base_address.offset(5)) & 0x01 != 0 {
                        Some(ptr::read_volatile(base_address))
                    } else {
                        None
                    }
                }
                UartWriterInner::SiFive => {
                    let base_address = base_address as *mut u32;
                    let rxdata = ptr::read_volatile(base_address);
                    if rxdata & 0x80000000 != 0 {
                        Some(rxdata as u8)
                    } else {
                        None
                    }
                }
            }
        }
    }
}
impl UartWriter {
    pub fn putchar(&mut self, ch: u8) {
        self.inner.putchar(self.va.unwrap_or(self.pa), ch);
    }

    #[link_section = ".text.init"]
    pub fn mputchar(&mut self, ch: u8) {
        self.inner.putchar(self.pa, ch);
    }

    pub fn getchar(&mut self) -> Option<u8> {
        self.inner.getchar(self.va.unwrap_or(self.pa))
    }

    pub unsafe fn init(&mut self, address: u64, ty: UartType) {
        if let UartWriterInner::Ns16550a { initialized: true } = self.inner {
            assert_eq!(self.pa, address);
            assert_eq!(ty, UartType::Ns16550a);
        } else {
            self.inner = match ty {
                UartType::Ns16550a => UartWriterInner::Ns16550a {
                    initialized: false,
                },
                UartType::SiFive => UartWriterInner::SiFive,
            };
            self.pa = address;
            assert_eq!(self.va, None);
        }
    }

    pub unsafe fn switch_to_virtual_addresses(&mut self) {
        self.va = Some(pmap::pa2va(self.pa));
    }
}
impl fmt::Write for UartWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.putchar(byte);
        }
        Ok(())
    }
}
unsafe impl Send for UartWriter {}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::SHARED_STATICS;
        let mut writer = SHARED_STATICS.uart_writer.lock();
        writer.write_str("\u{1b}[33m").unwrap();
        writer.write_fmt(format_args!($($arg)*)).unwrap();
        writer.write_str("\u{1b}[0m").unwrap();
    });
}
#[macro_export]
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

pub fn guest_println(guestid: u64, line: &[u8]) {
    use core::fmt::Write;
    use crate::SHARED_STATICS;
    let mut writer = SHARED_STATICS.uart_writer.lock();
    match guestid {
        1 => writer.write_str("\u{1b}[32m").unwrap(),
        2 => writer.write_str("\u{1b}[34m").unwrap(),
        _ => writer.write_str("\u{1b}[33m").unwrap(),
    }
    writer.write_str("\u{1b}[1m").unwrap();
    writer.write_fmt(format_args!("[{}] ", guestid)).unwrap();
    writer.write_str("\u{1b}[0m").unwrap();
    for &b in line {
        writer.putchar(b);
    }
    writer.write_str("\n").unwrap();
}

#[link_section = ".text.init"]
pub fn mwriter<'a>() -> Option<MutexGuard<'a, UartWriter>> {
    SHARED_STATICS.uart_writer.try_lock()
}
