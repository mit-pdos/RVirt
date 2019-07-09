use rvirt::println;

#[no_mangle]
pub fn machine_debug_abort(msg: &str) -> ! {
    println!("{}", msg);
    loop {}
}
#[no_mangle]
pub fn machine_debug_assert(cond: bool, msg: &str) {
    if !cond {
        machine_debug_abort(msg);
    }
}
#[no_mangle]
pub fn machine_debug_puts(s: &str) {
    println!("{}", s);
}
#[no_mangle]
pub fn machine_debug_puthex64(v: u64) {
    println!("{:x}", v);
}
#[no_mangle]
pub fn machine_debug_putint(v: u64) {
    println!("{}", v);
}
#[no_mangle]
pub fn machine_debug_newline() {
    println!("");
}

