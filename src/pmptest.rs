
use rvirt::*;
use crate::pagedebug;
use crate::pmp;

global_asm!(include_str!("mcode.S"));

#[inline(never)]
pub unsafe fn pmptest_mstart(hartid: u64, device_tree_blob: u64) {
    if hartid > 0 {
        loop {}
    }

    // Initialize some control registers
    csrs!(mideleg, 0x0222);
    csrs!(medeleg, 0xb1ff);
    csrw!(mie, 0x888);
    csrs!(mstatus, STATUS_MPP_S);
    // csrw!(mepc, sstart as u64); -- TODO!!!!!!!!!!!!!!1
    csrw!(mcounteren, 0xffffffff);
    csrw!(mscratch, 0x80800000 + 0x1000 * (hartid+1));

    asm!("LOAD_ADDRESS t0, mtrap_entry
          csrw 0x305, t0 // mtvec"
         ::: "t0"  : "volatile");

    // Minimal page table to boot into S mode.
    let boot_page_table_pa = SHARED_STATICS.boot_page_table.as_ptr() as u64;
    *((boot_page_table_pa) as *mut u64) = 0x00000000 | 0xcf;
    *((boot_page_table_pa+16) as *mut u64) = 0x20000000 | 0xcf;
    *((boot_page_table_pa+4088) as *mut u64) = 0x20000000 | 0xcf;
    csrw!(satp, 8 << 60 | (boot_page_table_pa >> 12));

    // Text segment
    pmp::install_pmp_napot(0, pmp::LOCK | pmp::READ | pmp::EXEC, 0x80000000, 0x200000);
    // Shared data segment
    pmp::install_pmp_napot(1, pmp::LOCK | pmp::READ | pmp::WRITE, 0x80200000, 0x200000);

    // // M-mode stack
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, M_ONLY << 16);
    // csrw!(pmpaddr3, pmpaddr(0x80200000 - (hartid+1) * 64*1024, 32*1024));
    // csrs!(pmpcfg0, LRW << 24);
    // csrw!(pmpaddr2, pmpaddr(0x80180000, 1<<19));
    // csrs!(pmpcfg0, LOCKED << 32);

    pmp::debug_pmp();
    pagedebug::debug_paging();

    asm!("mv a0, $0
          mret" :: "r"(device_tree_blob) : "a0", "a1" : "volatile");
}

// unsafe fn sstart(device_tree_blob: u64) {
//     asm!("li t0, $0
//           add sp, sp, t0" :: "i"(SYMBOL_PA2VA_OFFSET)  : "t0" : "volatile");
//     csrw!(stvec, (||{panic!("Trap on hart 0?!")}) as fn() as *const () as u64);

//     // Read and process host FDT.
//     let fdt = Fdt::new(device_tree_blob);
//     assert!(fdt.magic_valid());
//     assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
//     assert!(fdt.total_size() < 64 * 1024);
//     let machine = fdt.parse();

//     // Initialize UART
//     if let Some(ty) = machine.uart_type {
//         print::UART_WRITER.lock().init(machine.uart_address, ty);
//     }

//     println!("ONLINE");

//     loop {}
// }

// #[allow(unused)]
// unsafe fn ustart() {

// }
