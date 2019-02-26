use crate::pmap::*;
use crate::trap::{ShadowState, constants::SATP_PPN};

/// Perform any handling required in response to a guest page fault. Returns true if the fault could
/// be handled, or false if it should be forwarded on to the guest.
pub unsafe fn handle_page_fault(state: &mut ShadowState, cause: usize, _pc: usize) -> bool {
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
        } else {
            println!("Guest page table specified invalid guest address, va={:#x} pa={:#x}", guest_va, translation.guest_pa);
            loop {}
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
