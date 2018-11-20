#![no_std]
#![feature(lang_items)]
#![feature(start)]

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! {loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize { 0 }

#[link_section = ".text.init"]
#[no_mangle]
fn _start() {
    let mut i = 21;
    loop {
        if i % 2 == 0 {
            i = i / 2;
        } else {
            i = i * 3 + 1;
        }
    }
}
