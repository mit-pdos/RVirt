
//! ## Start-up sequence summary:
//! - QEMU loads hypervisor kernel (this program) and linux kernel (held in initrd) into memory
//! - QEMU launches hardcoded mrom reset vector, which jumps to 0x80000000
//! - _start is located at 0x80000000 as the only function in the .init.entrypoint section
//! - `_start` sets up the stack and calls into mstart
//! - `mstart` initializes machine-mode control registers as needed by the hypervisor
//! - `mstart` returns into supervisor-mode in sstart
//! - `sstart` returns into user-mode at the guest kernel entrypoint
//!       (running in emulated-supervisor-mode)
//!
//! ## Physical memory layout according to machine-mode
//!   (see also linker.ld, pmap.rs, qemu riscv/virt.c @ 4717595)
//!   note: although only 36 bits are described here, the address space is wider.
//! ```text
//!  START      - END         REGION
//!  0x        0 - 0x      100  QEMU VIRT_DEBUG
//!  0x      100 - 0x     1000  unmapped
//!  0x     1000 - 0x    12000  QEMU MROM (includes hard-coded reset vector; device tree)
//!  0x    12000 - 0x   100000  unmapped
//!  0x   100000 - 0x   101000  QEMU VIRT_TEST
//!  0x   101000 - 0x  2000000  unmapped
//!  0x  2000000 - 0x  2010000  QEMU VIRT_CLINT
//!  0x  2010000 - 0x  3000000  unmapped
//!  0x  3000000 - 0x  3010000  QEMU VIRT_PCIE_PIO
//!  0x  3010000 - 0x  c000000  unmapped
//!  0x  c000000 - 0x 10000000  QEMU VIRT_PLIC
//!  0x 10000000 - 0x 10000100  QEMU VIRT_UART0
//!  0x 10000100 - 0x 10001000  unmapped
//!  0x 10001000 - 0x 10002000  QEMU VIRT_VIRTIO
//!  0x 10002000 - 0x 30000000  unmapped
//!  0x 30000000 - 0x 40000000  QEMU
//!  0x 40000000 - 0x 80000000  QEMU VIRT_PCIE_MMIO
//!  0x 80000000 - 0x 80200000  text segment
//!  0x 80200000 - 0x 80400000  shared data
//!  0x 80400000 - 0x 80600000  hart 0 data segment
//!  0x 80600000 - 0x 80800000  hart 0 S-mode stack
//!  0x 80800000 - 0x 80810000  hart 0 M-mode stack
//!  0x 80810000 - 0x 80820000  hart 1 M-mode stack
//!  0x 80820000 - 0x 80830000  hart 2 M-mode stack
//!  0x 80830000 - 0x 80840000  hart 3 M-mode stack
//!  0x 808xxxxx - 0x 808xxxxx  ...
//!  0x 808f0000 - 0x 80900000  hart 15 M-mode stack
//!  0x c0000000 - 0x c0200000  hart 1 stack
//!  0x c0200000 - 0x c0400000  hart 1 data segment
//!  0x c0400000 - 0x c4000000  hart 1 heap
//!  0x c2000000 - 0x c4000000  hart 1 page tables
//!  0x c4000000 - 0x100000000  hart 1 guest memory
//!  0x100000000 - 0x100200000  hart 2 stack
//!  0x100200000 - 0x100400000  hart 2 data segment
//!  0x100400000 - 0x104000000  hart 2 heap
//!  0x102000000 - 0x104000000  hart 2 page tables
//!  0x104000000 - 0x140000000  hart 2 guest memory
//!  0x140000000 - 0x140200000  hart 3 stack
//!  0x140200000 - 0x140400000  hart 3 data segment
//!  0x140400000 - 0x144000000  hart 3 heap
//!  0x142000000 - 0x144000000  hart 3 page tables
//!  0x144000000 - 0x180000000  hart 3 guest memory
//! ```
//!
//! ## Initial supervisor virtual memory layout (boot page table)
//!    note: the Sv39 addressing mode is in use here
//! ```text
//!  VIRTUAL START      - VIRTUAL END          PHYS START   PHYS END     MODE   REGION
//!  0x        00000000 - 0x        40000000   0x00000000 - 0x40000000   RWX    QEMU memory sections
//!  0x        80000000 - 0x        c0000000   0x80000000 - 0xC0000000   RWX    hypervisor memory
//!  0xffffffffc0000000 - 0xffffffffffffffff   0x80000000 - 0xC0000000   RWX    hypervisor memory
//! ```
//!
//! ## Linux address space layout (with Sv39 addressing)
//!
//! In this addressing mode, Linux does not reserve any address space for a hypervisor. However, the
//! direct map region is 128GB (one quarter of the addres space) but physical memory takes up at
//! most a handful of GBs and Linux never accesses any higher addresses. Thus rvirt is able to use
//! the top 16GB of virtual addresses for its own code and data.
//!
//! ```text
//!  VIRTUAL START      - VIRTUAL END          REGION
//!  0x0000000000000000 - 0x0000003fffffffff   User memory
//!  0xffffffbfffffffff - 0xffffffdfffffffff   Kernel memory
//!  0xffffffdfffffffff - 0xffffffffffffffff   Direct map region
//! ```

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
mod constants;
mod context;
mod csr;
mod elf;
mod fdt;
mod ipi;
mod machdebug;
mod memory_region;
mod pfault;
mod plic;
mod pmap;
mod pmp;
mod pmptest;
mod sum;
mod trap;
mod virtio;

