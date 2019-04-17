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
mod machdebug;
mod memory_region;
mod pfault;
mod plic;
mod pmap;
mod pmp;
mod sum;
mod trap;
mod virtio;

use fdt::*;
use trap::constants::*;
use pmap::{boot_page_table_pa, pa2va};

/* mandatory rust environment setup */

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! { println!("Abort!"); loop {}}

/* Start-up sequence summary:
 *  - QEMU loads hypervisor kernel (this program), linux kernel, initrd into memory
 *  - QEMU launches hardcoded mrom reset vector, which jumps to 0x80000000,
 *  - _start is located at 0x80000000 as the only function in the .init.entrypoint section
 *  - _start sets up the stack and calls into mstart
 *  - mstart implements the small portion of machine-mode code needed by the hypervisor
 *  - mstart returns into supervisor-mode in sstart
 *  - sstart returns into user-mode at the guest kernel entrypoint
 *        (which is presumably emulated-supervisor-mode)
 */

/* Physical memory layout according to machine-mode
 *   (see also linker.ld, pmap.rs, qemu riscv/virt.c @ 4717595)
 *   note: although only 36 bits are described here, the address space is wider.
 *  START      - END         REGION
 *  0x        0 - 0x      100  QEMU VIRT_DEBUG
 *  0x      100 - 0x     1000  unmapped
 *  0x     1000 - 0x    12000  QEMU MROM (includes hard-coded reset vector; device tree)
 *  0x    12000 - 0x   100000  unmapped
 *  0x   100000 - 0x   101000  QEMU VIRT_TEST
 *  0x   101000 - 0x  2000000  unmapped
 *  0x  2000000 - 0x  2010000  QEMU VIRT_CLINT
 *  0x  2010000 - 0x  3000000  unmapped
 *  0x  3000000 - 0x  3010000  QEMU VIRT_PCIE_PIO
 *  0x  3010000 - 0x  c000000  unmapped
 *  0x  c000000 - 0x 10000000  QEMU VIRT_PLIC
 *  0x 10000000 - 0x 10000100  QEMU VIRT_UART0
 *  0x 10000100 - 0x 10001000  unmapped
 *  0x 10001000 - 0x 10002000  QEMU VIRT_VIRTIO
 *  0x 10002000 - 0x 30000000  unmapped
 *  0x 30000000 - 0x 40000000  QEMU
 *  0x 40000000 - 0x 80000000  QEMU VIRT_PCIE_MMIO
 *  0x 80000000 - 0x 80200000  text segment
 *  0x 80200000 - 0x 80400000  shared data
 *  0x 80400000 - 0x 80600000  hart 0 data segment
 *  0x 80600000 - 0x 80800000  hart 0 S-mode stack
 *  0x c0000000 - 0x c0200000  hart 1 stack
 *  0x c0200000 - 0x c0400000  hart 1 data segment
 *  0x c0400000 - 0x c4000000  hart 1 heap
 *  0x c2000000 - 0x c4000000  hart 1 page tables
 *  0x c4000000 - 0x100000000  hart 1 guest memory
 *  0x100000000 - 0x100200000  hart 2 stack
 *  0x100200000 - 0x100400000  hart 2 data segment
 *  0x100400000 - 0x104000000  hart 2 heap
 *  0x102000000 - 0x104000000  hart 2 page tables
 *  0x104000000 - 0x140000000  hart 2 guest memory
 *  0x140000000 - 0x140200000  hart 3 stack
 *  0x140200000 - 0x140400000  hart 3 data segment
 *  0x140400000 - 0x144000000  hart 3 heap
 *  0x142000000 - 0x144000000  hart 3 page tables
 *  0x144000000 - 0x180000000  hart 3 guest memory
 */

/* Initial supervisor virtual memory layout (boot page table)
 *    note: the Sv39 addressing mode is in use here
 *  VIRTUAL START      - VIRTUAL END          PHYS START   PHYS END     MODE   REGION
 *  0x00000000         - 0x        80000000   0x00000000 - 0x80000000   RWX    QEMU memory sections
 *  0xffffffffc0000000 - 0xffffffffffffffff   0x80000000 - 0xC0000000   RWX    hypervisor memory
 */

