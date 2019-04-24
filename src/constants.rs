
pub const SYMBOL_PA2VA_OFFSET: u64 = 0xffffffff40000000;
pub const MAX_HOST_HARTS: usize = 16;
pub const MAX_GUEST_HARTS: usize = 1;

/// Return a reference to a static variable that can be accessed from M-mode.
#[link_section = ".text.init"]
pub fn mstatic<'a, T>(t: &'a T) -> &'a T {
    let address = (t as *const T) as u64;
    unsafe {
        &*((address - SYMBOL_PA2VA_OFFSET) as *const T)
    }
}
