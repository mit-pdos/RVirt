#![no_std]
#![feature(asm)]
#![feature(const_str_len)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(linkage)]
#![feature(naked_functions)]
#![feature(proc_macro_hygiene)]
#![feature(ptr_offset_from)]
#![feature(start)]

#[macro_use]
mod riscv;
#[macro_use]
mod print;

mod csr;
mod elf;
mod fdt;
mod pmap;
mod trap;

use fdt::*;
use trap::constants::*;

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! { println!("Abort!"); loop {}}

extern {
    static mtrap_entry_offset: usize;
}

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
unsafe fn _start() {
    asm!("li sp, 0x80100000" :::: "volatile");

    let hartid = reg!(a0);
    let device_tree_blob = reg!(a1);
    mstart(hartid, device_tree_blob);
}

#[link_section = ".text.init"]
unsafe fn mstart(hartid: usize, device_tree_blob: usize) {
    // Initialize some control registers
    csrs!(mideleg, 0x222);
    csrs!(medeleg, 0xb1ff);
    csrw!(mtvec, mtrap_entry_offset + 0x80000000);
    csrw!(mie, 0x888);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, sstart as usize);

    // Minimal page table to boot into S mode.
    *((pmap::BOOT_PAGE_TABLE + 0) as *mut u64) = 0x00000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 8) as *mut u64) = 0x20000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 16) as *mut u64) = 0x20000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 24) as *mut u64) = 0x30000000 | 0xcf;
    csrw!(satp, 8 << 60 | (pmap::BOOT_PAGE_TABLE >> 12) as usize);

    asm!("mv a0, $1
          mv a1, $0
          mret" :: "r"(device_tree_blob), "r"(hartid) :: "volatile");
}

fn sstart(_hartid: usize, device_tree_blob: usize) {
   csrw!(stvec, crate::trap::strap_entry as *const () as usize + pmap::HVA_TO_XVA as usize);
   csrw!(sie, 0x888);
   println!("Hello World!");

    unsafe {
        let fdt = Fdt::new(device_tree_blob);
        assert!(fdt.magic_valid());
        assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
        // fdt.print();
        let machine = fdt.process();
        // header.print();

        println!("FDT loaded");
        pmap::init(&machine);
        println!("Memory initialized");

        // pmap::print_page_table(pmap::MPA_ROOT, 3);

        if let (Some(start), Some(_end)) = (machine.initrd_start, machine.initrd_end) {
            println!("Loading guest kernel... {:#x}-{:#x}", start, _end);
            let entry = elf::load_elf((start + pmap::HPA_OFFSET) as *const u8,
                                      (machine.hpm_offset + machine.guest_shift + pmap::HPA_OFFSET) as *mut u8);
            csrw!(sepc, ((entry as usize) + 3) & !3);

        } else {
            // TODO: proper length
            core::ptr::copy(u_entry as *const u8, (pmap::MPA_OFFSET + 0x80000000) as *mut u8, 0x10000);
            csrw!(sepc, 0x80000000);
        }

        asm!("auipc t0, 0
              add t1, t0, $0
              jr t1" :: "r"(pmap::HVA_TO_XVA + 10) : "t0", "t1" : "volatile");

        println!("Booting guest...");

        csrw!(satp, pmap::MPA_SATP);
        asm!("sfence.vma" :::: "volatile");

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
              sret" :: "r"(device_tree_blob) :: "volatile");
    }

    unreachable!();
}

#[naked]
fn u_entry() {
    unsafe { asm!("li sp, 0x80100000" :::: "volatile"); }
    csrw!(sscratch, 0xdeafbeef);

    // println!("000");
    // println!("..");
//    unsafe {
//        asm!("ecall" :::: "volatile");
        // asm!("ecall" :::: "volatile");
        // asm!("ecall" :::: "volatile");
//    }
    // println!("111");
    csrw!(sscratch, 0xdeafbeef);
    // println!("222");
    loop {}
}
