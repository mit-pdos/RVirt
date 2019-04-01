use spin::Mutex;
use crate::fdt::MachineMeta;
use crate::plic::PlicState;
use crate::pmap::{PageTables, PageTableRoot};
use crate::memory_region::MemoryRegion;
use crate::trap::constants::*;
use crate::trap::U64Bits;
use crate::{csr, pmap, print, trap, virtio};

pub static CONTEXT: Mutex<Option<Context>> = Mutex::new(None);

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

    pub divisor_latch: u16,
    pub interrupt_enable: u8,

    pub next_interrupt_time: u64,

    pub input_fifo: [u8; 16],
    pub input_bytes_ready: usize,
    // For some reason Linux ignores every other byte read from the UART? This field tracks whether
    // the next read will be ignored (if so we return zero instead of the real character).
    pub read_zero: bool,
}

pub struct Context {
    pub csrs: ControlRegisters,
    pub plic: PlicState,
    pub uart: Uart,
    pub virtio: VirtIO,

    pub saved_registers: MemoryRegion,
    pub guest_memory: MemoryRegion,
    pub shadow_page_tables: PageTables,

    pub guest_shift: u64,

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
    fn tx_interrupt(&self, current_time: u64) -> bool {
        self.next_interrupt_time  <= current_time && self.interrupt_enable & 0x2 != 0
    }
    fn rx_interrupt(&self) -> bool {
        self.input_bytes_ready >= 1 && self.interrupt_enable & 0x1 != 0
    }
    pub fn timer(state: &mut Context, current_time: u64) {
        state.uart.fill_fifo();
        if state.uart.tx_interrupt(current_time) || state.uart.rx_interrupt() {
            state.plic.set_pending(10, true);
            state.no_interrupt = false;
        }
    }

    pub fn fill_fifo(&mut self) {
        while self.input_bytes_ready < self.input_fifo.len() {
            if let Some(ch) = print::uart::getchar() {
                self.input_fifo[self.input_bytes_ready] = ch;
                self.input_bytes_ready += 1;
            } else {
                break;
            }
        }
    }

    pub fn read(&mut self, addr: u64) -> u8 {
        match (self.dlab, addr) {
            (false, 0x10000000) => {
                if self.input_bytes_ready > 0 {
                    if self.read_zero {
                        self.read_zero = false;
                        return 0;
                    }
                    self.read_zero = true;

                    let ret = self.input_fifo[0];
                    self.input_bytes_ready -= 1;
                    for i in 0..(self.input_bytes_ready) {
                        self.input_fifo[i] = self.input_fifo[i+1];
                    }
                    // println!("output = {}", ret as char);
                    ret
                } else {
                    0
                }
            }
            (false, 0x10000001) => self.interrupt_enable, // Interrupt enable (top four should always be zero)
            (_, 0x10000002) => { // Interrupt identification
                if self.rx_interrupt() {
                    0xc4
                } else if self.tx_interrupt(trap::get_mtime()) {
                    0xc2
                } else {
                    0xc1
                }
            },
            (true, 0x10000003) => 0x03,
            (false, 0x10000003) => 0x83,
            (_, 0x10000005) => {
                self.fill_fifo();
                let input_mask = if self.input_bytes_ready > 0 { 0x1 } else { 0x0 };
                if trap::get_mtime() >= self.next_interrupt_time {
                    0x30 | input_mask
                } else {
                    0x0 | input_mask
                }
            }
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

                let current_time = trap::get_mtime();
                let transmit_time = self.divisor_latch as u64 * 5;
                self.next_interrupt_time =
                    self.next_interrupt_time.max(current_time) + transmit_time;
            }
            (false, 0x10000001, _) => {
                let delta = value ^ self.interrupt_enable;
                self.interrupt_enable = value;
            }
            (true, 0x10000000, _) => {
                self.divisor_latch = (self.divisor_latch & 0xff00) | (value as u16);
            }
            (true, 0x10000001, _) => {
                self.divisor_latch = (self.divisor_latch & 0x00ff) | ((value as u16) << 8);
            }
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
            csr::time if self.smode => trap::get_mtime(),
            csr::time => unimplemented!(),
            c => {
                println!("Read from unrecognized CSR: {:#x}", c);
                return None;
            }
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
                // This should not be necessary. The RISC-V spec says that if a program wants to
                // flush the TLB after a page table swap it has to do so with a seperate
                // sfence.vma. Unfortunately, Linux does not seem to respect this and segfaults if
                // we don't flush here.
                pmap::flush_shadow_page_table(&mut self.shadow_page_tables);
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
            c => {
                println!("Write to unrecognized CSR: {:#x}", c);
                return false;
            }
        }

        return true;
    }

    pub fn shadow(&self) -> PageTableRoot {
        if (self.csrs.satp & SATP_MODE) == 0 {
            PageTableRoot::MPA
        } else if !self.smode {
            PageTableRoot::UVA
        } else if self.csrs.sstatus & STATUS_SUM == 0 {
            PageTableRoot::KVA
        } else {
            PageTableRoot::MVA
        }
    }
}

pub unsafe fn initialize(machine: &MachineMeta, shadow_page_tables: PageTables, guest_memory: MemoryRegion) {
    *CONTEXT.lock() = Some(Context{
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
        saved_registers: MemoryRegion::with_base_address(SSTACK_BASE, 0, 32 * 8),
        guest_memory,
        shadow_page_tables,
        plic: PlicState::new(),
        uart: Uart {
            dlab: false,
            interrupt_enable: 0,
            divisor_latch: 1,
            next_interrupt_time: 0,
            input_fifo: [0; 16],
            input_bytes_ready: 0,
            read_zero: true,
        },
        virtio: VirtIO {
            devices: [virtio::Device::new(); virtio::MAX_DEVICES],
            queue_guest_pages: [0; virtio::MAX_DEVICES * virtio::MAX_QUEUES],
            num_queue_guest_pages: 0,
        },
        guest_shift: machine.guest_shift,
        smode: true,
        no_interrupt: true,
    });
}
