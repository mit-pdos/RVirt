
use crate::{csr, pmap, print, virtio};
use crate::plic::PlicState;
use crate::memory_region::{MemoryRegion, PageTableRegion};
use crate::trap::constants::*;
use crate::trap::U64Bits;

pub struct ControlRegisters {
    // sedeleg: u64, -- Hard-wired to zero
    // sideleg: u64, -- Hard-wired to zero

    pub sstatus: u64,
    pub sie: u64,
    pub sip: u64,
    pub stvec: u64,
    // scounteren: u64, -- Hard-wired to zero
    pub sscratch: u64,
    pub sepc: u64,
    pub scause: u64,
    pub stval: u64,
    pub satp: u64,

    pub mtimecmp: u64,
}

pub struct VirtIO {
    pub devices: [virtio::Device; virtio::MAX_DEVICES],
    pub queue_guest_pages: [u64; virtio::MAX_DEVICES * virtio::MAX_QUEUES],
    pub num_queue_guest_pages: usize,
}

pub struct Uart {
    pub dlab: bool,
    pub interrupt_enable: u8,
    pub interrupt_id: u8,
}

pub struct Context {
    pub csrs: ControlRegisters,
    pub plic: PlicState,
    pub uart: Uart,
    pub virtio: VirtIO,

    pub guest_memory: MemoryRegion,

    // Whether the guest is in S-Mode.
    pub smode: bool,

    // If set, hypervisor exits do not need to check for pending interrupts
    pub no_interrupt: bool,
}


impl ControlRegisters {
    pub fn push_sie(&mut self) {
        self.sstatus.set(STATUS_SPIE, self.sstatus.get(STATUS_SIE));
        self.sstatus.set(STATUS_SIE, false);
    }

    pub fn pop_sie(&mut self) {
        self.sstatus.set(STATUS_SIE, self.sstatus.get(STATUS_SPIE));
        self.sstatus.set(STATUS_SPIE, true);
    }
}

impl Uart {
    pub fn read(&mut self, addr: u64) -> u8 {
        match (self.dlab, addr) {
            (false, 0x10000000) => 0,
            (false, 0x10000001) => 0, // Interrupt enable (top four should always be zero)
            (_, 0x10000002) => { // Interrupt identification
                let r = 0xc0 | self.interrupt_id;
                self.interrupt_id = 0;
                r
            },
            (true, 0x10000003) => 0x03,
            (false, 0x10000003) => 0x83,
            (_, 0x10000005) => 0x30, // TODO: Change if data ready
            (_, 0x10000006) => 0x10, // Clear to send, other bits don't matter to Linux
            (dlab, _) => {
                println!("UART: Read uimplemented ?? <- {:#x} (dlab={})", addr, dlab);
                loop {}
            }
        }
    }
    pub fn write(&mut self, plic: &mut PlicState, addr: u64, value: u8) {
        match (self.dlab, addr, value) {
            (false, 0x10000000, _) => {
                print::guest_putchar(value);
                if self.interrupt_enable & 0x2 != 0 {
                    self.interrupt_id |= 0x2;
                    plic.set_pending(10, true);
                }
            }
            (true, 0x10000000, _) => {} // DLL divisor latch LSB
            (false, 0x10000001, _) => self.interrupt_enable = value,
            (true, 0x10000001, _) => {} // DLM divisor latch MSB
            (_, 0x10000002, _) => {} // FIFO control
            (_, 0x10000003, _) => self.dlab = (value & 0x80) != 0,
            (_, 0x10000004, _) if value & 0xf0 == 0 => {} // Modem control
            _ => {
                println!("UART: Write unimplemented {:#x} -> {:#x} (dlab={})",
                         value, addr, self.dlab);
                loop {}
            }
        }
    }
}

