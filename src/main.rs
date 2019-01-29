#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(start)]
#![feature(const_str_len)]
#![feature(proc_macro_hygiene)]

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! {loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! {loop {}}

mod csr;
#[macro_use]
mod riscv;
#[macro_use]
mod print;
mod trap;

const HART_MEM_SIZE: usize = 32 * 1024 * 1024;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
fn _start() {
    unsafe {
        //        asm!("call pmp_init" :::: "volatile");
        asm!("li sp, 0x80100000" :::: "volatile");
    }

    let hartid = reg!(a0);
    let device_tree_blob = reg!(a1);

    // /// Range must be a power of 2 and at least 8.
    // macro_rules! pmp_addr {
    //     ( $base:expr, $range:expr ) => {
    //         $base + ($range - 1) / 8
    //     }
    // }

    // const PMP_R: usize = 0x1;
    // const PMP_W: usize = 0x2;
    // const PMP_X: usize = 0x4;
    // const PMP_NPOT: usize = 0x18;
    // const PMP_LOCK: usize = 0x10;

    // // Code should be execute only
    // csrwi!(pmpaddr0, pmp_addr!(0x80000000, 0x100000));
    // csrsi!(pmpcfg0, PMP_X | PMP_NPOT | PMP_LOCK);

    // // stack and heap should be RW
    // csrw!(pmpaddr1, pmp_addr!(0x80100000 + hartid * HART_MEM_SIZE, HART_MEM_SIZE));
    // csrsi!(pmpcfg0, (PMP_R | PMP_W | PMP_NPOT | PMP_LOCK) << 8);

    // if hartid == 0 {
    //     // DEBUG: RW
    //     csrwi!(pmpaddr2, pmp_addr!(0x0, 0x100));
    //     csrs!(pmpcfg0, (PMP_R | PMP_W | PMP_NPOT | PMP_LOCK) << 16);

    //     // MROM: R
    //     csrwi!(pmpaddr3, pmp_addr!(0x0, 0x20000));
    //     csrsi!(pmpcfg0, (PMP_R | PMP_NPOT | PMP_LOCK) << 24);

    //     // CLINT: RW
    //     csrwi!(pmpaddr4, pmp_addr!(0x2000000, 0x10000));
    //     csrsi!(pmpcfg1, PMP_R | PMP_W | PMP_NPOT | PMP_LOCK);

    //     // PLIC: RW
    //     csrwi!(pmpaddr5, pmp_addr!(0xc000000, 0x4000000));
    //     csrsi!(pmpcfg1, (PMP_R | PMP_W | PMP_NPOT | PMP_LOCK) << 8);
    // }

    // // UART and VIRTIO: RW
    // csrwi!(pmpaddr6, pmp_addr!(0x10000000, 0x1000 * 16));
    // csrsi!(pmpcfg1, (PMP_R | PMP_W | PMP_NPOT | PMP_LOCK) << 16);

    // // Everything else should be inaccessible
    // csrwi!(pmpaddr15, 0x1fffffff);
    // csrsi!(pmpcfg3, (PMP_NPOT | PMP_LOCK) << 24);

    // // Lock the remaining entries
    // csrsi!(pmpcfg0, PMP_LOCK << 24 | PMP_LOCK << 16 | PMP_LOCK << 8 | PMP_LOCK);
    // csrsi!(pmpcfg1, PMP_LOCK << 24 | PMP_LOCK << 16 | PMP_LOCK << 8 | PMP_LOCK);
    // csrsi!(pmpcfg2, PMP_LOCK << 24 | PMP_LOCK << 16 | PMP_LOCK << 8 | PMP_LOCK);
    // csrsi!(pmpcfg3, PMP_LOCK << 24 | PMP_LOCK << 16 | PMP_LOCK << 8 | PMP_LOCK);

    _start2(hartid, device_tree_blob);
}

fn _start2(hartid: usize, _device_tree_blob: usize) {
    for i in 0..10 {
        // let mut enabled = PRINT_LOCK.lock();
        // if !*enabled {
        //     uart::enable();
        //     *enabled = true;
        //     println!("Starting on {}!", hartid);
        // }

        println!("{}: Hello from {}, cycle={:x}", i, hartid, csrr!(cycle));
    }

    csrs!(mideleg, 0x222);
    csrs!(medeleg, 0xb1ff);
    csrs!(sstatus, 0x8);
    csrs!(mstatus, 0x8);
    csrw!(stvec, ((trap::strap_entry as *const () as usize) + 3) & !3);
    csrw!(mtvec, ((trap::mtrap_entry as *const () as usize) + 3) & !3);
    csrw!(sie, 0x888);
    csrw!(mie, 0x888);
    // csrw!(mip, 0x0);

    // // 0x3A0 = pmpcfg0
    // // 0x3B0 = pmppaddr0
    // unsafe {
    //     asm!("li t0, 0x80200fff\n
    //           li t1, 0x90\n
    //           csrw 0x3B0, t0\n
    //           csrw 0x3A0, t1\n" ::: "t0", "t1");
    // }
    // let pmpcfg0: usize;
    // let pmpaddr0: usize;
    // unsafe { asm!("csrr $0, 0x3A0" : "=r"(pmpcfg0)); }
    // unsafe { asm!("csrr $0, 0x3B0" : "=r"(pmpaddr0)); }

    println!("sepc");

//    let msip0 = unsafe { &mut *(0x2000000 as *mut usize) };
//    *msip0 = 1;

    println!("xxx");
    unsafe {

        csrw!(mepc, ((u_entry as *const () as usize) + 3) & !3);
        csrs!(mstatus, 1 << 11);
        asm!("mret" :::: "volatile");
    }
    println!("yyy");
    // println!("pmpaddr0 = {:X}, pmpcfg0 = {:X}", pmpaddr0, pmpcfg0);
    loop {}
}

fn u_entry() {
    println!("000");
    unsafe {
        asm!("mret" :::: "volatile");
        asm!("ecall" :::: "volatile");
    }
    println!("000");
    loop {}
}
