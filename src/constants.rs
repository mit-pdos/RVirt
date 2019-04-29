
/// The shift between the physical addresses of symbols and the virtual addresses for those same
/// symbols. This value must match the one used in the linker script (src/linker.ld).
pub const SYMBOL_PA2VA_OFFSET: u64 = 0xffffffff40000000;

/// Maximum number of harts on the host. If the platform has more than this many harts, it might
/// result in buffer overflows in various places.
pub const MAX_HOST_HARTS: usize = 16;

pub const MAX_GUEST_HARTS: usize = 8;

pub const MACHINE_SHARED_STATIC_ADDRESS: u64 = 0x80200000;
pub const SUPERVISOR_SHARED_STATIC_ADDRESS: u64 = 0xffffffffc0200000;