impl Context {
    pub const fn new() -> Self {
        Self {
            csrs: ControlRegisters{
                sstatus: 0,
                stvec: 0,
                sie: 0,
                sip: 0,
                sscratch: 0,
                sepc: 0,
                scause: 0,
                stval: 0,
                satp: 0,

                mtimecmp: u64::max_value(),
            },
            guest_memory: unsafe {  MemoryRegion::new(0, 0) },
            plic: PlicState::new(),
            uart: Uart {
                dlab: false,
                interrupt_enable: 0,
                interrupt_id: 0,
            },
            virtio: VirtIO {
                devices: [virtio::Device::new(); virtio::MAX_DEVICES],
                queue_guest_pages: [0; virtio::MAX_DEVICES * virtio::MAX_QUEUES],
                num_queue_guest_pages: 0,
            },
            smode: true,
            no_interrupt: true,
        }
    }

    pub fn get_csr(&mut self, csr: u32) -> Option<u64> {
        Some(match csr as u64 {
            csr::sstatus => {
                let real = csrr!(sstatus);
                self.csrs.sstatus = (self.csrs.sstatus & !SSTATUS_DYNAMIC_MASK) | (real & SSTATUS_DYNAMIC_MASK);
                self.csrs.sstatus
            }
            csr::satp => self.csrs.satp,
            csr::sie => self.csrs.sie,
            csr::stvec => self.csrs.stvec,
            csr::sscratch => self.csrs.sscratch,
            csr::sepc => self.csrs.sepc,
            csr::scause => self.csrs.scause,
            csr::stval => self.csrs.stval,
            csr::sip => self.csrs.sip,
            csr::sedeleg => 0,
            csr::sideleg => 0,
            csr::scounteren => 0,
            _ => return None,
        })
    }

    pub fn set_csr(&mut self, csr: u32, value: u64) -> bool {
        // println!("setting CSR={:#x} to {:#x} (pc={:#x})", csr, value, csrr!(sepc));
        match csr as u64 {
            csr::sstatus => {
                // User interrupts not supported
                let value = value & SSTATUS_WRITABLE_MASK;

                let changed = self.csrs.sstatus ^ value;
                self.csrs.sstatus = value;

                if changed & STATUS_MXR != 0 {
                    unimplemented!("STATUS.MXR");
                }
                if changed & STATUS_FS != 0 {
                    csrw!(sstatus, value & STATUS_FS | (csrr!(sstatus) & !STATUS_FS));
                }

                if changed.get(STATUS_SIE) && value.get(STATUS_SIE) {
                    // Enabling interrupts might cause one to happen right away.
                    self.no_interrupt = false;
                }
            }
            csr::satp => {
                let mode = (value & SATP_MODE) >> 60;
                if mode == 0 || mode == 8 {
                    self.csrs.satp = value & !SATP_ASID;
                } else {
                    println!("Attempted to install page table with unsupported mode");
                }
            }
            csr::sie => {
                let value = value & (IE_SEIE | IE_STIE | IE_SSIE);
                if !self.csrs.sie & value != 0 {
                    self.no_interrupt = false;
                }
                self.csrs.sie = value;
            }
            csr::stvec => self.csrs.stvec = value & !0x2,
            csr::sscratch => self.csrs.sscratch = value,
            csr::sepc => self.csrs.sepc = value,
            csr::scause => self.csrs.scause = value,
            csr::stval => self.csrs.stval = value,
            csr::sip => {
                if value & IP_SSIP != 0 {
                    self.no_interrupt = false;
                }
                self.csrs.sip = (self.csrs.sip & !IP_SSIP) | (value & IP_SSIP)
            }
            csr::sedeleg |
            csr::sideleg |
            csr::scounteren => {}
            _ => return false,
        }

        return true;
    }

    pub fn shadow(&self) -> pmap::PageTableRoot {
        if (self.csrs.satp & SATP_MODE) == 0 {
            pmap::MPA
        } else if !self.smode {
            pmap::UVA
        } else if self.csrs.sstatus & STATUS_SUM == 0 {
            pmap::KVA
        } else {
            pmap::MVA
        }
    }
}
