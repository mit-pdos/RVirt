
// use crate::trap::constants::STATUS_SUM;

#[inline(always)]
pub fn access_user_memory<T, F: Fn() -> T>(f: F) -> T {
//    csrs!(sstatus, STATUS_SUM);
    let t = f();
//    csrc!(sstatus, STATUS_SUM);
    t
}
