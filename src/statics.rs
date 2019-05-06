
use core::sync::atomic::AtomicBool;
use spin::Mutex;
use crate::print::{self, UartWriter};
use crate::constants::*;

#[derive(Copy, Clone, Debug)]
pub enum IpiReason {
    EnterSupervisor {
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        sp: u64,
        satp: u64,
        mepc: u64,
    }
}

#[repr(C,align(4096))]
pub struct Shared {
    pub boot_page_table: [u64; 1024],
    pub uart_writer: Mutex<UartWriter>,
    pub ipi_reason_array: [Mutex<Option<IpiReason>>; MAX_HOST_HARTS],
    pub hart_lottery: AtomicBool,
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



const MR: Mutex<Option<IpiReason>> = Mutex::new(None);

/// This static is never accessed directly, but is needed so that the memory backing SHARED_STATICS
/// is properly initialized.
///
/// We hard code an address for the UART. This value will be replaced once the device tree has been
/// parsed, but until then this provides a way to debug early boot issues. Once the memory subsystem
/// is initialized, this will again be updated to use virtual addresses instead of physical
/// addresses.
#[link_section = ".shared.data"]
pub static __SHARED_STATICS_IMPL: Shared = Shared {
    // see also: print::early_guess_uart
    uart_writer: Mutex::new(UartWriter {
        pa: 0x10010000,
        va: None,
        inner: print::UartWriterInner::SiFive,
    }),
    ipi_reason_array: [MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR,],
    boot_page_table: [0; 1024],
    hart_lottery: AtomicBool::new(true),
};
