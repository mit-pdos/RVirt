
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
#![feature(const_fn)]
#![feature(const_raw_ptr_deref)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(linkage)]
#![feature(naked_functions)]
#![feature(proc_macro_hygiene)]
#![feature(ptr_offset_from)]
#![feature(start)]
#![feature(try_blocks)]

#[macro_use]
pub mod riscv;
#[macro_use]
pub mod print;

pub mod backtrace;
pub mod constants;
pub mod context;
pub mod drivers;
pub mod elf;
pub mod fdt;
pub mod memory_region;
pub mod pfault;
pub mod plic;
pub mod pmap;
pub mod statics;
pub mod sum;
pub mod trap;
pub mod virtio;

pub use core::sync::atomic::{AtomicBool, Ordering};
pub use constants::SYMBOL_PA2VA_OFFSET;
pub use fdt::*;
pub use riscv::bits::*;
pub use pmap::{pa2va};
pub use statics::{__SHARED_STATICS_IMPL, IpiReason, SHARED_STATICS};
