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

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(_info: &::core::panic::PanicInfo) -> ! { loop{} }
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { loop {} }

