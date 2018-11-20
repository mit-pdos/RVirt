#![no_std]
#![feature(lang_items)]
#![feature(start)]

#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! {loop {}}
#[no_mangle] fn abort(){ loop{} }

#[start]
fn start(argc: isize, argv: *const *const u8) -> isize { 0 }

#[link_section = ".text.init"]
#[no_mangle]
fn entry() {
    let mut i = 21;
    loop {
        if i % 2 == 0 {
            i = i / 2;
        } else {
            i = i * 3 + 1;
        }
    }
}
