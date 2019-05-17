use arrayvec::ArrayVec;
use spin::Mutex;
use crate::fdt::MachineMeta;
use crate::memory_region::MemoryRegion;
use crate::plic::PlicState;
use crate::pmap::{PageTables, PageTableRoot};
use crate::riscv::bits::*;
use crate::riscv::csr;
use crate::statics::SHARED_STATICS;
use crate::trap::U64Bits;
use crate::{pmap, print, riscv, virtio};

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
    pub devices: ArrayVec<[virtio::Device; virtio::MAX_DEVICES]>,
    pub queue_guest_pages: ArrayVec<[u64; virtio::MAX_DEVICES * virtio::MAX_QUEUES]>,
}

pub struct Uart {
    pub dlab: bool,

    pub divisor_latch: u16,
    pub interrupt_enable: u8,

    pub next_interrupt_time: u64,

    pub input_fifo: [u8; 16],
    pub input_bytes_ready: usize,

    pub line_buffer: ArrayVec<[u8; 256]>,
    pub guestid: Option<u64>,
}

pub struct HostClint {
    pub mtime: MemoryRegion,
    pub mtimecmp: MemoryRegion,
}

pub struct HostPlic {
    pub claim_clear: MemoryRegion<u32>,
}

pub struct SavedRegisters {
    registers: MemoryRegion,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IrqMapping {
    Virtio { device_index: u8, guest_irq: u16 },
    Ignored,
}

pub struct Context {
    pub csrs: ControlRegisters,
    pub plic: PlicState,
    pub uart: Uart,
    pub virtio: VirtIO,

    pub saved_registers: SavedRegisters,
    pub guest_memory: MemoryRegion,
    pub shadow_page_tables: PageTables,

    pub guest_shift: u64,

    /// Whether the guest is in S-Mode.
    pub smode: bool,

    /// If set, hypervisor exits do not need to check for pending interrupts
    pub no_interrupt: bool,

    pub tlb_caches_invalid_ptes: bool,
    pub consecutive_page_fault_count: u64,

    pub host_clint: HostClint,
    pub host_plic: HostPlic,

    /// Map from host external interrupt number to guest external interrupt nmuber
    pub irq_map: [IrqMapping; 512],
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
    const IRQ: u32 = 10;

    fn tx_interrupt(&self, current_time: u64) -> bool {
        self.next_interrupt_time  <= current_time && self.interrupt_enable & 0x2 != 0
    }
    fn rx_interrupt(&self) -> bool {
        self.input_bytes_ready >= 1 && self.interrupt_enable & 0x1 != 0
    }
    pub fn timer(state: &mut Context, current_time: u64) {
        state.uart.fill_fifo();
        if state.uart.tx_interrupt(current_time) || state.uart.rx_interrupt() {
            state.plic.set_pending(Uart::IRQ, true);
            state.no_interrupt = false;
        }
    }

    pub fn fill_fifo(&mut self) {
        while self.input_bytes_ready < self.input_fifo.len() {
            if let Some(ch) = SHARED_STATICS.uart_writer.lock().getchar() {
                self.input_fifo[self.input_bytes_ready] = ch;
                self.input_bytes_ready += 1;
            } else {
                break;
            }
        }
    }

    const TRANSMIT_HOLDING_REGISTER: u64 = 0x10000000;
    const RECEIVE_BUFFER_REGISTER: u64 = 0x10000000;
    const DIVISOR_LATCH_LSB: u64 = 0x10000000;
    const INTERRUPT_ENABLE_REGISTER: u64 = 0x10000001;
    const DIVISOR_LATCH_MSB: u64 = 0x10000001;
    const FIFO_CONTROL_REGISTER: u64 = 0x10000002;
    const INTERRUPT_IDENTIFICATION_REGISTER: u64 = 0x10000002;
    const LINE_CONTROL_REGISTER: u64 = 0x10000003;
    const MODEM_CONTROL_REGISTER: u64 = 0x10000004;
    const LINE_STATUS_REGISTER: u64 = 0x10000005;
    const MODEM_STATUS_REGISTER: u64 = 0x10000006;
    #[allow(unused)]
    const SCRATCH_REGISTER: u64 = 0x10000007;

    // bits for interrupt identification register
    const IIR_FIFOS_ENABLED: u8 = 0xC0;
    const IIR_INTERRUPT_NOT_PENDING: u8 = 0x01; // set to zero for interrupt pending
    // note: bits 1-3 are an enumeration as follows, not a bitmask
    const IIR_TX_INTERRUPT: u8 = 0x02; // transmit fifo has room for more data
    const IIR_RX_INTERRUPT: u8 = 0x04; // receive fifo contains data

    // bits for line control register
    const LCR_EIGHT_BIT_WORDS: u8 = 0x03; // eight bit words
    const LCR_DIVISOR_LATCH_ACCESS: u8 = 0x80; // divisor latch access bit (DLAB)

