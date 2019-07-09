#![no_std]
#![feature(asm)]
#![feature(const_slice_len)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(start)]

use rvirt::*;

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { println!("Abort!"); loop {}}

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

    mstart(hartid, device_tree_blob);
}

#[inline(never)]
unsafe fn mstart(hartid: u64, device_tree_blob: u64) {
    csrs!(mideleg, 0x0222);
    csrs!(medeleg, 0xb1ff);
    csrw!(mie, 0x088);
    csrc!(mstatus, STATUS_MPP_M);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, PAYLOAD.as_ptr() as u64);
    csrw!(mcounteren, 0xffffffff);
    csrw!(mscratch, M_MODE_STACK_BASE + M_MODE_STACK_STRIDE * hartid);
    csrw!(pmpaddr0, 0xffffffffffffffff);
    csrw!(pmpcfg0, csrr!(pmpcfg0) | 0x1f);
    csrw!(satp, 0);

    asm!("lla t0, mtrap_entry
          csrw mtvec, t0"
         ::: "t0" : "volatile");

    riscv::sfence_vma();

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
