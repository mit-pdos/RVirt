use crate::context::Context;
use crate::trap::{self, constants::SATP_PPN};
use crate::{pmap::*, riscv, virtio};
use riscv_decode::Instruction;

/// Perform any handling required in response to a guest page fault. Returns true if the fault could
/// be handled, or false if it should be forwarded on to the guest.
pub fn handle_page_fault(state: &mut Context, cause: u64, instruction: Option<u32>) -> bool {
    let shadow = state.shadow();
    if shadow == PageTableRoot::MPA {
        println!("Page fault without guest paging enabled?");
        return false;
    }

    let guest_va = csrr!(stval);
    //assert!((guest_va & SV39_MASK) < (511 << 30));

    let access = match cause {
        12 => PTE_EXECUTE,
        13 => PTE_READ,
        15 => PTE_WRITE,
        _ => unreachable!(),
    };

    let page = guest_va & !0xfff;
    if let Some(translation) = translate_guest_address(&state.guest_memory, (state.csrs.satp & SATP_PPN) << 12, page) {
        // Check R/W/X bits
        if translation.pte_value & access == 0 {
            return false;
        }

        // Check U bit
        match shadow {
            PageTableRoot::UVA => if translation.pte_value & PTE_USER == 0 { return false; }
            PageTableRoot::KVA => if translation.pte_value & PTE_USER != 0 { return false; }
            PageTableRoot::MVA => {}
            _ => unreachable!(),
        }

        if state.guest_memory.in_region(translation.guest_pa) {
            let host_pa = translation.guest_pa + state.guest_shift;

            // Set A and D bits
            let new_pte = if (translation.pte_value & PTE_DIRTY) == 0 && access == PTE_WRITE {
                translation.pte_value | PTE_DIRTY | PTE_ACCESSED
            } else if (translation.pte_value & PTE_ACCESSED) == 0 {
                translation.pte_value | PTE_ACCESSED
            } else {
                translation.pte_value
            };

            if new_pte != translation.pte_value {
                // TODO: do this atomically
                state.guest_memory[translation.pte_addr] = new_pte;
            }

            let perm = if (new_pte & PTE_DIRTY) == 0 && access != PTE_WRITE {
                (new_pte & (PTE_READ | PTE_EXECUTE))
            } else {
                (new_pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE))
            };

            if virtio::is_queue_access(state, translation.guest_pa) {
                let guest_pa = (translation.guest_pa & !0xfff) | (guest_va & 0xfff);
                let host_pa = (host_pa & !0xfff) | (guest_va & 0xfff);
                let instruction = instruction.expect("attempted to execute code from virtio queue page");
                return virtio::handle_queue_access(state, guest_pa, host_pa, instruction);
            }

            let reserved_bits = match translation.level {
                PageTableLevel::Level4KB => 0x000,
                PageTableLevel::Level2MB => 0x100,
                PageTableLevel::Level1GB => 0x200,
            };

            state.shadow_page_tables.set_mapping(
                shadow, page, (host_pa >> 2) | reserved_bits | perm | PTE_AD | PTE_USER | PTE_VALID);
            riscv::sfence_vma_addr(guest_va);
            return true;
        } else if access != PTE_EXECUTE && state.smode {
            let pa = (translation.guest_pa & !0xfff) | (guest_va & 0xfff);
            if let Some(instruction) = instruction {
                if is_uart_access(pa) {
                    return handle_uart_access(state, pa, instruction);
                }

                if is_plic_access(pa) {
                    return handle_plic_access(state, pa, instruction)
                }

                if virtio::is_device_access(state, pa) {
                    return virtio::handle_device_access(state, pa, instruction);
                }
            }
        }
    }

    false
}

#[inline(always)]
fn is_uart_access(guest_pa: u64) -> bool {
    guest_pa >= 0x10000000 && guest_pa < 0x10000100
}
fn handle_uart_access(state: &mut Context, guest_pa: u64, instruction: u32) -> bool {
    match riscv_decode::decode(instruction).ok() {
        Some(Instruction::Lb(i)) => {
            let value = state.uart.read(&state.host_clint, guest_pa) as u64;
            trap::set_register(state, i.rd(), value);
        }
        Some(Instruction::Sb(i)) => {
            let value = (trap::get_register(state, i.rs2()) & 0xff) as u8;
            state.uart.write(&state.host_clint, guest_pa, value);
        }
        Some(instr) => {
            println!("UART: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, csrr!(sepc));
            loop {}
        }
        _ => return false,
    }
    riscv::set_sepc(csrr!(sepc) + riscv_decode::instruction_length(instruction as u16) as u64);
    true
}

#[inline(always)]
fn is_plic_access(guest_pa: u64) -> bool {
    guest_pa >= 0x0c000000 && guest_pa < 0x10000000
}
fn handle_plic_access(state: &mut Context, guest_pa: u64, instruction: u32) -> bool {
    match riscv_decode::decode(instruction).ok() {
        Some(Instruction::Lw(i)) => {
            let value = state.plic.read_u32(guest_pa) as i32 as i64 as u64;
            // println!("PLIC: Read value {:#x} at address {:#x}", value, guest_pa);
            trap::set_register(state, i.rd(), value)
        }
        Some(Instruction::Sw(i)) => {
            let value = trap::get_register(state, i.rs2()) as u32;
            // println!("PLIC: Writing {:#x} to address {:#x}", value, guest_pa);

            let mut clear_seip = false;
            state.plic.write_u32(guest_pa, value, &mut clear_seip);
            if clear_seip {
                state.csrs.sip &= !0x200;
            }
            state.no_interrupt = false;
        }
        Some(instr) => {
            println!("PLIC: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, csrr!(sepc));
            loop {}
        }
        _ => {
            println!("Unrecognized instruction targetting PLIC {:#x} at {:#x}!", instruction, csrr!(sepc));
            loop {}
        }
    }
    riscv::set_sepc(csrr!(sepc) + riscv_decode::instruction_length(instruction as u16) as u64);
    true
}
