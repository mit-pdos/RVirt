use crate::context::Context;
use crate::{pmap, trap};

#[allow(unused)]
pub unsafe fn print_guest_backtrace(state: &mut Context, pc: u64) {
    println!(" {:x}", pc);

    let mut ra = trap::get_register(1);
    let mut sp = trap::get_register(2);
    let mut fp = trap::get_register(8);

    let page_table_ppn = state.csrs.satp & trap::constants::SATP_PPN;

    let mut old_fp = 0;
    while old_fp != fp {
        println!(" {:x}", ra);

        ra = match fp.checked_sub(8).and_then(|a| pmap::read64(page_table_ppn, a)) {
            Some(v) => v,
            None => break,
        };

        old_fp = fp;
        fp = match fp.checked_sub(16).and_then(|a| pmap::read64(page_table_ppn, a)) {
            Some(v) => v,
            None => break,
        };
    }

}