    // bits for line status register
    const LSR_DATA_READY: u8 = 0x01;
    #[allow(unused)]
    const LSR_BREAK_INTERRUPT: u8 = 0x10;
    const LSR_TRANSMITTER_HAS_ROOM: u8 = 0x20;
    const LSR_TRANSMITTER_EMPTY: u8 = 0x40;

    // bits for modem status register
    const MSR_CLEAR_TO_SEND: u8 = 0x10;

    // bits for modem control register
    const MCR_LOOPBACK_ENABLE: u8 = 0x10;
    const MCR_RESERVED_BITS: u8 = 0xe0;

    pub fn read(&mut self, host_clint: &HostClint, addr: u64) -> u8 {
        match (self.dlab, addr) {
            (false, Uart::RECEIVE_BUFFER_REGISTER) => {
                if self.input_bytes_ready > 0 {
                    let ret = self.input_fifo[0];
                    self.input_bytes_ready -= 1;
                    for i in 0..(self.input_bytes_ready) {
                        self.input_fifo[i] = self.input_fifo[i+1];
                    }
                    ret
                } else {
                    0
                }
            }
            (true, Uart::DIVISOR_LATCH_LSB) => (self.divisor_latch & 0xff) as u8,
            (true, Uart::DIVISOR_LATCH_MSB) => (self.divisor_latch >> 8) as u8,
            (false, Uart::INTERRUPT_ENABLE_REGISTER) => self.interrupt_enable, // (top four should always be zero)
            (_, Uart::INTERRUPT_IDENTIFICATION_REGISTER) => {
                if self.rx_interrupt() {
                    Uart::IIR_FIFOS_ENABLED | Uart::IIR_RX_INTERRUPT
                } else if self.tx_interrupt(host_clint.get_mtime()) {
                    Uart::IIR_FIFOS_ENABLED | Uart::IIR_TX_INTERRUPT
                } else {
                    Uart::IIR_FIFOS_ENABLED | Uart::IIR_INTERRUPT_NOT_PENDING
                }
            },
            (true, Uart::LINE_CONTROL_REGISTER) => Uart::LCR_EIGHT_BIT_WORDS,
            (false, Uart::LINE_CONTROL_REGISTER) => Uart::LCR_EIGHT_BIT_WORDS | Uart::LCR_DIVISOR_LATCH_ACCESS,
            (_, Uart::LINE_STATUS_REGISTER) => {
                self.fill_fifo();

                let mut lsr = 0;
                if self.input_bytes_ready > 0 {
                    lsr |= Uart::LSR_DATA_READY;
                }
                if host_clint.get_mtime() >= self.next_interrupt_time {
                    lsr |= Uart::LSR_TRANSMITTER_HAS_ROOM | Uart::LSR_TRANSMITTER_EMPTY;
                }
                lsr
            }
            (_, Uart::MODEM_STATUS_REGISTER) => Uart::MSR_CLEAR_TO_SEND, // other bits don't matter to Linux
            (dlab, _) => {
                println!("UART: Read uimplemented ?? <- {:#x} (dlab={})", addr, dlab);
                loop {}
            }
        }
    }
    pub fn write(&mut self, host_clint: &HostClint, addr: u64, value: u8) {
        match (self.dlab, addr, value) {
            (false, Uart::TRANSMIT_HOLDING_REGISTER, _) => {
                self.output_byte(value as u8);

                let current_time = host_clint.get_mtime();
                let transmit_time = self.divisor_latch as u64 * 5;
                self.next_interrupt_time =
                    self.next_interrupt_time.max(current_time) + transmit_time;
            }
            (false, Uart::INTERRUPT_ENABLE_REGISTER, _) => {
                self.interrupt_enable = value;
            }
            (true, Uart::DIVISOR_LATCH_LSB, _) => {
                self.divisor_latch = (self.divisor_latch & 0xff00) | (value as u16);
            }
            (true, Uart::DIVISOR_LATCH_MSB, _) => {
                self.divisor_latch = (self.divisor_latch & 0x00ff) | ((value as u16) << 8);
            }
            (_, Uart::FIFO_CONTROL_REGISTER, _) => {}
            (_, Uart::LINE_CONTROL_REGISTER, _) => self.dlab = (value & Uart::LCR_DIVISOR_LATCH_ACCESS) != 0,
            (_, Uart::MODEM_CONTROL_REGISTER, _) if value & (Uart::MCR_LOOPBACK_ENABLE | Uart::MCR_RESERVED_BITS) == 0 => {}
            _ => {
                println!("UART: Write unimplemented {:#x} -> {:#x} (dlab={})",
                         value, addr, self.dlab);
                loop {}
            }
        }
    }

