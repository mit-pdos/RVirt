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
mod pfault;
mod plic;
mod pmap;
mod trap;
mod virtio;

use fdt::*;
use trap::constants::*;

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

    asm!("auipc t0, 0
          c.addi t0, 18
          csrw 0x305, t0 // mtvec
          c.j continue

.align 4
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
    *((pmap::BOOT_PAGE_TABLE + 0) as *mut u64) = 0x00000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 8) as *mut u64) = 0x20000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 16) as *mut u64) = 0x20000000 | 0xcf;
    *((pmap::BOOT_PAGE_TABLE + 24) as *mut u64) = 0x30000000 | 0xcf;
    csrw!(satp, 8 << 60 | (pmap::BOOT_PAGE_TABLE >> 12));

    asm!("mv a0, $1
          mv a1, $0
          mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1" : "volatile");
}

fn sstart(_hartid: u64, device_tree_blob: u64) {
    csrw!(stvec, crate::trap::strap_entry as *const () as u64 + pmap::HVA_TO_XVA);
    csrw!(sie, 0x222);

    unsafe {
        // Read and process host FDT.
        let fdt = Fdt::new(device_tree_blob);
        assert!(fdt.magic_valid());
        assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
        let machine = fdt.process();

        // Initialize memory subsystem.
        pmap::init(&machine);
        asm!("li t0, 0x40000000
              sub sp, sp, t0" ::: "t0" : "volatile");
        pmap::init2();

        let fdt = Fdt::new(pmap::pa2va(device_tree_blob));

        // Load guest binary
        let entry;
        let guest_dtb;
        if let (Some(start), Some(_end)) = (machine.initrd_start, machine.initrd_end) {
            let ret = elf::load_elf((start + pmap::HPA_OFFSET) as *const u8,
                                    (machine.hpm_offset + machine.guest_shift + pmap::HPA_OFFSET) as *mut u8);
            entry = ret.0;
            guest_dtb = (ret.1 | 0x1fffff) + 1;

        } else {
            println!("No guest kernel provided. Make sure to pass one with `-initrd ...`");
            loop {}
        }
        csrw!(sepc, entry);

        // Load and mask guest FDT.
        core::ptr::copy((device_tree_blob + pmap::HPA_OFFSET) as *const u8,
                        pmap::MPA.address_to_pointer(guest_dtb),
                        fdt.total_size() as usize);
        let fdt = Fdt::new(pmap::MPA.address_to_pointer::<u8>(guest_dtb) as u64);
        fdt.process();

        for i in 1..127 { // priority
            *(pmap::pa2va(0xc000000 + i*4) as *mut u32) = 1;
        }
        // *((0xc000000 + 7*4) as *mut u32) = 1;
        // *((0xc000000 + 10*4) as *mut u32) = 3;
        // for i in 0..4 { // Hart 0 M-mode enables
        //     *((0xc002000 + i*4) as *mut u32) = !0;
        // }

        *(pmap::pa2va(0xc002080) as *mut u32) = 0xfffffffe;
        *(pmap::pa2va(0xc002084) as *mut u32) = !0;
        *(pmap::pa2va(0xc002088) as *mut u32) = !0;
        *(pmap::pa2va(0xc00208c) as *mut u32) = !0;

        // *((0xc002080) as *mut u32) = 0x80;
        // *((0xc002084) as *mut u32) = 0;
        // *((0xc002088) as *mut u32) = 0;
        // *((0xc00208c) as *mut u32) = 0;

        // *(0x0c200000 as *mut u32) = 0; // Hart 0 M-mode threshold
        *(pmap::pa2va(0x0c201000) as *mut u32) = 0; // Hart 0 S-mode threshold
        // *(0x0c202000 as *mut u32) = 0; // Hart 0 S-mode threshold
        // *(0x0c203000 as *mut u32) = 0; // Hart 0 S-mode threshold
        // asm!("fence" :::: "volatile");
        // *(0x0c01000 as *mut u32) = 1; // Hart 0 S-mode threshold

        // for i in 1..=8 {
        //     let addr = 0x10000000 + 0x1000 * i;
        //     println!("ADDR = {:#x}", addr);
        //     println!("  magic={:#x}", *(addr as *const u32));
        //     println!("  version={:#x}", *((addr+4) as *const u32));
        //     println!("  type={:#x}", *((addr+8) as *const u32));
        // }

        // Jump into the guest kernel.
        //
        // First we set a1 with a pointer to the device tree block. Ideally the preceeding moves
        // into s1..s3 shouldn't be necessary, but LLVM doesn't seem to be honoring the listed
        // clobber registers and insists on passing one of the inputs in a1 so we have to save the
        // inputs before setting a1 to $2. Next we jump to high addresses (offset passed in
        // $0). After that we install guest page tables (satp passed in $1) and do a TLB
        // flush. Finally, we clear out all remaining registers and issue an sret.
        asm!("mv s0, $0
              mv s1, $1
              mv s2, $2

              mv a1, s2

              auipc t0, 0
              add t1, t0, s0
              jr t1

              csrw 0x180, s1
              sfence.vma

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
              sret" :: "r"(pmap::HVA_TO_XVA + 10), "r"(pmap::MPA.satp()), "r"(guest_dtb) : "s0", "s1", "s2", "memory" : "volatile");
    }

    unreachable!();
}
