use core::{fmt, ptr};
use spin::{Mutex, MutexGuard};
use crate::fdt::UartType;
use crate::pmap;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
// see: https://os.phil-opp.com/printing-to-screen

enum UartWriterInner {
    Ns16550a { initialized: bool },
    SiFive,
}

pub struct UartWriter {
    pa: u64,
    va: Option<u64>,
    inner: UartWriterInner,
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

/// Hard code an address for the UART. This value will be replaced once the device tree has been
/// parsed, but until then this provides a way to debug early boot issues. Once the memory subsystem
/// is initialized, this will again be updated to use virtual addresses instead of physical addresses.
#[link_section = ".shared.data"]
pub static UART_WRITER: Mutex<UartWriter> = Mutex::new(UartWriter {
    pa: 0x10000000,
    va: None,
    inner: UartWriterInner::Ns16550a { initialized: false },
});

macro_rules! print {
    ($($arg:tt)*) => ({
        use core::fmt::Write;
        use crate::print::UART_WRITER;
        let mut writer = UART_WRITER.lock();
        writer.write_str("\u{1b}[33m").unwrap();
        writer.write_fmt(format_args!($($arg)*)).unwrap();
        writer.write_str("\u{1b}[0m").unwrap();
    });
}
macro_rules! println {
    ($fmt:expr) => (print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => (print!(concat!($fmt, "\n"), $($arg)*));
}

pub fn guest_println(hartid: u64, line: &[u8]) {
    use core::fmt::Write;
    use crate::print::UART_WRITER;
    let mut writer = UART_WRITER.lock();
    match hartid {
        1 => writer.write_str("\u{1b}[32m").unwrap(),
        2 => writer.write_str("\u{1b}[34m").unwrap(),
        _ => writer.write_str("\u{1b}[33m").unwrap(),
    }
    writer.write_str("\u{1b}[1m").unwrap();
    writer.write_fmt(format_args!("[{}] ", hartid)).unwrap();
    writer.write_str("\u{1b}[0m").unwrap();
    for &b in line {
        writer.putchar(b);
    }
    writer.write_str("\n").unwrap();
}

#[link_section = ".text.init"]
pub fn mwriter<'a>() -> Option<MutexGuard<'a, UartWriter>> {
    let writer_ptr = &UART_WRITER as *const _ as u64;
    let writer_ptr = writer_ptr - 0xffffffff40000000;
    unsafe { (*(writer_ptr as *const Mutex<UartWriter>)).try_lock() }
}
