use crate::context::Context;
use crate::memory_region::MemoryRegion;
use crate::riscv::bits;
use crate::pmap;

#[allow(unused)]
pub unsafe fn print_guest_backtrace(guest_memory: &MemoryRegion, state: &mut Context, pc: u64) {
    println!(" {:x}", pc);

    let mut ra = state.saved_registers.get(1);
    let mut sp = state.saved_registers.get(2);
    let mut fp = state.saved_registers.get(8);

    let page_table_ppn = state.csrs.satp & bits::SATP_PPN;

    let mut old_fp = 0;
    while old_fp != fp {
        println!(" {:x}", ra);

        ra = match fp.checked_sub(8).and_then(|a| pmap::read64(guest_memory, page_table_ppn, a)) {
            Some(v) => v,
            None => break,
        };

        old_fp = fp;
        fp = match fp.checked_sub(16).and_then(|a| pmap::read64(guest_memory, page_table_ppn, a)) {
            Some(v) => v,
            None => break,
        };
    }
}
