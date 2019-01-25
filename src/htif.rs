use core::ptr;

#[no_mangle]
#[link_section = ".htif"]
static mut tohost: u64 = 0;
#[no_mangle]
#[link_section = ".htif"]
static mut fromhost: u64 = 0;

static mut HTIF_CONSOLE_BUF: Option<u64> = None;

fn read_fromhost() -> u64 {
    unsafe { ptr::read_volatile(&mut fromhost as *mut u64) }
}
fn clear_fromhost() {
    unsafe { ptr::write_volatile(&mut fromhost as *mut u64, 0) }
}
fn write_tohost(v: u64) {
    unsafe { ptr::write_volatile(&mut tohost as *mut u64, v) }
}
fn tohost_set() -> bool {
    unsafe { ptr::read_volatile(&mut tohost as *mut u64) != 0 }
}

fn check_fromhost() {
    let fh = read_fromhost();
    if fh == 0 {
        return;
    }

    clear_fromhost();

    let dev = fh >> 56;
    let cmd = (fh << 8) >> 56;
    let data = (fh << 16) >> 16;

    //    assert_eq!(dev, 1);
    match cmd {
        0 => unsafe { HTIF_CONSOLE_BUF = Some(data) }
        1 => {}
        _ => loop {}
    }
}

pub fn putchar(ch: u8) {
    while tohost_set() {
        check_fromhost();
    }
    write_tohost((1 << 56) | (1 << 48) | ch as u64);
}
