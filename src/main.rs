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
mod pmap;
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

#[no_mangle]
#[link_section = ".htif"]
static mut tohost: u64 = 0;
#[no_mangle]
#[link_section = ".htif"]
static mut fromhost: u64 = 0;

fn _start2(_hartid: usize, device_tree_blob: usize) {
    unsafe { tohost = 1 << 56 | 1 << 48 | 'Y' as u64; }
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
        let machine = fdt.process();
        // header.print();

        pmap::init(&machine);

        if let (Some(start), Some(_end)) = (machine.initrd_start, machine.initrd_end) {
            println!("Loading guest kernel... {:#x}-{:#x}", start, _end);
            let entry = elf::load_elf(start as *const u8, (machine.hpm_offset + machine.guest_shift) as *mut u8);
            println!("Booting guest kernel...");
            csrw!(mepc, ((entry as usize) + 3) & !3);
        } else {
            csrw!(mepc, ((u_entry as *const () as usize) + 3) & !3);
        }

//        csrs!(mstatus, 1 << 11);
        asm!("mv a1, $0
              li ra, 0
              li sp, 0
              li gp, 0
              li tp, 0
              li t0, 0
              li t1, 0
              li t2, 0
              li s0, 0
              li s1, 0
              li a0, 0
              li a2, 0
              li a3, 0
              li a4, 0
              li a5, 0
              li a6, 0
              li a7, 0
              li s2, 0
              li s3, 0
              li s4, 0
              li s5, 0
              li s6, 0
              li s7, 0
              li s8, 0
              li s9, 0
              li s10, 0
              li s11, 0
              li t3, 0
              li t4, 0
              li t5, 0
              li t6, 0
              mret" :: "r"(device_tree_blob) :: "volatile");
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
