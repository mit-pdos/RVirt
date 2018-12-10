#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(start)]

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! {loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! {loop {}}

#[macro_use]
mod print;
mod spin;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
fn _start() {
    let hartid: usize;
    let _device_tree_blob: usize;
    unsafe {
        asm!("slli t0, a0, 12
              li t1, 0x800f1000
              add sp, t0, t1" ::: "t0", "t1" : "volatile");
        asm!("mv $0, a0" :"=r"(hartid) ::: "volatile");
        asm!("mv $0, a1" : "=r"(_device_tree_blob) ::: "volatile");
    }

    for i in 0..10 {
        // let mut enabled = PRINT_LOCK.lock();
        // if !*enabled {
        //     uart::enable();
        //     *enabled = true;
        //     println!("Starting on {}!", hartid);
        // }

        println!("{}: Hello from {}", i, hartid);
    }
    // // 0x3A0 = pmpcfg0
    // // 0x3B0 = pmppaddr0
    // unsafe {
    //     asm!("li t0, 0x0fffffff\n
    //           li t1, 0x98\n
    //           csrw 0x3B0, t0\n
    //           csrw 0x3A0, t1\n" ::: "t0", "t1");
    // }
    // let pmpcfg0: usize;
    // let pmpaddr0: usize;
    // unsafe { asm!("csrr $0, 0x3A0" : "=r"(pmpcfg0)); }
    // unsafe { asm!("csrr $0, 0x3B0" : "=r"(pmpaddr0)); }

    // println!("pmpaddr0 = {:X}, pmpcfg0 = {:X}", pmpaddr0, pmpcfg0)
    loop {}
}


