use core::{fmt, ptr};
use spin::Mutex;
use crate::fdt::UartType;
use crate::pmap;

// see https://github.com/riscv/riscv-pk/blob/master/machine/uart16550.c
// see: https://os.phil-opp.com/printing-to-screen

pub enum UartWriter {
    Ns16550a { initialized: bool, base_address: *mut u8 },
    SiFive { base_address: *mut u32 },
}
impl UartWriter {
    unsafe fn initialize_ns16550a(base_address: *mut u8) {
        ptr::write_volatile(base_address.offset(1), 0x00);
        ptr::write_volatile(base_address.offset(3), 0x80);
        ptr::write_volatile(base_address.offset(0), 0x03);
        ptr::write_volatile(base_address.offset(1), 0x00);
        ptr::write_volatile(base_address.offset(3), 0x03);
        ptr::write_volatile(base_address.offset(2), 0xC7);
    }

    pub fn putchar(&mut self, ch: u8) {
        unsafe {
            match *self {
                UartWriter::Ns16550a { ref mut initialized, base_address) => {
                    if !*initialized {
                        Self::initialize_ns16550a(base_address);
                        *initialized = true;
                    }

                    while ptr::read_volatile(base_address.offset(5)) & 0x20 == 0 {
                        // do nothing
                    }
                    ptr::write_volatile(base_address, ch)
                }
                UartWriter::SiFive { base_address } => {
                    while ptr::read_volatile(base_address) & 0x80000000 != 0 {
                        // do nothing
                    }
                    ptr::write_volatile(base_address, ch as u32)
                }
            }
        }
    }

    pub fn getchar(&mut self) -> Option<u8> {
        unsafe {
            match *self {
                UartWriter::Ns16550a { ref mut initialized, base_address } => {
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
                UartWriter::SiFive { base_address } => {
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

    pub unsafe fn init(&mut self, address: u64, ty: UartType) {
        if let UartWriter::Ns16550a { initialized: true, base_address } = *self {
            assert_eq!(address, base_address as u64);
            assert_eq!(ty, UartType::Ns16550a);
        } else {
            *self = match ty {
                UartType::Ns16550a => UartWriter::Ns16550a {
                    initialized: false,
                    base_address: address as *mut u8
                },
                UartType::SiFive => UartWriter::SiFive {
                    base_address: address as *mut u32
                },
            };
        }
    }

    pub unsafe fn switch_to_virtual_addresses(&mut self) {
        match *self {
            UartWriter::Ns16550a { ref mut base_address, .. } => {
                *base_address = pmap::pa2va(*base_address as u64) as *mut u8
            }
            UartWriter::SiFive { ref mut base_address, .. } => {
                *base_address = pmap::pa2va(*base_address as u64) as *mut u32
            }
        }
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
pub static UART_WRITER: Mutex<UartWriter> = Mutex::new(UartWriter::Ns16550a {
    initialized: false,
    base_address: 0x10000000 as *mut u8,
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

pub fn guest_putchar(c: u8) {
    UART_WRITER.lock().putchar(c);
}
