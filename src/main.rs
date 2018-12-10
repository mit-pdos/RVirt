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
mod uart;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
fn _start() {
    let _hartid: usize;
    let device_tree_blob: usize;
    unsafe {
        asm!("li sp, 0x800f1000");
        asm!("mv $0, a0" : "=r"(_hartid));
        asm!("mv $0, a1" : "=r"(device_tree_blob));
    }

    uart::enable();
    println!("Hello world!");
    println!("dtb = {:X}", device_tree_blob);

    // 0x3A0 = pmpcfg0
    // 0x3B0 = pmppaddr0
    unsafe {
        asm!("li t0, 0x0fffffff\n
              li t1, 0x98\n
              csrw 0x3B0, t0\n
              csrw 0x3A0, t1\n" ::: "t0", "t1");
    }
    let pmpcfg0: usize;
    let pmpaddr0: usize;
    unsafe { asm!("csrr $0, 0x3A0" : "=r"(pmpcfg0)); }
    unsafe { asm!("csrr $0, 0x3B0" : "=r"(pmpaddr0)); }

    println!("pmpaddr0 = {:X}, pmpcfg0 = {:X}", pmpaddr0, pmpcfg0)
}


