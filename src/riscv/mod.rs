#[macro_use]
pub mod instructions;

pub mod csr;
pub mod bits;
pub mod sbi;

pub use instructions::*;

pub const CAUSE_STRINGS: [&str; 16] = [
    "Instruction address misaligned",
    "Instruction access fault",
    "Illegal instruction",
    "Breakpoint",
    "Load address misaligned",
    "Load access fault",
    "Store/AMO address misaligned",
    "Store/AMO access fault",
    "Environment call from U-mode",
    "Environment call from S-mode",
    "Reserved (10)",
    "Environment call from M-mode",
    "Instruction page fault",
    "Load page fault",
    "Reserved (13)",
    "Store/AMO page fault"
];

pub fn cause_to_str(cause: u64) -> &'static str {
    if (cause as i64) < 0 {
        "Interrupt"
    } else if cause >= 16 {
        "Reserved (>=16)"
    } else {
        CAUSE_STRINGS[cause as usize]
    }
}
