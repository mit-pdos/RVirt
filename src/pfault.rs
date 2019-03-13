use crate::pmap::*;
use crate::trap::{self, ShadowState, constants::SATP_PPN};
use crate::{print, sum, virtio};
use riscv_decode::Instruction;

/// Perform any handling required in response to a guest page fault. Returns true if the fault could
/// be handled, or false if it should be forwarded on to the guest.
pub unsafe fn handle_page_fault(state: &mut ShadowState, cause: u64, pc: u64) -> bool {
    let shadow = state.shadow();
    if shadow == MPA {
        println!("Page fault without guest paging enabled?");
        return false;
    }

    let guest_va = csrr!(stval);
    assert!((guest_va & SV39_MASK) < (511 << 30));

    let access = match cause {
        12 => PTE_EXECUTE,
        13 => PTE_READ,
        15 => PTE_WRITE,
        _ => unreachable!(),
    };

    let page = guest_va & !0xfff;
    if let Some(translation) = translate_guest_address((state.satp & SATP_PPN) << 12, page) {
        let guest_pte = sum::access_user_memory(|| *translation.pte);

        // Check R/W/X bits
        if guest_pte & access == 0 {
            // println!("Bad access bit guest_va={:#x}, guest_pte={:#x}, cause={}", guest_va, guest_pte, cause);
            return false;
        }

        // Check U bit
        match shadow {
            UVA => if guest_pte & PTE_USER == 0 { return false; }
            KVA => if guest_pte & PTE_USER != 0 { return false; }
            MVA => {}
            _ => unreachable!(),
        }

        if let Some(host_pa) = mpa2pa(translation.guest_pa) {
            // Set A and D bits
            // TODO: set bits atomically
            let pte = sum::access_user_memory(|| {
                if (*translation.pte & PTE_DIRTY) == 0 && access == PTE_WRITE {
                    *translation.pte = *translation.pte | PTE_DIRTY | PTE_ACCESSED;
                } else if (*translation.pte & PTE_ACCESSED) == 0 {
                    *translation.pte = *translation.pte | PTE_ACCESSED;
                }
                *translation.pte
            });

            let perm = if (pte & PTE_DIRTY) == 0 && access != PTE_WRITE {
                (guest_pte & (PTE_READ | PTE_EXECUTE))
            } else {
                (guest_pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE))
            };

            if virtio::is_queue_access(state, translation.guest_pa) {
                let guest_pa = (translation.guest_pa & !0xfff) | (guest_va & 0xfff);
                let host_pa = (host_pa & !0xfff) | (guest_va & 0xfff);
                return virtio::handle_queue_access(state, guest_pa, host_pa, pc);
            }

            *shadow.get_pte(page) = (host_pa >> 2) | perm | PTE_AD | PTE_USER | PTE_VALID;
            return true;
        } else if access != PTE_EXECUTE && state.smode {
            let pa = (translation.guest_pa & !0xfff) | (guest_va & 0xfff);

            if is_uart_access(pa) {
                return handle_uart_access(state, pa, pc);
            }

            if is_plic_access(pa) {
                return handle_plic_access(state, pa, pc)
            }

            if virtio::is_device_access(pa) {
                return virtio::handle_device_access(state, pa, pc);
            }
        }

        println!("Guest page table specified invalid guest address, va={:#x}, pa={:#x}, pc={:x}",
                 guest_va, translation.guest_pa, csrr!(sepc));
        println!("shadow.satp={:#x}", shadow.satp());
        for i in 0..16 {
            print!("x{:<2}={:16x}       ", i*2, trap::get_register(i*2));
            println!("x{:<2}={:16x}", i*2+1, trap::get_register(i*2+1));
        }
        return false;
    } else {
        println!("forwarding page fault: \n sepc = {:x}, stval = {:#x}", csrr!(sepc), guest_va);
        crate::backtrace::print_guest_backtrace(state, csrr!(sepc));
        for i in 0..16 {
            print!("x{:<2}={:16x}       ", i*2, trap::get_register(i*2));
            println!("x{:<2}={:16x}", i*2+1, trap::get_register(i*2+1));
        }
        println!("sp={:#x}", trap::get_register(2));
        println!("fp={:#x}", trap::get_register(8));
        // if !state.smode {
        //     crate::pmap::print_guest_page_table((state.satp & SATP_PPN) << 12, 2, 0);
        //     loop {}
        // }

        return false;
    }
}

