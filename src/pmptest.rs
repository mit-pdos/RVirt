
use crate::print;
use crate::trap;
use crate::fdt::*;
use crate::pmap;
use crate::trap::constants::*;
use crate::pmap::{boot_page_table_pa, pa2va};
use crate::pmp;
use crate::machdebug;

#[link_section = ".text.init"]
#[inline(never)]
pub unsafe fn pmptest_mstart(hartid: u64, device_tree_blob: u64) {

    // Initialize some control registers
    csrs!(mideleg, 0x0222);
    csrs!(medeleg, 0xb1ff); // passthrough ALL traps to S-mode code
    csrw!(mie, 0x888);
    csrs!(mstatus, STATUS_MPP_S);
    csrw!(mepc, sstart2 as u64);
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
        loop {}
    }

    machdebug::machine_debug_init();
    pmp::debug_pmp();

    // TODO: figure out why we have to do this dance instead of just assigning things directly
    // i.e. why is it that rust will assign a0/a1? how do we stop that?
    asm!("mv x30, $1
          mv x31, $0
          mv a0, x30
          mv a1, x31
          mret" :: "r"(device_tree_blob), "r"(hartid) : "a0", "a1" : "volatile");
}

unsafe fn sstart2(hartid: u64, device_tree_blob: u64) {
    assert_eq!(hartid, 0);

    asm!("li t0, 0xffffffff40000000
          add sp, sp, t0" ::: "t0" : "volatile");
    csrw!(stvec, crate::trap::strap_entry as *const () as u64);
//    csrw!(sie, 0x222);
    csrs!(sstatus, trap::constants::STATUS_SUM);

    println!("FDT {}\n", device_tree_blob);

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

    println!("ONLINE");

    loop {}
}
