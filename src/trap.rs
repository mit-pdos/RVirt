use spin::Mutex;
use riscv_decode::Instruction;
use crate::{csr, pfault, pmap};

#[allow(unused)]
pub mod constants {
    pub const TVEC_MODE: u64 = 0x3;
    pub const TVEC_BASE: u64 = !TVEC_MODE;

    pub const STATUS_UIE: u64 = 1 << 0;
    pub const STATUS_SIE: u64 = 1 << 1;
    pub const STATUS_UPIE: u64 = 1 << 4;
    pub const STATUS_SPIE: u64 = 1 << 5;
    pub const STATUS_SPP: u64 = 1 << 8;
    pub const STATUS_FS: u64 = 3 << 13;
    pub const STATUS_XS: u64 = 3 << 15;
    pub const STATUS_SUM: u64 = 1 << 18;
    pub const STATUS_MXR: u64 = 1 << 19;
    pub const STATUS_SD: u64 = 1 << 63;

    pub const STATUS_MPP_M: u64 = 3 << 11;
    pub const STATUS_MPP_S: u64 = 1 << 11;
    pub const STATUS_MPP_U: u64 = 0 << 11;

    // Mask of writable bits in sstatus.
    pub const SSTATUS_WRITABLE_MASK: u64 =
        STATUS_MXR |
        STATUS_SUM |
        STATUS_FS |
        STATUS_SPP |
        STATUS_SPIE |
        STATUS_SIE;
    pub const SSTATUS_DYNAMIC_MASK: u64 = STATUS_SD | STATUS_FS;

    pub const IP_SSIP: u64 = 1 << 1;
    pub const IP_STIP: u64 = 1 << 5;
    pub const IP_SEIP: u64 = 1 << 9;

    pub const IE_SSIE: u64 = 1 << 1;
    pub const IE_STIE: u64 = 1 << 5;
    pub const IE_SEIE: u64 = 1 << 9;

    pub const SATP_MODE: u64 = 0xf << 60;
    pub const SATP_ASID: u64 = 0xffff << 44;
    pub const SATP_PPN: u64 = 0xfff_ffffffff;

    pub const MSTACK_BASE: u64 = 0x80300000 - 16*8;
    pub const SSTACK_BASE: u64 = 0x80400000 - 32*8;
}
use self::constants::*;

pub const MAX_TSTACK_ADDR: u64 = 0x80400000;

pub const CLINT_ADDRESS: u64 = 0x2000000;
pub const CLINT_MTIMECMP0_OFFSET: u64 = 0x4000;
pub const CLINT_MTIME_OFFSET: u64 = 0x0000BFF8;

trait U64Bits {
    fn get(&self, mask: Self) -> bool;
    fn set(&mut self, mask: Self, value: bool);
}
impl U64Bits for u64 {
    fn get(&self, mask: Self) -> bool {
        *self & mask != 0
    }
    fn set(&mut self, mask: Self, value: bool) {
        if value {
            *self |= mask;
        } else {
            *self &= !mask;
        }
    }
}

// 0x340 = mscratch
// 0x140 = sscratch


