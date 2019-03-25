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
#![feature(try_blocks)]

#[macro_use]
mod riscv;
#[macro_use]
mod print;

mod backtrace;
mod context;
mod csr;
mod elf;
mod fdt;
mod memory_region;
mod pfault;
mod plic;
mod pmap;
mod sum;
mod trap;
mod virtio;

use fdt::*;
use trap::constants::*;
use pmap::{pa2va, BOOT_PAGE_TABLE};

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! { println!("Abort!"); loop {}}

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
#[inline(never)]
unsafe fn mstart(hartid: u64, device_tree_blob: u64) {
    // Initialize some control registers
    csrs!(mideleg, 0x0222);
    csrs!(medeleg, 0xb1ff);
    csrw!(mie, 0x888);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, sstart as u64);
    csrw!(mcounteren, 0xffffffff);

    if hartid > 0 {
        // TODO: do something useful with extra cores
        loop {}
    }

    asm!("
.align 4
          auipc t0, 0
          c.addi t0, 16
          csrw 0x305, t0 // mtvec
          c.j continue
          c.nop
          c.nop

mtrap_entry:
          csrw 0x340, sp // mscratch
          li sp, 0x80300000
          sd t0, 0(sp)
          sd t1, 8(sp)

          csrr t0, 0x342 // mcause
          li t1, 0x8000000000000003
          beq t0, t1, msoftware_interrupt

          li t1, 0x8000000000000007
          beq t0, t1, mtimer_interrupt

          li t1, 0x800000000000000b
          beq t0, t1, mexternal_interrupt

unknown_cause:
          j unknown_cause

msoftware_interrupt:
          j msoftware_interrupt

mtimer_interrupt:
          li t0, 0x80
          csrc 0x344, t0 // mip.mtip = 0

          li t0, 0x20
          csrs 0x144, t0 // sip.stip = 1

          li t0, 0xffffffff
          li t1, 0x2004000
          sd t0, 0(t1)  // mtimecmp0 = -1

          j return

mexternal_interrupt:
          j mexternal_interrupt

return:
          ld t0, 0(sp)
          ld t1, 8(sp)
          csrr sp, 0x340 // mscratch
          mret
continue:" ::: "t0"  : "volatile");

    // Minimal page table to boot into S mode.
    *({const C: u64 = BOOT_PAGE_TABLE; C} as *mut u64) = 0x00000000 | 0xcf;
    *({const C: u64 = BOOT_PAGE_TABLE+16; C} as *mut u64) = 0x20000000 | 0xcf;
    *({const C: u64 = BOOT_PAGE_TABLE+4088; C} as *mut u64) = 0x20000000 | 0xcf;
    csrw!(satp, 8 << 60 | (BOOT_PAGE_TABLE >> 12));

    asm!("mv a0, $1
          mv a1, $0
          mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1" : "volatile");
}

unsafe fn sstart(_hartid: u64, device_tree_blob: u64) {
    asm!("li t0, 0xffffffff40000000
          add sp, sp, t0" ::: "t0" : "volatile");
    csrw!(stvec, crate::trap::strap_entry as *const () as u64);
    csrw!(sie, 0x222);
    csrs!(sstatus, trap::constants::STATUS_SUM);

    // Read and process host FDT.
    let fdt = Fdt::new(device_tree_blob);
    assert!(fdt.magic_valid());
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    let machine = fdt.process();

    // Initialize memory subsystem.
    let (shadow_page_tables, guest_memory) = pmap::init(&machine);
    let fdt = Fdt::new(pa2va(device_tree_blob));

    // Program PLIC
    for i in 1..127 { // priority
        *(pa2va(0xc000000 + i*4) as *mut u32) = 1;
    }
    *(pa2va(0xc002080) as *mut u32) = 0xfffffffe; // Hart 0 enabled
    *(pa2va(0xc002084) as *mut u32) = !0;         //    .
    *(pa2va(0xc002088) as *mut u32) = !0;         //    .
    *(pa2va(0xc00208c) as *mut u32) = !0;         //    .
    *(pa2va(0x0c201000) as *mut u32) = 0;         // Hart 0 S-mode threshold

    // Load guest binary
    let (entry, max_addr) = sum::access_user_memory(||{
        elf::load_elf(pa2va(machine.initrd_start) as *const u8,
                      pa2va(machine.gpm_offset + machine.guest_shift) as *mut u8)
    });
    let guest_dtb = (max_addr | 0x1fffff) + 1;
    csrw!(sepc, entry);

    // Load and mask guest FDT.
    sum::access_user_memory(||{
        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        pa2va(guest_dtb + machine.guest_shift) as *mut u8,
                        fdt.total_size() as usize);
        let fdt = Fdt::new(pa2va(guest_dtb + machine.guest_shift));
        fdt.process();
    });

    // Initialize context
    context::initialize(&machine, shadow_page_tables, guest_memory);

    // Jump into the guest kernel.
    asm!("mv a1, $0 // dtb = guest_dtb

          li ra, 0
          li sp, 0
          li gp, 0
          li tp, 0
          li t0, 0
          li t1, 0
          li t2, 0
          li s0, 0
          li s1, 0
          li a0, 0  // hartid = 0
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
          sret" :: "r"(guest_dtb) : "memory" : "volatile");

    unreachable!();
}
