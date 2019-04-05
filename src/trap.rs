use riscv_decode::Instruction;
use crate::context::{Context, CONTEXT};
use crate::{pfault, pmap, print, sum};

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

    pub const SSTACK_BASE: u64 = 0xffffffffc0a00000 - 32*8;
}
use self::constants::*;

pub trait U64Bits {
    fn get(&self, mask: Self) -> bool;
    fn set(&mut self, mask: Self, value: bool);
}
impl U64Bits for u64 {
    #[inline(always)]
    fn get(&self, mask: Self) -> bool {
        *self & mask != 0
    }
    #[inline(always)]
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
pub unsafe fn strap_entry() -> ! {
    asm!(".align 4
          csrw 0x140, sp      // Save stack pointer in sscratch
          li sp, $0           // Set stack pointer

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

          jal ra, strap       // Call `strap`
          li sp, $0           // Reset stack pointer, just to be safe

          // Restore registers
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

          // Restore stack pointer and return
          csrr sp, 0x140
          sret" :: "i"(SSTACK_BASE) : "memory" : "volatile");

    unreachable!()
}

#[no_mangle]
pub unsafe fn strap() {
    let cause = csrr!(scause);
    let status = csrr!(sstatus);

    if status.get(STATUS_SPP) {
        println!("Trap from within hypervisor?!");
        println!("sepc = {:#x}", csrr!(sepc));
        println!("stval = {:#x}", csrr!(stval));
        println!("cause = {}", cause);
        loop {}
    }

    let mut state = CONTEXT.lock();
    let mut state = (&mut *state).as_mut().unwrap();

    if (cause as isize) < 0 {
        handle_interrupt(&mut state, cause);
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == 12 || cause == 13 || cause == 15 {
        let pc = csrr!(sepc);
        if pfault::handle_page_fault(&mut state, cause, pc) {
            maybe_forward_interrupt(&mut state, pc);
        } else {
            forward_exception(&mut state, cause, pc);
        }
    } else if cause == 2 && state.smode {
        let pc = csrr!(sepc);
        let (instruction, decoded, len) = decode_instruction_at_address(&mut state, pc);
        let mut advance_pc = true;
        match decoded {
            Some(Instruction::Sret) => {
                if !state.csrs.sstatus.get(STATUS_SIE) && state.csrs.sstatus.get(STATUS_SPIE) {
                    state.no_interrupt = false;
                }
                state.csrs.pop_sie();
                state.smode = state.csrs.sstatus.get(STATUS_SPP);
                state.csrs.sstatus.set(STATUS_SPP, false);
                csrw!(sepc, state.csrs.sepc);
                advance_pc = false;

                if !state.smode {
                    state.no_interrupt = false;
                }
            }
            Some(fence @ Instruction::SfenceVma(_)) => pmap::handle_sfence_vma(&mut state, fence),
            Some(Instruction::Csrrw(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let value = get_register(state, i.rs1());
                state.set_csr(i.csr(), value);
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrs(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = get_register(state, i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrc(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = get_register(state, i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrwi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), i.zimm() as u64);
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrsi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrci(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(decoded) => {
                println!("Unrecognized instruction! {:?} @ pc={:#x}", decoded, pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
            None => {
                println!("Unrecognized instruction {:#x} @ pc={:#x}", instruction, pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
        }

        if advance_pc {
            csrw!(sepc, pc + len);
        }
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == 8 && state.smode {
        match get_register(state, 17) {
            0 => {
                state.csrs.sip.set(IP_STIP, false);
                state.csrs.mtimecmp = get_register(state, 10);
                state.host_clint.set_mtimecmp(state.csrs.mtimecmp);
            }
            1 => print::guest_putchar(get_register(state, 10) as u8),
            5 => asm!("fence.i" :::: "volatile"),
            6 | 7 => pmap::handle_sfence_vma(&mut state,
                                             Instruction::SfenceVma(riscv_decode::types::RType(0)) /* TODO */),
            i => {
                println!("Got ecall from guest function={}!", i);
                loop {}
            }
        }
        csrw!(sepc, csrr!(sepc) + 4);
    } else {
        if cause != 8 { // no need to print anything for guest syscalls...
            println!("Forward exception (cause = {}, smode={})!", cause, state.smode);
        } else {
            // println!("system call: {}({:#x}, {:#x}, {:#x}, {:#x})",
            //          syscall_name(get_register(state, 17)),
            //          get_register(state, 10), get_register(state, 11),
            //          get_register(state, 12), get_register(state, 13)
            // );
            // if syscall_name(get_register(state, 17)) == "write" {
            //     let fd = get_register(state, 10);
            //     let ptr = get_register(state, 11);
            //     let len = get_register(state, 12);
            //     if fd == 1 {
            //         print!("data = ");
            //         for i in 0..len {
            //             print::guest_putchar(*((ptr + i) as *const u8));
            //         }
            //     }
            // }
        }
        forward_exception(&mut state, cause, csrr!(sepc));
    }

    state.shadow_page_tables.install_root(state.shadow());
}

fn handle_interrupt(state: &mut Context, cause: u64) {
    let interrupt = cause & 0xff;
    match interrupt {
        0x1 => {
            // Software
            unimplemented!();
        }
        0x5 => {
            // Timer
            csrc!(sip, 1 << interrupt);
            assert_eq!(csrr!(sip) & (1 << interrupt), 0);

            let time = state.host_clint.get_mtime();
            crate::context::Uart::timer(state, time);
            if state.csrs.mtimecmp <= time {
                state.csrs.sip |= IP_STIP;
                state.no_interrupt = false;
            }

            let mut next = 0xffffffff;
            if state.uart.next_interrupt_time > time {
                next = next.min(state.uart.next_interrupt_time);
            }
            if state.csrs.mtimecmp > time {
                next = next.min(state.csrs.mtimecmp);
            }
            if next < 0xffffffff {
                state.host_clint.set_mtimecmp(next);
            }
        }
        0x9 => {
            // External
            let claim = state.host_plic.claim_and_clear();
            state.plic.set_pending(claim, true);

            // Guest might have masked out this interrupt
            if state.plic.interrupt_pending() {
                state.no_interrupt = false;
                state.csrs.sip |= IP_SEIP;
            } else {
                assert_eq!(state.csrs.sip & IP_SEIP, 0);
                println!("Guest masked external interrupt");
            }

        }
        i => {
            println!("Got interrupt #{}", i);
            unreachable!()
        }
    }
}

fn maybe_forward_interrupt(state: &mut Context, sepc: u64) {
    if state.no_interrupt {
        return;
    }

    if !state.csrs.sip.get(IP_SEIP) && state.plic.interrupt_pending() {
        state.csrs.sip.set(IP_SEIP, true);
    }

    if (!state.smode || state.csrs.sstatus.get(STATUS_SIE)) && (state.csrs.sie & state.csrs.sip != 0) {
        let cause = if state.csrs.sip.get(IP_SEIP) {
            9
        } else if state.csrs.sip.get(IP_STIP) {
            5
        } else if state.csrs.sip.get(IP_SSIP) {
            1
        } else {
            unreachable!()
        };

        // println!("||> Forwarding timer interrupt! (state.smode={}, sepc={:#x})", state.smode, sepc);
        // forward interrupt
        state.csrs.push_sie();
        state.csrs.sepc = sepc;
        state.csrs.scause = (1 << 63) | cause;
        state.csrs.sstatus.set(STATUS_SPP, state.smode);
        state.csrs.stval = 0;
        state.smode = true;

        match state.csrs.stvec & TVEC_MODE {
            0 => csrw!(sepc, state.csrs.stvec & TVEC_BASE),
            1 => csrw!(sepc, (state.csrs.stvec & TVEC_BASE) + 4 * cause),
            _ => unreachable!(),
        }
    } else {
        state.no_interrupt = true;
    }
}

fn forward_exception(state: &mut Context, cause: u64, sepc: u64) {
    // println!("||> Forward exception sepc={:#x}", sepc);
    state.csrs.push_sie();
    state.csrs.sepc = sepc;
    state.csrs.scause = cause;
    state.csrs.sstatus.set(STATUS_SPP, state.smode);
    state.csrs.stval = csrr!(stval);
    state.smode = true;
    csrw!(sepc, state.csrs.stvec & TVEC_BASE);
}

pub fn set_register(state: &mut Context, reg: u32, value: u64) {
    match reg {
        0 => {},
        1 | 3..=31 => state.saved_registers[reg as u64 * 8] = value,
        2 => csrw!(sscratch, value),
        _ => unreachable!(),
    }
}
pub fn get_register(state: &mut Context, reg: u32) -> u64 {
    match reg {
        0 => 0,
        1 | 3..=31 => state.saved_registers[reg as u64 * 8],
        2 => csrr!(sscratch),
        _ => unreachable!(),
    }
}

pub unsafe fn decode_instruction_at_address(_state: &mut Context, guest_va: u64) -> (u32, Option<Instruction>, u64) {
    let pc_ptr = guest_va as *const u16;
    let (len, instruction) = sum::access_user_memory(||{
        let il: u16 = *pc_ptr;
        match riscv_decode::instruction_length(il) {
            2 => (2, il as u32),
            4 => (4, il as u32 | ((*pc_ptr.offset(1) as u32) << 16)),
            _ => unreachable!(),
        }
    });
    (instruction, riscv_decode::decode(instruction).ok(), len as u64)
}