#[naked]
#[no_mangle]
#[link_section = ".text.strap_entry"]
pub unsafe fn strap_entry() -> ! {
    const ROOT_SATP: u64 = pmap::ROOT.satp();
    asm!(".align 4
          // Save stack pointer in sscratch
          csrw 0x140, sp

          // switch to root page table
          li sp, $0
          csrw 0x180, sp

          // Set stack pointer
          li sp, 0x80400000
          addi sp, sp, -32*8

          // Save registers
          sd ra, 1*8(sp)
          sd gp, 3*8(sp)
          sd tp, 4*8(sp)
          sd t0, 5*8(sp)
          sd t1, 6*8(sp)
          sd t2, 7*8(sp)
          sd s0, 8*8(sp)
          sd s1, 9*8(sp)
          sd a0, 10*8(sp)
          sd a1, 11*8(sp)
          sd a2, 12*8(sp)
          sd a3, 13*8(sp)
          sd a4, 14*8(sp)
          sd a5, 15*8(sp)
          sd a6, 16*8(sp)
          sd a7, 17*8(sp)
          sd s2, 18*8(sp)
          sd s3, 19*8(sp)
          sd s4, 20*8(sp)
          sd s5, 21*8(sp)
          sd s6, 22*8(sp)
          sd s7, 23*8(sp)
          sd s8, 24*8(sp)
          sd s9, 25*8(sp)
          sd s10, 26*8(sp)
          sd s11, 27*8(sp)
          sd t3, 28*8(sp)
          sd t4, 29*8(sp)
          sd t5, 30*8(sp)
          sd t6, 31*8(sp)

          jal ra, strap

          // Save return value
          sd a0, 0(sp)

          // Load other registers
          ld ra, 1*8(sp)
          ld gp, 3*8(sp)
          ld tp, 4*8(sp)
          ld t0, 5*8(sp)
          ld t1, 6*8(sp)
          ld t2, 7*8(sp)
          ld s0, 8*8(sp)
          ld s1, 9*8(sp)
          ld a0, 10*8(sp)
          ld a1, 11*8(sp)
          ld a2, 12*8(sp)
          ld a3, 13*8(sp)
          ld a4, 14*8(sp)
          ld a5, 15*8(sp)
          ld a6, 16*8(sp)
          ld a7, 17*8(sp)
          ld s2, 18*8(sp)
          ld s3, 19*8(sp)
          ld s4, 20*8(sp)
          ld s5, 21*8(sp)
          ld s6, 22*8(sp)
          ld s7, 23*8(sp)
          ld s8, 24*8(sp)
          ld s9, 25*8(sp)
          ld s10, 26*8(sp)
          ld s11, 27*8(sp)
          ld t3, 28*8(sp)
          ld t4, 29*8(sp)
          ld t5, 30*8(sp)
          ld t6, 31*8(sp)

          // Load value address and use it to set SATP
          ld sp, 0(sp)
          csrw 0x180, sp

          // Restore stack pointer and return
          csrr sp, 0x140
          sret" :: "i"(ROOT_SATP) :: "volatile");

    unreachable!()
}

#[derive(Default)]
pub struct ShadowState {
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

    // Whether the guest is in S-Mode.
    pub smode: bool,

    pub uart_dlab: bool,
}
impl ShadowState {
    pub const fn new() -> Self {
        Self {
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

            smode: true,

            uart_dlab: false,
        }
    }
    pub fn push_sie(&mut self) {
        self.sstatus.set(STATUS_SPIE, self.sstatus.get(STATUS_SIE));
        self.sstatus.set(STATUS_SIE, false);
    }
    pub fn pop_sie(&mut self) {
        self.sstatus.set(STATUS_SIE, self.sstatus.get(STATUS_SPIE));
        self.sstatus.set(STATUS_SPIE, true);
    }

    pub fn get_csr(&mut self, csr: u32) -> Option<u64> {
        Some(match csr as u64 {
            csr::sstatus => {
                let real = csrr!(sstatus);
                self.sstatus = (self.sstatus & !SSTATUS_DYNAMIC_MASK) | (real & SSTATUS_DYNAMIC_MASK);
                self.sstatus
            }
            csr::satp => self.satp,
            csr::sie => self.sie,
            csr::stvec => self.stvec,
            csr::sscratch => self.sscratch,
            csr::sepc => self.sepc,
            csr::scause => self.scause,
            csr::stval => self.stval,
            csr::sip => self.sip,
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

                let changed = self.sstatus ^ value;
                self.sstatus = value;

                if changed & STATUS_MXR != 0 {
                    unimplemented!("STATUS.MXR");
                }
                if changed & STATUS_FS != 0 {
                    unimplemented!("STATUS.FS");
                }
            }
            csr::satp => {
                let mode = (value & SATP_MODE) >> 60;
                if mode == 0 || mode == 8 {
                    self.satp = value & !SATP_ASID;
                }
            }
            csr::sie => self.sie = value & (IE_SEIE | IE_STIE | IE_SSIE),
            csr::stvec => self.stvec = value & !0x2,
            csr::sscratch => self.sscratch = value,
            csr::sepc => self.sepc = value,
            csr::scause => self.scause = value,
            csr::stval => self.stval = value,
            csr::sip => self.sip = (self.sip & !IP_SSIP) | (value & IP_SSIP),
            csr::sedeleg |
            csr::sideleg |
            csr::scounteren => {}
            _ => return false,
        }

        return true;
    }

    pub fn shadow(&self) -> pmap::PageTableRoot {
        if (self.satp & SATP_MODE) == 0 {
            pmap::MPA
        } else if !self.smode {
            pmap::UVA
        } else if self.sstatus & STATUS_SUM == 0 {
            pmap::KVA
        } else {
            pmap::MVA
        }
    }
}

static SHADOW_STATE: Mutex<ShadowState> = Mutex::new(ShadowState::new());

#[no_mangle]
pub unsafe fn strap() -> u64 {
    let cause = csrr!(scause);
    let status = csrr!(sstatus);

    if status.get(STATUS_SPP) {
        println!("Trap from within hypervisor?!");
        println!("sepc = {:#x}", csrr!(sepc));
        println!("cause = {}", cause);
        loop {}
    }

    let mut state = SHADOW_STATE.lock();
    if (cause as isize) < 0 {
        // let cause = if cause & 0xff == 1 {
        //     // Treat software interrupts as timer interrupts so we can reset them without an SBI
        //     // call.
        //     0x8000000000000005
        // } else {
        //     cause
        // };
        csrw!(sip, 0);
        state.sip = state.sip | (1 << (cause & 0xff));
        // println!("Got interrupt at pc={:#x}, smode={}, spp={}", csrr!(sepc), state.smode, state.sstatus.get(STATUS_SPP));
        handle_interrupt(&mut state, cause, csrr!(sepc));
    } else if cause == 12 || cause == 13 || cause == 15 {
        let pc = csrr!(sepc);
        if !pfault::handle_page_fault(&mut state, cause, pc) {
            forward_exception(&mut state, cause, pc);
        }
    } else if cause == 2 && state.smode {
        let pc = csrr!(sepc);
        let (decoded, len) = decode_instruction_at_address(&mut state, pc);
        let mut advance_pc = true;
        match decoded {
            Some(Instruction::Sret) => {
                state.pop_sie();
                state.smode = state.sstatus.get(STATUS_SPP);
                state.sstatus.set(STATUS_SPP, false);
                csrw!(sepc, state.sepc);
                advance_pc = false;
                // if state.sip.get(IP_SSIP) {
                //     println!("Software interupt?");
                //     handle_interrupt(&mut state, 0x8000000000000001, csrr!(sepc));
                // } else if state.sip.get(IP_STIP) {
                //     println!("Timer interupt?");
                //     handle_interrupt(&mut state, 0x8000000000000005, csrr!(sepc));
                // }
            }
            Some(fence @ Instruction::SfenceVma(_)) => pmap::handle_sfence_vma(&mut state, fence),
            Some(Instruction::Csrrw(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), get_register(i.rs1()));
                set_register(i.rd(), prev);
            }
            Some(Instruction::Csrrs(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), prev | get_register(i.rs1()));
                set_register(i.rd(), prev);
            }
            Some(Instruction::Csrrc(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), prev & !get_register(i.rs1()));
                set_register(i.rd(), prev);
            }
            Some(Instruction::Csrrwi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), i.zimm() as u64);
                set_register(i.rd(), prev);
            }
            Some(Instruction::Csrrsi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), prev | (i.zimm() as u64));
                set_register(i.rd(), prev);
            }
            Some(Instruction::Csrrci(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), prev & !(i.zimm() as u64));
                set_register(i.rd(), prev);
            }
            Some(decoded) => {
                println!("Unrecognized instruction! {:?} @ pc={:#x}", decoded, pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
            None => {
                println!("Unrecognized instruction @ pc={:#x}", pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
        }
        if advance_pc {
            csrw!(sepc, pc + len);
        }
    } else if cause == 8 && state.smode {
        match get_register(17) {
            0 => {
                let ticks = get_register(10);
                let mtime = get_mtime();

                state.sip.set(IP_STIP, false);
                state.mtimecmp = mtime + ticks*100;

                // println!("Setting timer to fire in {} ticks (mtime = {})", ticks, mtime);
                set_mtimecmp0(state.mtimecmp);
            }
            1 => print!("{}", get_register(10) as u8 as char),
            i => {
                println!("Got ecall from guest function={}!", i);
                loop {}
            }
        }
        csrw!(sepc, csrr!(sepc) + 4);
    } else {
        println!("Forward exception (cause = {}, smode={})!", cause, state.smode);
        forward_exception(&mut state, cause, csrr!(sepc));
    }

    state.shadow().satp()
}

fn handle_interrupt(state: &mut ShadowState, cause: u64, sepc: u64) {
    let enabled = state.sstatus.get(STATUS_SIE);
    let unmasked = state.sie & (1 << (cause & 0xff)) != 0;

    if (!state.smode || enabled) && unmasked {
        // println!("||> Forwarding timer interrupt! (state.smode={}, sepc={:#x})", state.smode, sepc);
        // forward interrupt
        state.push_sie();
        state.sepc = sepc;
        state.scause = cause;
        state.sstatus.set(STATUS_SPP, state.smode);
        state.stval = 0;
        state.smode = true;

        match state.stvec & TVEC_MODE {
            0 => csrw!(sepc, state.stvec & TVEC_BASE),
            1 => csrw!(sepc, (state.stvec & TVEC_BASE) + 4 * cause & 0xff),
            _ => unreachable!(),
        }
    }
}

fn forward_exception(state: &mut ShadowState, cause: u64, sepc: u64) {
    // println!("||> Forward exception sepc={:#x}", sepc);
    state.push_sie();
    state.sepc = sepc;
    state.scause = cause;
    state.sstatus.set(STATUS_SPP, state.smode);
    state.stval = csrr!(stval);
    state.smode = true;
    csrw!(sepc, state.stvec & TVEC_BASE);
}

pub fn set_register(reg: u32, value: u64) {
    match reg {
        0 => {},
        1 | 3..=31 => unsafe { *(SSTACK_BASE as *mut u64).offset(reg as isize) = value as u64; }
        2 => csrw!(sscratch, value),
        _ => unreachable!(),
    }
}
pub fn get_register(reg: u32) -> u64 {
    match reg {
        0 => 0,
        1 | 3..=31 => unsafe { *(SSTACK_BASE as *const u64).offset(reg as isize) as u64 },
        2 => csrr!(sscratch),
        _ => unreachable!(),
    }
}

fn get_mtime() -> u64 {
    unsafe { *((CLINT_ADDRESS + CLINT_MTIME_OFFSET) as *const u64) }
}
fn set_mtimecmp0(value: u64) {
    unsafe { *((CLINT_ADDRESS + CLINT_MTIMECMP0_OFFSET) as *mut u64) = value; }
}

pub unsafe fn decode_instruction_at_address(state: &mut ShadowState, guest_va: u64) -> (Option<Instruction>, u64) {
    let pc_ptr = state.shadow().address_to_pointer(guest_va);

    let il: u16 = *pc_ptr;
    let len = riscv_decode::instruction_length(il);
    let instruction = match len {
        2 => il as u32,
        4 => il as u32 | ((*pc_ptr.offset(1) as u32) << 16),
        _ => unreachable!(),
    };
    (riscv_decode::try_decode(instruction), len as u64)
}
