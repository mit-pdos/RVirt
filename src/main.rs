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
mod elf;
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
        let fdt = Fdt::new(device_tree_blob);
        assert!(fdt.magic_valid());
        assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
        // fdt.print();
        let meta = fdt.process();
        // header.print();

        if let (Some(start), Some(_end)) = (meta.initrd_start, meta.initrd_end) {
            println!("Loading guest kernel... {:#x}-{:#x}", start, _end);
            let entry = elf::load_elf(start as *const u8, (meta.hpm_offset + meta.guest_shift) as *mut u8);
            println!("Booting guest kernel...");
            csrw!(mepc, ((entry as usize) + 3) & !3);
        } else {
            csrw!(mepc, ((u_entry as *const () as usize) + 3) & !3);
        }

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
