use riscv_decode::Instruction;
use crate::context::{Context, CONTEXT, IrqMapping};
use crate::riscv::bits::*;
use crate::{pfault, pmap, riscv, sum, virtio};

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

#[naked]
#[no_mangle]
pub unsafe fn strap_entry() -> ! {
    asm!(".align 4
          csrw sscratch, sp   // Save stack pointer in sscratch
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
          csrr sp, sscratch
          sret" :: "i"(SSTACK_BASE) : "memory" : "volatile");

    unreachable!()
}

#[no_mangle]
pub fn strap() {
    let cause = csrr!(scause);
    let status = csrr!(sstatus);

    if status.get(STATUS_SPP) {
        println!("Trap from within hypervisor?!");
        println!("sepc = {:#x}", csrr!(sepc));
        println!("stval = {:#x}", csrr!(stval));
        println!("cause = {}", cause);

        // No other threads could be accessing CONTEXT here, and even if we interrupted a critical
        // section, we're about to crash anyway so it doesn't matter that much.
        unsafe { CONTEXT.force_unlock() }
        let state = CONTEXT.lock();
        let state = (&*state).as_ref().unwrap();

        println!("reg ra = {:#x}", state.saved_registers.get(1));
        println!("reg sp = {:#x}", state.saved_registers.get(2));
        for i in 3..32 {
            println!("reg x{} = {:#x}", i, state.saved_registers.get(i));
        }

        loop {}
    }

    let mut state = CONTEXT.lock();
    let mut state = (&mut *state).as_mut().unwrap();

    // For the processor to have generated a load/store page fault or an illegal instruction fault,
    // the processor must have been able to load the relevant instruction (or else an access fault
    // or instruction page fault would have been triggered). Thus, it is safe to access memory
    // pointed to by `sepc`.
    let instruction = match cause {
        SCAUSE_LOAD_PAGE_FAULT |
        SCAUSE_STORE_PAGE_FAULT |
        SCAUSE_ILLEGAL_INSN => unsafe {
            Some(load_instruction_at_address(&mut state, csrr!(sepc)))
        }
        _ => None,
    };

    if (cause as isize) < 0 {
        handle_interrupt(&mut state, cause);
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == SCAUSE_INSN_PAGE_FAULT || cause == SCAUSE_LOAD_PAGE_FAULT || cause == SCAUSE_STORE_PAGE_FAULT {
        let pc = csrr!(sepc);
        if pfault::handle_page_fault(&mut state, cause, instruction.map(|i|i.0)) {
            maybe_forward_interrupt(&mut state, pc);
        } else {
            forward_exception(&mut state, cause, pc);
        }
    } else if cause == SCAUSE_ILLEGAL_INSN && state.smode {
        let pc = csrr!(sepc);
        let (instruction, len) = instruction.unwrap();
        let mut advance_pc = true;
        match riscv_decode::decode(instruction).ok() {
            Some(Instruction::Sret) => {
                if !state.csrs.sstatus.get(STATUS_SIE) && state.csrs.sstatus.get(STATUS_SPIE) {
                    state.no_interrupt = false;
                }
                state.csrs.pop_sie();
                state.smode = state.csrs.sstatus.get(STATUS_SPP);
                state.csrs.sstatus.set(STATUS_SPP, false);
                riscv::set_sepc(state.csrs.sepc);
                advance_pc = false;

                if !state.smode {
                    state.no_interrupt = false;
                }
            }
            Some(Instruction::SfenceVma(rtype)) => pmap::handle_sfence_vma(&mut state, rtype),
            Some(Instruction::Csrrw(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let value = state.saved_registers.get(i.rs1());
                state.set_csr(i.csr(), value);
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Csrrs(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = state.saved_registers.get(i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Csrrc(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = state.saved_registers.get(i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Csrrwi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), i.zimm() as u64);
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Csrrsi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Csrrci(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                state.saved_registers.set(i.rd(), prev);
            }
            Some(Instruction::Wfi) => {}
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
            riscv::set_sepc(pc + len);
        }
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == SCAUSE_ENV_CALL && state.smode {
        match state.saved_registers.get(17) {
            0 => {
                state.csrs.sip.set(IP_STIP, false);
                state.csrs.mtimecmp = state.saved_registers.get(10);
                riscv::sbi::set_timer(state.csrs.mtimecmp);
            }
            1 => {
                let value = state.saved_registers.get(10) as u8;
                state.uart.output_byte(value)
            }
            5 => riscv::fence_i(),
            6 | 7 => {
                // Current versions of the Linux kernel pass wrong arguments to these SBI calls. As
                // a result, this function ignores the arguments and just does a global fence. This
                // will eventually be fixed by https://patchwork.kernel.org/patch/10872353.
                pmap::flush_shadow_page_table(&mut state.shadow_page_tables);
            }
            i => {
                println!("Got ecall from guest function={}!", i);
                loop {}
            }
        }
        riscv::set_sepc(csrr!(sepc) + 4);
    } else {
        if cause != SCAUSE_ENV_CALL { // no need to print anything for guest syscalls...
            println!("Forward exception (cause = {}, smode={})!", cause, state.smode);
        }
        forward_exception(&mut state, cause, csrr!(sepc));
    }

    state.shadow_page_tables.install_root(state.shadow());
}

fn handle_interrupt(state: &mut Context, cause: u64) {
    let interrupt = cause & 0xff;
    match interrupt {
        0x1 => {
            // Software interrupt
            unreachable!();
        }
        0x5 => {
            // Timer interrupt
            let time = state.host_clint.get_mtime();
            let mut next = time + 1_000_000;

            crate::context::Uart::timer(state, time);
            if state.csrs.mtimecmp <= time {
                state.csrs.sip |= IP_STIP;
                state.no_interrupt = false;
            } else {
                next = next.min(state.csrs.mtimecmp);
            }

            if state.uart.next_interrupt_time > time {
                next = next.min(state.uart.next_interrupt_time);
            }
            riscv::sbi::set_timer(next);
        }
        0x9 => {
            // External
            let host_irq = state.host_plic.claim_and_clear();
            let guest_irq = state.irq_map[host_irq as usize];
            match guest_irq {
                IrqMapping::Virtio { device_index, guest_irq } => {
                    let forward = match state.virtio.devices[device_index as usize] {
                        virtio::Device::Passthrough { .. } => true,
                        virtio::Device::Unmapped => false,
                        virtio::Device::Macb(ref mut macb) => macb.interrupt(&mut state.guest_memory),
                    };

                    if forward {
                        state.plic.set_pending(guest_irq as u32, true);

                        // Guest might have masked out this interrupt
                        if state.plic.interrupt_pending() {
                            state.no_interrupt = false;
                            state.csrs.sip |= IP_SEIP;
                        } else {
                            assert_eq!(state.csrs.sip & IP_SEIP, 0);
                        }
                    }
                }
                IrqMapping::Ignored => {}
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
            0 => riscv::set_sepc(state.csrs.stvec & TVEC_BASE),
            1 => riscv::set_sepc((state.csrs.stvec & TVEC_BASE) + 4 * cause),
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
    riscv::set_sepc(state.csrs.stvec & TVEC_BASE);
}

pub unsafe fn load_instruction_at_address(_state: &mut Context, guest_va: u64) -> (u32, u64) {
    let pc_ptr = guest_va as *const u16;
    sum::access_user_memory(||{
        let il: u16 = *pc_ptr;
        match riscv_decode::instruction_length(il) {
            2 => (il as u32, 2),
            4 => (il as u32 | ((*pc_ptr.offset(1) as u32) << 16), 4),
            _ => unreachable!(),
        }
    })
}
