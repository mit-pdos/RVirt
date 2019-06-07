#[naked]
#[inline(never)]
fn ecall(_a0: u64, _a1: u64, _a2: u64, _a3: u64, _a4: u64, _a5: u64, _a6: u64, _a7: u64) {
    unsafe { asm!("ecall" :: : "a0" : "volatile") }
}

pub fn set_timer(stime_value: u64) {
    ecall(stime_value, 0, 0, 0, 0, 0, 0, 0);
}

pub fn clear_ipi() {
    ecall(0, 0, 0, 0, 0, 0, 0, 3);
}

pub fn send_ipi(hart_mask_pointer: u64) {
    ecall(hart_mask_pointer, 0, 0, 0, 0, 0, 0, 4);
}

pub fn shutdown() {
    ecall(0, 0, 0, 0, 0, 0, 0, 8);
}

pub fn send_ipi_to_hart(hart: u64) {
    let mask: u64 = 1 << hart;
    send_ipi(&mask as *const u64 as u64);
}
