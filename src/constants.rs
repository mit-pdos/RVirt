
/// The shift between the physical addresses of symbols and the virtual addresses for those same
/// symbols. This value must match the one used in the linker script (src/linker.ld).
pub const SYMBOL_PA2VA_OFFSET: u64 = 0xffffffff40000000;

/// Maximum number of harts on the host. If the platform has more than this many harts, it might
/// result in buffer overflows in various places.
pub const MAX_HOST_HARTS: usize = 16;

pub const MAX_GUEST_HARTS: usize = 8;

/// Return a reference to a static variable that can be accessed from M-mode.
#[link_section = ".text.init"]
pub fn mstatic<'a, T>(t: &'a T) -> &'a T {
    let address = (t as *const T) as u64;
    unsafe {
        &*((address - SYMBOL_PA2VA_OFFSET) as *const T)
    }
}
