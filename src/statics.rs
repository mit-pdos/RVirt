use arr_macro::arr;
use core::sync::atomic::AtomicBool;
use spin::Mutex;
use crate::constants::*;
use crate::print::{self, UartWriter};
use crate::drivers::macb::MacbDriver;
use crate::pmap;

#[derive(Copy, Clone, Debug)]
pub enum IpiReason {
    TriggerHartEntry {
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        a4: u64,
        sp: u64,
        satp: u64,
    }
}

#[repr(C,align(4096))]
pub struct Shared {
    pub boot_page_tables: [[u64; 1024]; MAX_HOST_HARTS],
    pub ipi_reason_array: [Mutex<Option<IpiReason>>; MAX_HOST_HARTS],
    pub uart_writer: Mutex<UartWriter>,
    pub hart_lottery: AtomicBool,
    pub net: Mutex<Option<MacbDriver>>,
}

pub struct ConditionalPointer(u64);


#[cfg(feature = "physical_symbol_addresses")]
 pub const SHARED_STATICS: ConditionalPointer = ConditionalPointer(MACHINE_SHARED_STATIC_ADDRESS);
#[cfg(not(feature = "physical_symbol_addresses"))]
pub const SHARED_STATICS: ConditionalPointer = ConditionalPointer(SUPERVISOR_SHARED_STATIC_ADDRESS);

impl core::ops::Deref for ConditionalPointer {
    type Target = Shared;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.0 as *const Shared) }
    }
}

const fn make_boot_page_tables_array() -> [[u64; 1024]; MAX_HOST_HARTS] {
    const BASE: u64 = SUPERVISOR_SHARED_STATIC_ADDRESS - SYMBOL_PA2VA_OFFSET;
    const STRIDE: u64 = 1024 * 8;

    let mut i = 0;
    arr![pmap::make_boot_page_table({i += 1; BASE + (i - 1) * STRIDE}); 16]
}

/// This static is never accessed directly, but is needed so that the memory backing SHARED_STATICS
/// is properly initialized.
///
/// We hard code an address for the UART. This value will be replaced once the device tree has been
/// parsed, but until then this provides a way to debug early boot issues.
#[link_section = ".shared.data"]
pub static __SHARED_STATICS_IMPL: Shared = Shared {
    boot_page_tables: make_boot_page_tables_array(),
    ipi_reason_array: arr![Mutex::new(None); 16],
    // see also: print::early_guess_uart
    uart_writer: Mutex::new(UartWriter {
        pa: 0x10000000,
        inner: print::UartWriterInner::Ns16550a { initialized: false },
    }),
    hart_lottery: AtomicBool::new(true),
    net: Mutex::new(None),
};