    pub fn output_byte(&mut self, value: u8) {
        if let Some(guestid) = self.guestid {
            let len = self.line_buffer.len();
            if len > 0 && self.line_buffer[len - 1] == '\r' as u8 && value != '\n' as u8 {
                print::guest_println(guestid, &self.line_buffer);
                self.line_buffer.clear();
            }
            if value == '\n' as u8 || self.line_buffer.is_full() {
                print::guest_println(guestid, &self.line_buffer);
                self.line_buffer.clear();
            } else {
                self.line_buffer.push(value);
            }
        } else {
            SHARED_STATICS.uart_writer.lock().putchar(value);
        }
    }
}

impl HostClint {
    pub fn get_mtime(&self) -> u64 {
        self.mtime[0]
    }
}

impl HostPlic {
    pub fn claim_and_clear(&mut self) -> u32 {
        let claim = self.claim_clear[0];
        riscv::barrier();
        self.claim_clear[0] = claim;
        claim
    }
}

impl SavedRegisters {
    pub fn get(&self, reg: u32) -> u64 {
        match reg {
            0 => 0,
            1 | 3..=31 => self.registers[reg as u64 * 8],
            2 => csrr!(sscratch),
            _ => unreachable!(),
        }
    }
    pub fn set(&mut self, reg: u32, value: u64) {
        match reg {
            0 => {},
            1 | 3..=31 => self.registers[reg as u64 * 8] = value,
            2 => riscv::set_sscratch(value),
            _ => unreachable!(),
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
            csr::time if self.smode => self.host_clint.get_mtime(),
            csr::time => unimplemented!(),
            c => {
                println!("Read from unrecognized CSR: {:#x}", c);
                return None;
            }
        })
    }

    pub fn set_csr(&mut self, csr: u32, value: u64) -> bool {
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
                    riscv::set_sstatus_fs(value);
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
                // This should not be necessary. However, currently QEMU doesn't trap when
                // sfence.vma is executed from user mode so flush here to compensate.
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

pub unsafe fn initialize(machine: &MachineMeta,
                         guest_machine: &MachineMeta,
                         shadow_page_tables: PageTables,
                         guest_memory: MemoryRegion,
                         guest_shift: u64,
                         hartid: u64,
                         guestid: Option<u64>) {
    let mut irq_map = [IrqMapping::Ignored; 512];
    let mut virtio_devices = ArrayVec::new();
    for i in 0..4 {
        let index = (guestid.unwrap_or(1) as usize - 1) * 4 + i;
        if index < machine.virtio.len() {
            virtio_devices.push(virtio::Device::new(machine.virtio[index].base_address));
            let host_irq = machine.virtio[index].irq;
            let mut guest_irq = None;
            for j in 0..4 {
                if guest_machine.virtio[j].base_address == 0x10001000 + 0x1000 * i as u64 {
                    guest_irq = Some(guest_machine.virtio[j].irq);
                    break;
                }
            }
            assert_eq!(irq_map[host_irq as usize], IrqMapping::Ignored);
            irq_map[host_irq as usize] = IrqMapping::Virtio {
                device_index: i as u8,
                guest_irq: guest_irq.unwrap() as u16
            };
        } else {
            virtio_devices.push(virtio::Device::Unmapped);
        }
    }

    let plic_context = machine.harts.iter().find(|h| h.hartid == hartid).unwrap().plic_context;

    let context = Context {
        csrs: ControlRegisters {
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
        saved_registers: SavedRegisters {
            registers: MemoryRegion::with_base_address(SSTACK_BASE, 0, 32 * 8)
        },
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
            line_buffer: ArrayVec::new(),
            guestid,
        },
        virtio: VirtIO {
            devices: virtio_devices,
            queue_guest_pages: ArrayVec::new(),
        },
        guest_shift,
        smode: true,
        no_interrupt: true,
        host_clint: HostClint {
            mtime: MemoryRegion::with_base_address(
                pmap::pa2va(machine.clint_address + 0xbff8), 0, 8),
            mtimecmp: MemoryRegion::with_base_address(
                pmap::pa2va(machine.clint_address + 0x4000 + 8*hartid), 0, 8),
        },
        host_plic: HostPlic {
            claim_clear: MemoryRegion::with_base_address(
                pmap::pa2va(machine.plic_address + 0x200004 + 0x1000 * plic_context), 0, 8),
        },
        consecutive_page_fault_count: 0,
        tlb_caches_invalid_ptes: false,
        irq_map,
    };

    // Memory backing for CONTEXT might not be in a valid state, so force_unlock() first, and avoid
    // calling drop on the old contents. This is safe because no other hart will be trying to access
    // this memory right now.
    CONTEXT.force_unlock();
    let old = CONTEXT.lock().replace(context);
    core::mem::forget(old);
}