#[naked]
#[no_mangle]
#[link_section = ".text.entrypoint"]
unsafe fn _start() {
    asm!("li sp, 0x80a00000
          beqz a0, stack_init_done
          li sp, 0x80200000
          slli t0, a0, 30
          add sp, sp, t0
          stack_init_done: " :::: "volatile");

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
          li sp, 0x80110000
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
          li t0, 0x02000004
          sw zero, 0,(t0)

          csrw 0x341, ra // mepc

          li t0, 0x1000
          csrc 0x300, t0 // mstatus.mpp[1] = 0

          csrr a0, 0xf14 // mhartid

          j return

mtimer_interrupt:
          li t0, 0x80
          csrc 0x344, t0 // mip.mtip = 0

          li t0, 0x20
          csrs 0x144, t0 // sip.stip = 1

          csrr t0, 0xf14 // mhartid
          slli t0, t0, 3
          li t1, 0x2004000
          add t1, t0, t1
          li t0, 0xffffffffffff
          sd t0, 0(t1)  // mtimecmp[hartid] = 2^48 - 1

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
    *((boot_page_table_pa()) as *mut u64) = 0x00000000 | 0xcf;
    *((boot_page_table_pa()+16) as *mut u64) = 0x20000000 | 0xcf;
    *((boot_page_table_pa()+4088) as *mut u64) = 0x20000000 | 0xcf;
    csrw!(satp, 8 << 60 | (boot_page_table_pa() >> 12));

    // Physical Memory Protection
    fn pmpaddr(addr: u64, size: u64) -> u64 {
        assert!(size.is_power_of_two());
        assert!(size >= 16);
        (addr + (size/16 - 1))
    }

    const LXR: u64 = 0x9d; // Lock + Execute + Read
    const LRW: u64 = 0x9b; // Lock + Read + Write

    // Text segment
    csrw!(pmpaddr0, pmpaddr(0x80000000, 2<<20));
    csrs!(pmpcfg0, LXR);

    // Shared data segment
    csrw!(pmpaddr1, pmpaddr(0x80200000, 2<<20));
    csrs!(pmpcfg0, LRW << 8);

    // // M-mode stack
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, M_ONLY << 16);
    // csrw!(pmpaddr3, pmpaddr(0x80200000 - (hartid+1) * 64*1024, 32*1024));
    // csrs!(pmpcfg0, LRW << 24);
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, LOCKED << 32);

    if hartid > 0 {
        let base_address = (1 << 30) * (hartid + 2);
        csrw!(satp, 8 << 60 | (base_address >> 12));
        asm!("mv ra, $0
              mv sp, $1
              mv a1, $2" :: "r"(hart_entry as u64), "r"(base_address + (4<<20) + pmap::DIRECT_MAP_OFFSET), "r"(base_address+4096) :: "volatile");
        csrsi!(mstatus, 0x8); //MIE
        loop {}
    } else {
        asm!("mv a0, $1
              mv a1, $0
              mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1" : "volatile");
    }
}

unsafe fn sstart(hartid: u64, device_tree_blob: u64) {
    assert_eq!(hartid, 0);

    asm!("li t0, 0xffffffff40000000
          add sp, sp, t0" ::: "t0" : "volatile");
    csrw!(stvec, crate::trap::strap_entry as *const () as u64);
//    csrw!(sie, 0x222);
    csrs!(sstatus, trap::constants::STATUS_SUM);

    // Read and process host FDT.
    let fdt = Fdt::new(device_tree_blob);
    assert!(fdt.magic_valid());
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    assert!(fdt.total_size() < 64 * 1024);
    let machine = fdt.parse();

    // Initialize UART
    if let Some(ty) = machine.uart_type {
        print::UART_WRITER.lock().init(machine.uart_address, ty);
    }

    assert!(machine.initrd_end <= machine.physical_memory_offset + pmap::HART_SEGMENT_SIZE);
    assert!(machine.initrd_end - machine.initrd_start <= pmap::HEAP_SIZE);
    if machine.initrd_end == 0 {
        println!("WARN: No guest kernel provided. Make sure to pass one with `-initrd ...`");
    }

    // Initialize memory subsystem.
    pmap::monitor_init();
    let fdt = Fdt::new(pa2va(device_tree_blob));

    // Program PLIC
    for i in 1..127 { // priority
        *(pa2va(machine.plic_address + i*4) as *mut u32) = 1;
    }
    *(pa2va(machine.plic_address + 0x2180) as *mut u32) = 0xfffffffe; // Hart 1 enabled
    *(pa2va(machine.plic_address + 0x2184) as *mut u32) = !0;         //    .
    *(pa2va(machine.plic_address + 0x2188) as *mut u32) = !0;         //    .
    *(pa2va(machine.plic_address + 0x218c) as *mut u32) = !0;         //    .
    *(pa2va(machine.plic_address + 0x203000) as *mut u32) = 0;        // Hart 1 S-mode threshold

    for i in 1..2 {
        let hart_base_pa = machine.physical_memory_offset + i * pmap::HART_SEGMENT_SIZE;

        (*(pa2va(hart_base_pa) as *mut pmap::BootPageTable)).init();
        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        pa2va(hart_base_pa + 4096) as *mut u8,
                        fdt.total_size() as usize);
        core::ptr::copy(pa2va(machine.initrd_start) as *const u8,
                        pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *mut u8,
                        (machine.initrd_end - machine.initrd_start) as usize);

        // Send IPI
        *(pa2va(machine.clint_address + i*4) as *mut u32) = 1;
    }
    loop {}

}

pub unsafe fn hart_entry(hartid: u64, device_tree_blob: u64) {
    csrw!(stvec, crate::trap::strap_entry as *const () as u64);
    csrw!(sie, 0x222);
    csrs!(sstatus, trap::constants::STATUS_SUM);

    let hart_base_pa = (1 << 30) * (hartid + 2);

    // Read and process host FDT.
    let fdt = Fdt::new(pa2va(device_tree_blob));
    assert!(fdt.magic_valid());
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    let machine = fdt.parse();

    // Initialize memory subsystem.
    let (shadow_page_tables, guest_memory, guest_shift) = pmap::init(hart_base_pa, &machine);

    // Load guest binary
    let (entry, max_addr) = sum::access_user_memory(||{
        elf::load_elf(pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *const u8,
                      machine.physical_memory_offset as *mut u8)
    });
    let guest_dtb = (max_addr | 0x1fffff) + 1;
    csrw!(sepc, entry);

    // Load guest FDT.
    sum::access_user_memory(||{
        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        guest_dtb as *mut u8,
                        fdt.total_size() as usize);
        let guest_fdt = Fdt::new(guest_dtb);
        guest_fdt.mask(guest_memory.len());
    });

    // Initialize context
    context::initialize(&machine, shadow_page_tables, guest_memory, guest_shift, hartid);

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