use core::sync::atomic::{AtomicBool, Ordering};
use constants::mstatic;
use fdt::*;
use ipi::{REASON_ARRAY, Reason};
use trap::constants::*;
use pmap::{boot_page_table_pa, pa2va};
use pmptest::pmptest_mstart;

global_asm!(include_str!("mcode.S"));

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { println!("Abort!"); loop {}}

const TEST_PMP: bool = false;

/// First Hart to set this to false gets to run domain 0.
static HART_LOTTERY: AtomicBool = AtomicBool::new(true);

#[naked]
#[no_mangle]
#[link_section = ".text.entrypoint"]
unsafe fn _start() {
    asm!("li t0 0x80000000
          csrw mtvec, t0
          csrw stvec, zero
          li sp, 0x80810000
          slli t0, a0, 16
          add sp, sp, t0" :::: "volatile");

    let hartid = reg!(a0);
    let device_tree_blob = reg!(a1);
    if TEST_PMP {
        pmptest_mstart(hartid, device_tree_blob);
    } else {
        mstart(hartid, device_tree_blob);
    }
}

#[link_section = ".text.init"]
#[inline(never)]
unsafe fn mstart(hartid: u64, device_tree_blob: u64) {
    use crate::machdebug::*;
    // Initialize some control registers
    // csrw!(medeleg, 0xb1ff);
    // csrw!(mideleg, 0x0222);
    csrw!(mie, 0/*0x888*/);
    csrw!(sie, 0);
    csrc!(mstatus, STATUS_MPP_M);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, sstart as u64);
    csrw!(mcounteren, 0xffffffff);
    csrw!(mscratch, 0x80810000 + 0x10000 * hartid);

    // machine_debug_puts("A\r\n");

    csrw!(medeleg, 0);
    csrw!(mideleg, 0);

    asm!("LOAD_ADDRESS t0, mtrap_entry
          csrw 0x305, t0 // mtvec"
         ::: "t0"  : "volatile");

    csrw!(pmpaddr0, 0xffffffff_ffffffff);
    csrw!(pmpcfg0, 0x1f);
    // machine_debug_puts("A\r\n");

    // Minimal page table to boot into S mode.
    *((boot_page_table_pa()) as *mut u64) = 0x00000000 | 0xcf;
    *((boot_page_table_pa()+8) as *mut u64) = 0x10000000 | 0xcf;
    *((boot_page_table_pa()+16) as *mut u64) = 0x20000000 | 0xcf;
    *((boot_page_table_pa()+24) as *mut u64) = 0x30000000 | 0xcf;
    *((boot_page_table_pa()+4088) as *mut u64) = 0x20000000 | 0xcf;
    csrw!(satp, 8 << 60 | (boot_page_table_pa() >> 12));

    // Text segment
    pmp::install_pmp_napot(0, pmp::LOCK | pmp::READ | pmp::EXEC, 0x80000000, 0x200000);
    // Shared data segment
    pmp::install_pmp_napot(1, pmp::LOCK | pmp::READ | pmp::WRITE, 0x80200000, 0x200000);
    // Everything else unrestricted
    pmp::install_pmp_napot(2, pmp::READ | pmp::WRITE | pmp::EXEC, 0, 16 << 30);

    // // M-mode stack
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, M_ONLY << 16);
    // csrw!(pmpaddr3, pmpaddr(0x80200000 - (hartid+1) * 64*1024, 32*1024));
    // csrs!(pmpcfg0, LRW << 24);
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, LOCKED << 32);

    if mstatic(&HART_LOTTERY).swap(false,  Ordering::SeqCst) {
        asm!("mv a0, $1
              mv a1, $0
              mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1" : "volatile");
    } else  {
        asm!("LOAD_ADDRESS t0, start_hart
             csrw 0x305, t0 // mtvec"
             ::: "t0"  : "volatile");
        csrsi!(mstatus, 0x8); //MIE
        loop {}
    }
}