#[inline(always)]
fn is_uart_access(guest_pa: u64) -> bool {
    guest_pa >= 0x10000000 && guest_pa < 0x10000100
}
unsafe fn handle_uart_access(state: &mut ShadowState, guest_pa: u64, pc: u64) -> bool {
    let (_instruction, decoded, len) = trap::decode_instruction_at_address(state, pc);
    match decoded {
        Some(Instruction::Lb(i)) => trap::set_register(i.rd(), uart_read(state, guest_pa) as u64),
        Some(Instruction::Sb(i)) => uart_write(state, guest_pa, (trap::get_register(i.rs2()) & 0xff) as u8),
        Some(instr) => {
            println!("UART: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, pc);
            loop {}
        }
        _ => return false,
    }
    csrw!(sepc, pc + len);
    true
}

fn uart_read(state: &mut ShadowState, addr: u64) -> u8 {
    match (state.uart_dlab, addr) {
        (false, 0x10000000) => 0,
        (false, 0x10000001) => 0, // Interrupt enable (top four should always be zero)
        (_, 0x10000002) => { // Interrupt identification
            let r = 0xc0 | state.uart_interrupt_id;
            state.uart_interrupt_id = 0;
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
fn uart_write(state: &mut ShadowState, addr: u64, value: u8) {
    match (state.uart_dlab, addr, value) {
        (false, 0x10000000, _) => {
            print::guest_putchar(value);
            if state.uart_interrupt_enable & 0x2 != 0 {
                state.uart_interrupt_id |= 0x2;
                state.plic.set_pending(10, true);
            }
        }
        (true, 0x10000000, _) => {} // DLL divisor latch LSB
        (false, 0x10000001, _) => state.uart_interrupt_enable = value,
        (true, 0x10000001, _) => {} // DLM divisor latch MSB
        (_, 0x10000002, _) => {} // FIFO control
        (_, 0x10000003, _) => state.uart_dlab = (value & 0x80) != 0,
        (_, 0x10000004, _) if value & 0xf0 == 0 => {} // Modem control
        _ => {
            println!("UART: Write unimplemented {:#x} -> {:#x} (dlab={})",
                     value, addr, state.uart_dlab);
            loop {}
        }
    }
}

#[inline(always)]
fn is_plic_access(guest_pa: u64) -> bool {
    guest_pa >= 0x0c000000 && guest_pa < 0x10000000
}
unsafe fn handle_plic_access(state: &mut ShadowState, guest_pa: u64, pc: u64) -> bool {
    let (instruction, decoded, len) = trap::decode_instruction_at_address(state, pc);
    match decoded {
        Some(Instruction::Lw(i)) => {
            let value = state.plic.read_u32(guest_pa) as i32 as i64 as u64;
            // println!("PLIC: Read value {:#x} at address {:#x}", value, guest_pa);
            trap::set_register(i.rd(), value)
        }
        Some(Instruction::Sw(i)) => {
            let value = trap::get_register(i.rs2()) as u32;
            // println!("PLIC: Writing {:#x} to address {:#x}", value, guest_pa);

            let mut clear_seip = false;
            state.plic.write_u32(guest_pa, value, &mut clear_seip);
            if clear_seip {
                state.sip &= !0x200;
            }
            state.no_interrupt = false;
        }
        Some(instr) => {
            println!("PLIC: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, pc);
            loop {}
        }
        _ => {
            println!("Unrecognized instruction targetting PLIC {:#x} at {:#x}!", instruction, pc);
            loop {}
        }
    }
    csrw!(sepc, pc + len);
    true
}
