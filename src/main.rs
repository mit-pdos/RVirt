#![no_std]
#![feature(asm)]
#![feature(lang_items)]
#![feature(naked_functions)]
#![feature(start)]

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! {loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] pub fn abort() -> ! {loop {}}

mod uart;

#[naked]
#[no_mangle]
#[link_section = ".text.init"]
fn _start(/*hartid: usize, device_tree_blob: usize*/) {
    unsafe {
        asm!("li sp, 0x800f1000");
    }
    uart::enable();
    for b in "Hello world!\n".bytes() {
        uart::putchar(b);
    }
}