unsafe fn sstart(hartid: u64, device_tree_blob: u64) {
    asm!("li t0, 0xffffffff40000000
          add sp, sp, t0" ::: "t0" : "volatile");
    csrw!(stvec, (||{
        println!("scause={:x}", csrr!(scause));
        println!("sepc={:x}", csrr!(sepc));
        panic!("Trap on dom0 hart?!")
    }) as fn() as *const () as u64);

    let fdt = Fdt::new(device_tree_blob);
    println!("B");
    assert!(fdt.magic_valid());
    println!("C");
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    println!("D");
    assert!(fdt.total_size() < 64 * 1024);
    println!("E");
    let machine = fdt.parse();
    fdt.print();

    // Initialize UART
    if let Some(ty) = machine.uart_type {
        print::UART_WRITER.lock().init(machine.uart_address, ty);
    }

    println!("B: Hello world!");

    // Do some sanity checks now that the UART is initialized and we have a better chance of
    // successfully printing output.
    assert!(machine.initrd_end <= machine.physical_memory_offset + pmap::HART_SEGMENT_SIZE);
    assert!(machine.initrd_end - machine.initrd_start <= pmap::HEAP_SIZE);
    if machine.initrd_end == 0 {
        println!("WARN: No guest kernel provided. Make sure to pass one with `-initrd ...`");
    }

    // Initialize memory subsystem.
    pmap::monitor_init();
    println!("A");
    let fdt = Fdt::new(pa2va(device_tree_blob));
    println!("B");

    // Program PLIC priorities
    for i in 1..127 {
        *(pa2va(machine.plic_address + i*4) as *mut u32) = 1;
    }
    println!("C");

    let mut guestid = 1;
    for &Hart { hartid, plic_context } in machine.harts.iter().filter(|h| h.hartid != hartid) {
        let hart_base_pa = machine.physical_memory_offset + pmap::HART_SEGMENT_SIZE * guestid;

        let mut irq_mask = 0;
        for j in 0..4 {
            let index = ((guestid-1) * 4 + j) as usize;
            if index < machine.virtio.len() {
                let irq = machine.virtio[index].irq;
                assert!(irq < 32);
                irq_mask |= 1u32 << irq;
            }
        }

        *(pa2va(machine.plic_address + 0x200000 + 0x1000 * plic_context) as *mut u32) = 0;
        *(pa2va(machine.plic_address + 0x2000 + 0x80 * plic_context) as *mut u32) = irq_mask;

        (*(pa2va(hart_base_pa) as *mut pmap::BootPageTable)).init();
        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        pa2va(hart_base_pa + 4096) as *mut u8,
                        fdt.total_size() as usize);
        core::ptr::copy(pa2va(machine.initrd_start) as *const u8,
                        pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *mut u8,
                        (machine.initrd_end - machine.initrd_start) as usize);

        // Send IPI
        *REASON_ARRAY[hartid as usize].lock() = Some(Reason::EnterSupervisor {
            a0: hartid,
            a1: hart_base_pa + 4096,
            a2: hart_base_pa,
            a3: guestid as u64,
            sp: hart_base_pa + (4<<20) + pmap::DIRECT_MAP_OFFSET,
            satp: 8 << 60 | (hart_base_pa >> 12),
            mepc: hart_entry as u64,
        });
        *(pa2va(machine.clint_address + hartid*4) as *mut u32) = 1;

        guestid += 1;
    }
    loop {}
}

#[no_mangle]
unsafe fn hart_entry(hartid: u64, device_tree_blob: u64, hart_base_pa: u64, guestid: u64) {
    csrw!(stvec, crate::trap::strap_entry as *const () as u64);
    csrw!(sie, 0x222);
    csrs!(sstatus, trap::constants::STATUS_SUM);

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
    let guest_machine = sum::access_user_memory(||{
        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        guest_dtb as *mut u8,
                        fdt.total_size() as usize);
        let guest_fdt = Fdt::new(guest_dtb);
        guest_fdt.mask(guest_memory.len());
        guest_fdt.parse()
    });

    // Initialize context
    context::initialize(&machine, &guest_machine, shadow_page_tables, guest_memory, guest_shift, hartid, guestid);

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
