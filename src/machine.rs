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

use rvirt::*;

pub mod machdebug;
pub mod pagedebug;
pub mod pmp;
pub mod pmptest;

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { machdebug::machine_debug_abort("panic()!"); loop{} }
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { machdebug::machine_debug_abort("abort()!"); loop {} }

const TEST_PMP: bool = false;

const M_MODE_STACK_BASE: u64 = 0x80810000;
const M_MODE_STACK_STRIDE: u64 = 0x10000;

const SUPERVISOR_START_ADDRESS: u64 = 0xffffffffc0100000;


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
    csrw!(mepc, SUPERVISOR_START_ADDRESS);
    csrw!(mcounteren, 0xffffffff);
    csrw!(mscratch, M_MODE_STACK_BASE + M_MODE_STACK_STRIDE * hartid);

    csrw!(pmpaddr7, 0xffffffff_ffffffff);
    csrs!(pmpcfg0, 0x1f << 56);

    if SHARED_STATICS.hart_lottery.swap(false,  Ordering::SeqCst) {
        asm!("LOAD_ADDRESS t0, mtrap_entry
              csrw 0x305, t0 // mtvec"
             ::: "t0"  : "volatile");

        // Text segment
        pmp::install_pmp_napot(0, pmp::LOCK | pmp::READ | pmp::EXEC, 0x80000000, 0x200000);
        // Shared data segment
        pmp::install_pmp_napot(1, pmp::LOCK | pmp::READ | pmp::WRITE, 0x80200000, 0x200000);

        // Minimal page table to boot into S mode. See [1] for FU540 errata related to mixing huge
        // pages and PMP.
        //
        // [1] https://github.com/riscv/riscv-isa-manual/issues/347
        let boot_page_table_pa = SHARED_STATICS.boot_page_table.as_ptr() as u64;
        *((boot_page_table_pa) as *mut u64) = 0x00000000 | 0xcf;
        *((boot_page_table_pa+16) as *mut u64) = ((boot_page_table_pa + 4096) >> 2) | 0x01;
        *((boot_page_table_pa+24) as *mut u64) = 0x30000000 | 0xcf;
        *((boot_page_table_pa+4088) as *mut u64) = ((boot_page_table_pa + 4096) >> 2) | 0x01;
        *((boot_page_table_pa+4096) as *mut u64) = 0x20000000 | 0xcb;
        for i in 1..512 {
            *((boot_page_table_pa + 4096 + i*8) as *mut u64) = (0x20000000 + (i<<19)) | 0xc7;
        }
        csrw!(satp, 8 << 60 | (boot_page_table_pa >> 12));

        // pmp::debug_pmp();
        // pagedebug::debug_paging();

        // TODO: figure out why we have to do this dance instead of just assigning things directly
        // i.e. why is it that rust will assign a0/a1? how do we stop that? In the mean time, use
        // the `gp` and `tp` registers at temporaries (the ABI prohibits Rust from passing arguments
        // in them).
        asm!("mv gp, $1
              mv tp, $0
              mv a0, gp
              mv a1, tp
              mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1", "gp", "tp" : "volatile");
    } else  {
        asm!("LOAD_ADDRESS t0, start_hart
             csrw 0x305, t0 // mtvec"
             ::: "t0"  : "volatile");
        csrsi!(mstatus, 0x8); //MIE
        loop {}
    }
}

#[no_mangle]
pub unsafe fn handle_ipi() {
    let hartid = csrr!(mhartid);
    let reason = { SHARED_STATICS.ipi_reason_array.get_unchecked(hartid as usize).lock().take() };

    match reason {
        Some(IpiReason::EnterSupervisor{ a0, a1, a2, a3, sp, satp, mepc}) => {
            csrw!(mepc, mepc);
            csrw!(satp, satp);
            asm!("mv a0, $0
                  mv a1, $1
                  mv a2, $2
                  mv a3, $3
                  mv sp, $4
                  mret" :: "r"(a0), "r"(a1), "r"(a2), "r"(a3), "r"(sp) : "a0", "a1", "a2", "a3", "sp" : "volatile");
        }
        None => {
            machdebug::machine_debug_abort("Got IPI but reason wasn't specified?");
        }
    }
}
