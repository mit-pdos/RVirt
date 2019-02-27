use crate::pmap::*;
use crate::trap::{self, ShadowState, constants::SATP_PPN};
use riscv_decode::Instruction;

/// Perform any handling required in response to a guest page fault. Returns true if the fault could
/// be handled, or false if it should be forwarded on to the guest.
pub unsafe fn handle_page_fault(state: &mut ShadowState, cause: usize, pc: u64) -> bool {
    let shadow = state.shadow();
    if shadow == MPA {
        println!("Page fault without guest paging enabled?");
        return false;
    }

    let guest_va = csrr!(stval) as u64;
    assert!((guest_va & SV39_MASK) < (511 << 30));

    let access = match cause {
        12 => PTE_EXECUTE,
        13 => PTE_READ,
        15 => PTE_WRITE,
        _ => unreachable!(),
    };

    let page = guest_va & !0xfff;
    if let Some(translation) = translate_guest_address(((state.satp & SATP_PPN) as u64) << 12, page, AccessType::Read) {
        let guest_pte = *translation.pte;

        // Check R/W/X bits
        if guest_pte & access == 0 {
            println!("Bad access bit guest_va={:#x}, guest_pte={:#x}, cause={}", guest_va, guest_pte, cause);
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
            if (*translation.pte & PTE_DIRTY) == 0 && access == PTE_WRITE {
                *translation.pte = *translation.pte | PTE_DIRTY | PTE_ACCESSED;
            } else if (*translation.pte & PTE_ACCESSED) == 0 {
                *translation.pte = *translation.pte | PTE_ACCESSED;
            }

            let perm = if (*translation.pte & PTE_DIRTY) == 0 && access != PTE_WRITE {
                (guest_pte & (PTE_READ | PTE_EXECUTE))
            } else {
                (guest_pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE))
            };

            *shadow.get_pte(page) = (host_pa >> 2) | perm | PTE_AD | PTE_USER | PTE_VALID;
            return true;
        } else if translation.guest_pa >= 0x10000000 && translation.guest_pa < 0x10000100 && access != PTE_EXECUTE && state.smode {
            return handle_uart_access(state, (translation.guest_pa & !0xfff) | (guest_va & 0xfff), pc);
        } else {
            println!("Guest page table specified invalid guest address, va={:#x} pa={:#x}", guest_va, translation.guest_pa);
            return false;
        }
    } else {
        // println!("satp: {:#x}", state.satp);
        println!("forwarding page fault: \n sepc = {:#x}, stval = {:#x}, stvec = {:#x}",
                 csrr!(sepc) as u64 & SV39_MASK, guest_va & SV39_MASK, state.stvec);
        // print_guest_page_table(((state.satp & SATP_PPN) as u64) << 12, 2, 0);
        return false;
    }

}

unsafe fn handle_uart_access(state: &mut ShadowState, guest_pa: u64, pc: u64) -> bool {
    let (decoded, len) = trap::decode_instruction_at_address(state, pc);
    match decoded {
        Some(Instruction::Lb(i)) => trap::set_register(i.rd(), uart_read(state, guest_pa) as usize),
        Some(Instruction::Sb(i)) => uart_write(state, guest_pa, (trap::get_register(i.rs2()) & 0xff) as u8),
        Some(instr) => {
            println!("UART: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, pc);
            loop {}
        }
        _ => return false,
    }
    csrw!(sepc, (pc + len) as usize);
    true
}

fn uart_read(state: &mut ShadowState, addr: u64) -> u8 {
    match (state.uart_dlab, addr) {
        (false, 0x10000000) => 0,
        (false, 0x10000001) => 0, // Interrupt enable (top four should always be zero)
        (_, 0x10000002) => 0xc0, // Interrupt identification
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
        (false, 0x10000000, _) => print!("{}", value as char),
        (true, 0x10000000, _) => {} // DLL divisor latch LSB
        (false, 0x10000001, 0) => {} // disable interrupts
        (false, 0x10000001, _) => {} // TODO: actually trigger some interrupts as requested
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
