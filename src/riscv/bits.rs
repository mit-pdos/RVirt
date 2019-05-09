pub const TVEC_MODE: u64 = 0x3;
pub const TVEC_BASE: u64 = !TVEC_MODE;

pub const STATUS_UIE: u64 = 1 << 0;
pub const STATUS_SIE: u64 = 1 << 1;
pub const STATUS_UPIE: u64 = 1 << 4;
pub const STATUS_SPIE: u64 = 1 << 5;
pub const STATUS_SPP: u64 = 1 << 8;
pub const STATUS_FS: u64 = 3 << 13;
pub const STATUS_XS: u64 = 3 << 15;
pub const STATUS_SUM: u64 = 1 << 18;
pub const STATUS_MXR: u64 = 1 << 19;
pub const STATUS_SD: u64 = 1 << 63;

pub const STATUS_MPP_M: u64 = 3 << 11;
pub const STATUS_MPP_S: u64 = 1 << 11;
pub const STATUS_MPP_U: u64 = 0 << 11;

// Mask of writable bits in sstatus.
pub const SSTATUS_WRITABLE_MASK: u64 =
    STATUS_MXR |
STATUS_SUM |
STATUS_FS |
STATUS_SPP |
STATUS_SPIE |
STATUS_SIE;
pub const SSTATUS_DYNAMIC_MASK: u64 = STATUS_SD | STATUS_FS;

pub const IP_SSIP: u64 = 1 << 1;
pub const IP_STIP: u64 = 1 << 5;
pub const IP_SEIP: u64 = 1 << 9;

pub const IE_SSIE: u64 = 1 << 1;
pub const IE_STIE: u64 = 1 << 5;
pub const IE_SEIE: u64 = 1 << 9;

pub const SATP_MODE: u64 = 0xf << 60;
pub const SATP_ASID: u64 = 0xffff << 44;
pub const SATP_PPN: u64 = 0xfff_ffffffff;

pub const SSTACK_BASE: u64 = 0xffffffffc0a00000 - 32*8;

pub const SCAUSE_INSN_MISALIGNED: u64 = 0;
pub const SCAUSE_INSN_ACCESS_FAULT: u64 = 1;
pub const SCAUSE_ILLEGAL_INSN: u64 = 2;
pub const SCAUSE_BREAKPOINT: u64 = 3;
pub const SCAUSE_LOAD_ACCESS_FAULT: u64 = 5;
pub const SCAUSE_ATOMIC_MISALIGNED: u64 = 6;
pub const SCAUSE_STORE_ACCESS_FAULT: u64 = 7;
pub const SCAUSE_ENV_CALL: u64 = 8;
pub const SCAUSE_INSN_PAGE_FAULT: u64 = 12;
pub const SCAUSE_LOAD_PAGE_FAULT: u64 = 13;
pub const SCAUSE_STORE_PAGE_FAULT: u64 = 15;
