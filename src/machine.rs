#![no_std]
#![feature(asm)]
#![feature(const_slice_len)]
#![feature(const_str_len)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(linkage)]
#![feature(naked_functions)]
#![feature(proc_macro_hygiene)]
#![feature(ptr_offset_from)]
#![feature(start)]
#![feature(try_blocks)]

use rvirt::*;

pub mod machdebug;
pub mod pagedebug;
pub mod pmp;
pub mod pmptest;

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! { machdebug::machine_debug_abort("panic()!"); loop{} }
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { machdebug::machine_debug_abort("abort()!"); loop {} }

const TEST_PMP: bool = false;

const M_MODE_STACK_BASE: u64 = 0x80810000;
const M_MODE_STACK_STRIDE: u64 = 0x10000;

#[link_section = ".payload"]
static PAYLOAD: [u8; include_bytes!(concat!("../", env!("PAYLOAD"))).len()] =
    *include_bytes!(concat!("../", env!("PAYLOAD")));

global_asm!(include_str!("mcode.S"));

#[naked]
#[no_mangle]
#[link_section = ".text.entrypoint"]
unsafe fn _start(hartid: u64, device_tree_blob: u64) {
    asm!("li sp, $0
          li t1, $1
          mul t0, a0, t1
          add sp, sp, t0" :: "i"(M_MODE_STACK_BASE), "i"(M_MODE_STACK_STRIDE) :: "volatile");

    // Simple trick to loop forever if this hart does not support supervisor mode.
    csrw!(mtvec, 0x80000000);
    csrw!(stvec, 0);

    if TEST_PMP {
        pmptest::pmptest_mstart(hartid, device_tree_blob);
    } else {
        mstart(hartid, device_tree_blob);
    }
}

#[inline(never)]
unsafe fn mstart(hartid: u64, device_tree_blob: u64) {
    // Initialize some control registers
    csrs!(mideleg, 0x0222);
    csrs!(medeleg, 0xb1ff);
    csrw!(mie, 0x088);
    csrc!(mstatus, STATUS_MPP_M);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, PAYLOAD.as_ptr() as u64 - SYMBOL_PA2VA_OFFSET);
    csrw!(mcounteren, 0xffffffff);
    csrw!(mscratch, M_MODE_STACK_BASE + M_MODE_STACK_STRIDE * hartid);

    csrw!(satp, 0);

    pmp::install_pmp_allmem(7, pmp::READ | pmp::WRITE | pmp::EXEC);

    asm!("LOAD_ADDRESS t0, mtrap_entry
              csrw mtvec, t0"
         ::: "t0"  : "volatile");


    // // Text segment
    // pmp::install_pmp_napot(0, pmp::LOCK | pmp::READ | pmp::EXEC, 0x80000000, 0x200000);
    // // Shared data segment
    // pmp::install_pmp_napot(1, pmp::LOCK | pmp::READ | pmp::WRITE, 0x80200000, 0x200000);

    // pmp::debug_pmp();
    // pagedebug::debug_paging();

    enter_supervisor(hartid, device_tree_blob);
}

#[naked]
#[inline(never)]
unsafe fn enter_supervisor(_hartid: u64, _device_tree_blob: u64) {
    asm!("mret" :::: "volatile");
}

#[no_mangle]
pub unsafe fn forward_exception() {
    use crate::riscv::bits::*;

    csrw!(sepc, csrr!(mepc));
    csrw!(scause, csrr!(mcause));
    csrw!(stval, csrr!(mtval));
    csrw!(mepc, csrr!(stvec) & !0x3);

    let status = csrr!(mstatus);
    if status & STATUS_SIE != 0 {
        csrs!(mstatus, STATUS_SPIE);
    } else {
        csrc!(mstatus, STATUS_SPIE);
    }
    if status & STATUS_MPP_S != 0 {
        csrs!(mstatus, STATUS_SPP);
    } else {
        csrc!(mstatus, STATUS_SPP);
    }
    csrc!(mstatus, STATUS_SIE | STATUS_MPP_M);
    csrs!(mstatus, STATUS_MPP_S);
}
