
use crate::trap::constants::STATUS_FS;

/** atomic read from CSR */
#[macro_export]
macro_rules! csrr {
    ( $r:ident ) => {
        {
            let value: u64;
            #[allow(unused_unsafe)]
            unsafe { asm!("csrr $0, $1" : "=r"(value) : "i"(crate::csr::$r)) };
            value
        }
    };
}

/** atomic write to CSR */
#[macro_export]
macro_rules! csrw {
    ( $r:ident, $x:expr ) => {
        {
            let x: u64 = $x;
            asm!("csrw $0, $1" :: "i"(crate::csr::$r), "r"(x));
        }
    };
}

/** atomic write to CSR from immediate */
#[macro_export]
macro_rules! csrwi {
    ( $r:ident, $x:expr ) => {
        {
            const X: u64 = $x;
            asm!("li t0, $1
                  csrw $0, t0"
                 :
                 : "i"(crate::csr::$r), "i"(X)
                 : "t0"
                 : "volatile");
        }
    };
}

/** atomic read and set bits in CSR */
#[macro_export]
macro_rules! csrs {
    ( $r:ident, $x:expr ) => {
        {
            let x: u64 = $x;
            asm!("csrs $0, $1" :: "i"(crate::csr::$r), "r"(x));
        }
    };
}

/** atomic read and set bits in CSR using immediate */
#[macro_export]
macro_rules! csrsi {
    ( $r:ident, $x:expr ) => {
        {
            const X: u64 = $x;
            asm!("li t0, $1
                  csrs $0, t0"
                 :
                 : "i"(crate::csr::$r), "i"(X)
                 : "t0"
                 : "volatile");
        }
    };
}

/** atomic read and clear bits in CSR */
#[macro_export]
macro_rules! csrc {
    ( $r:ident, $x:expr ) => {
        {
            let x: u64 = $x;
            asm!("csrc $0, $1" :: "i"(crate::csr::$r), "r"(x));
        }
    };
}

pub fn sfence_vma() {
    unsafe { asm!("sfence.vma" ::: "memory" : "volatile") }
}

pub fn barrier() {
    unsafe { asm!("" ::: "memory" : "volatile") }
}

pub fn fence_i() {
    unsafe { asm!("fence.i" :::: "volatile") }
}

/// Set the `sepc` CSR to the indicated value.
///
/// Since traps from S-mode always cause a hyperivsor panic, the value of `sstatus.spp` will always
/// be zero. Thus, mret will always cause a vmexit and so any value for sepc is safe.
pub fn set_sepc(value: u64) {
    unsafe { csrw!(sepc, value) }
}

/// Set the `sscratch` CSR. This is safe because `sscratch` does not impact processor execution.
pub fn set_sscratch(value: u64) {
    unsafe { csrw!(sscratch, value) }
}

/// Clear the indicated bits of `sip`. This is safe because interrupt state is not used to enforce
/// safety invariants.
pub fn clear_sip(mask: u64) {
    unsafe { csrc!(sip, mask) }
}

/// Set the FS bits of `sstatus`. This is safe because rvirt does not use hardware floating point
/// support.
pub fn set_sstatus_fs(new: u64) {
    unsafe { csrw!(sstatus, (new & STATUS_FS) | (csrr!(sstatus) & !STATUS_FS)) }
}
