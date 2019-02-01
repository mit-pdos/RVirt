#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(start)]
#![feature(const_str_len)]
#![feature(proc_macro_hygiene)]
#![feature(ptr_offset_from)]

#[macro_use]
mod riscv;
#[macro_use]
mod print;

mod csr;
mod fdt;
mod trap;

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! { println!("Abort!"); loop {}}

use fdt::*;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
fn _start() {
    unsafe {
        asm!("li sp, 0x80100000" :::: "volatile");
    }

    let hartid = reg!(a0);
    let device_tree_blob = reg!(a1);
    _start2(hartid, device_tree_blob);
}

fn _start2(_hartid: usize, device_tree_blob: usize) {
    csrs!(mideleg, 0x222);
    csrs!(medeleg, 0xb1ff);
    csrs!(sstatus, 0x8);
    csrs!(mstatus, 0x8);
    csrw!(stvec, ((trap::strap_entry as *const () as usize) + 3) & !3);
    csrw!(mtvec, ((trap::mtrap_entry as *const () as usize) + 3) & !3);
    csrw!(sie, 0x888);
    csrw!(mie, 0x888);

    unsafe {
        let header = Header::new(device_tree_blob);
        assert!(header.magic_valid());
        assert!(header.version() >= 17 && header.last_comp_version() <= 17);
        // header.print();
        let _meta = header.process();
        // header.print();
    }

    unsafe {
        csrw!(mepc, ((u_entry as *const () as usize) + 3) & !3);
//        csrs!(mstatus, 1 << 11);
        asm!("mret" :::: "volatile");
    }
    unreachable!();
}

fn u_entry() {
    println!("000");
    csrw!(sscratch, 0xdeafbeef);
    // println!("..");
    unsafe {
        asm!("ecall" :::: "volatile");
        // asm!("ecall" :::: "volatile");
        // asm!("ecall" :::: "volatile");
    }
    println!("111");
    csrw!(sscratch, 0xdeafbeef);
    println!("000");
    loop {}
}
