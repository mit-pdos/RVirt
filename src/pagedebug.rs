
use rvirt::*;
use crate::machdebug::*;
use crate::pagedebug::PageWalkError::{ErrUnmapped};
use crate::trap::constants::STATUS_SUM;

pub const MODE_NONE: u8 = 0;
pub const MODE_SV39: u8 = 8;
pub const MODE_SV48: u8 = 9;
pub const MODE_SV57_RES: u8 = 10;
pub const MODE_SV64_RES: u8 = 11;

global_asm!(include_str!("loadaddress.S"));

#[link_section = ".text.init"]
fn mode_to_str(mode: u8) -> &'static str {
    match mode {
        MODE_NONE => "bare: no translation or protection",
        MODE_SV39 => "sv39: page-based 39-bit virtual addressing",
        MODE_SV48 => "sv48: page-based 48-bit virtual addressing",
        MODE_SV57_RES => "sv57: reserved for page-based 57-bit virtual addressing",
        MODE_SV64_RES => "sv64: reserved for page-based 64-bit virtual addressing",
        _ => "reserved"
    }
}

// returns (mode, asid, ppn)
#[link_section = ".text.init"]
fn parse_satp(satp: u64) -> (u8, u16, u64) {
    ((satp >> 60) as u8, (satp >> 44) as u16, satp & 0xfff_ffff_ffff)
}

const FLAG_VALID: u8 = 0x01;
const FLAG_READ: u8 = 0x02;
const FLAG_WRITE: u8 = 0x04;
const FLAG_EXEC: u8 = 0x08;
const FLAG_USER: u8 = 0x10;
const FLAG_GLOBAL: u8 = 0x20;
const FLAG_ACCESSED: u8 = 0x40;
const FLAG_DIRTY: u8 = 0x80;

#[derive(Clone)]
#[derive(Copy)]
#[derive(PartialEq)]
enum PageWalkError {
    ErrNone,
    ErrUnmapped,
    ErrReserved,
    ErrTooDeep,
    ErrMisalignedSuperpage,
}

#[link_section = ".text.init"]
fn pwe_to_str(err: PageWalkError) -> &'static str {
    match err {
        PageWalkError::ErrNone => "ok",
        PageWalkError::ErrUnmapped => "unmapped",
        PageWalkError::ErrReserved => "reserved bit pattern in use",
        PageWalkError::ErrTooDeep => "page table is too deep",
        PageWalkError::ErrMisalignedSuperpage => "superpage is misaligned",
    }
}

type PageWalkerCallback<Data> = fn(flags: u8, rsw: u8, va: u64, pa: u64, len: u64, err: PageWalkError, data: &mut Data);

const LEVELS_SV39: u8 = 3;
const PTESIZE_SV39: u64 = 8;
const SIGN_BITS_SV39: u8 = 64 - 39;
const VPN_BITS_EACH: u8 = 9;
const PPN_BITS_EACH: u8 = 9;
const PAGE_BITS: u8 = 12;
const PAGE_SIZE: u64 = 1u64 << PAGE_BITS;

#[link_section = ".text.init"]
fn sign_extend(v: u64, bits: u8) -> u64 {
    (((v << bits) as i64) >> bits) as u64
}

// TODO: handle getting blocked by PMP
#[link_section = ".text.init"]
unsafe fn walk_page_table_iter<Data>(a: u64, i: u8, vabase: u64, cb: PageWalkerCallback<Data>, data: &mut Data) {
    for entry in 0..512u64 {
        let pte = *((a + entry * PTESIZE_SV39) as *const u64);
        let ppn = (pte >> 10) & 0xfff_ffff_ffff; // mask because higher bits are reserved as of priv-v1.10
        let pabase = ppn << PAGE_BITS;
        let valocal = sign_extend(vabase + (entry << PAGE_BITS + VPN_BITS_EACH * i), SIGN_BITS_SV39);
        let flags = pte as u8;
        let rsw = ((pte >> 8) & 0x3) as u8;
        let pagelen = PAGE_SIZE << (PPN_BITS_EACH * i);

        let err;

        if (flags & (FLAG_VALID | FLAG_READ | FLAG_WRITE | FLAG_EXEC)) == FLAG_VALID {
            if i == 0 {
                err = PageWalkError::ErrTooDeep;
            } else {
                walk_page_table_iter(pabase, i - 1, valocal, cb, data);
                continue;
            }
        } else {
            if (flags & FLAG_VALID) == 0 {
                err = PageWalkError::ErrUnmapped;
            } else if (flags & (FLAG_VALID | FLAG_READ | FLAG_WRITE)) == (FLAG_VALID | FLAG_WRITE) {
                err = PageWalkError::ErrReserved;
            } else if (pabase & (pagelen - 1)) != 0 {
                err = PageWalkError::ErrMisalignedSuperpage;
            } else {
                err = PageWalkError::ErrNone;
            }
        }
        cb(flags, rsw, valocal, pabase, pagelen, err, data);
        if entry == 255 {
            cb(0, 0, 0x4000000000, 0, 0xffffff8000000000, ErrUnmapped, data);
        }
    }
}

#[link_section = ".text.init"]
unsafe fn walk_page_table<Data>(root: u64, cb: PageWalkerCallback<Data>, data: &mut Data) {
    walk_page_table_iter(root, LEVELS_SV39 - 1, 0, cb, data);
}

#[link_section = ".text.init"]
fn flag(flags: u8, f: &str, flag: u8) {
    let mut spaces = 1;
    if (flags & flag) == flag {
        machine_debug_puts(f);
    } else {
        spaces += f.len();
    }
    for _ in 0..spaces {
        machine_debug_puts(" ");
    }
}

struct CompressionWalker<'data, Data> {
    cb: PageWalkerCallback<Data>,
    data: &'data mut Data,
    haslast: bool,
    lastflags: u8,
    lastrsw: u8,
    totallen: u64,
    endva: u64,
    endpa: u64,
    lasterr: PageWalkError,
}

#[link_section = ".text.init"]
fn compression_walk<Data>(flags: u8, rsw: u8, va: u64, pa: u64, len: u64, err: PageWalkError, walker: &mut CompressionWalker<Data>) {
    if walker.haslast && (flags != walker.lastflags || rsw != walker.lastrsw || va != walker.endva || (pa != walker.endpa && err != ErrUnmapped) || err != walker.lasterr) {
        /*if flags != walker.lastflags {
            machine_debug_puts("FLAGS\n");
        }
        if rsw != walker.lastrsw {
            machine_debug_puts("RSW\n");
        }
        if va != walker.endva {
            machine_debug_puts("VA\n");
        }
        if pa != walker.endpa && err != ErrUnmapped {
            machine_debug_puts("PA\n");
        }
        if err != walker.lasterr {
            machine_debug_puts("ERR\n");
        }
        machine_debug_puts("RETIRED\n");*/
        // retire last entry
        (walker.cb)(walker.lastflags, walker.lastrsw, walker.endva - walker.totallen, walker.endpa - walker.totallen, walker.totallen, walker.lasterr, walker.data);
        walker.haslast = false;
    }
    if walker.haslast {
        // extend last entry
        walker.totallen += len;
        walker.endva += len;
        walker.endpa += len;
    } else {
        // create new entry
        walker.haslast = true;
        walker.lastflags = flags;
        walker.lastrsw = rsw;
        walker.totallen = len;
        walker.endva = va + len;
        walker.endpa = pa + len;
        walker.lasterr = err;
    }
    if va + len == 0 { // last entry; retire because we won't be coming back
        (walker.cb)(walker.lastflags, walker.lastrsw, walker.endva - walker.totallen, walker.endpa - walker.totallen, walker.totallen, walker.lasterr, walker.data);
        walker.haslast = false;
    }
}

#[link_section = ".text.init"]
#[inline(never)]
unsafe fn walk_page_table_compressed<Data>(root: u64, cb: PageWalkerCallback<Data>, data: &mut Data) {
    let mut ourdata = CompressionWalker{
        cb,
        data,
        haslast: false,
        lastflags: 0,
        lastrsw: 0,
        totallen: 0,
        endva: 0,
        endpa: 0,
        lasterr: PageWalkError::ErrNone
    };
    walk_page_table(root, compression_walk, &mut ourdata);
}

#[link_section = ".text.init"]
#[no_mangle]
fn debug_walk(flags: u8, rsw: u8, va: u64, pa: u64, len: u64, err: PageWalkError, _: &mut ()) {
    flag(flags, "VALID", FLAG_VALID);
    flag(flags, "R", FLAG_READ);
    flag(flags, "W", FLAG_WRITE);
    flag(flags, "X", FLAG_EXEC);
    flag(flags, "USER", FLAG_USER);
    flag(flags, "GLOBAL", FLAG_GLOBAL);
    flag(flags, "ACC", FLAG_ACCESSED);
    flag(flags, "DIRTY", FLAG_DIRTY);
    machine_debug_puts(" ");
    machine_debug_putint(rsw as u64);
    machine_debug_puts("  ");
    machine_debug_puthex64(va);
    machine_debug_puts("-");
    machine_debug_puthex64(va + len - 1);
    if err != ErrUnmapped {
        machine_debug_puts(" ");
        machine_debug_puthex64(pa);
        machine_debug_puts("-");
        machine_debug_puthex64(pa + len - 1);
        machine_debug_puts(" ");
    } else {
        machine_debug_puts("                                       ")
    }
    machine_debug_puts(pwe_to_str(err));
    machine_debug_newline();
}

#[link_section = ".text.init"]
#[inline(never)]
pub fn debug_paging() {
    machine_debug_mark_begin();
    let hart = csrr!(mhartid);
    machine_debug_puts("==================================================== PAGE TABLE STATE (hart ");
    machine_debug_putint(hart);
    machine_debug_puts(") ===================================================\n");
    let (mode, asid, ppn) = parse_satp(csrr!(satp));
    let root = ppn << PAGE_BITS;

    machine_debug_puts("Paging mode: ");
    machine_debug_puts(mode_to_str(mode));
    machine_debug_newline();

    machine_debug_puts("ASID: ");
    machine_debug_putint(asid as u64);
    machine_debug_newline();

    machine_debug_puts("Page table address: ");
    machine_debug_puthex64(root);
    machine_debug_newline();

    if (csrr!(sstatus) & STATUS_SUM) != 0 {
        machine_debug_puts("Supervisor: can access user memory\n");
    } else {
        machine_debug_puts("Supervisor: limited to supervisor memory\n");
    }

    if mode != MODE_SV39 {
        machine_debug_puts("debugging not implemented for this paging mode.\n")
    } else {
        machine_debug_puts("VALID R W X USER GLOBAL ACC DIRTY RSW   VIRTUAL (low)      VIRTUAL (high)     PHYSICAL (low)     PHYSICAL (high)  TRAVERSAL-ERROR\n");

        unsafe {
            let debug_walk_ptr: u64;
            asm!("LOAD_ADDRESS $0, debug_walk" : "=r"(debug_walk_ptr));
            walk_page_table_compressed(root, core::mem::transmute(debug_walk_ptr), &mut ());
        }
        machine_debug_puts("VALID R W X USER GLOBAL ACC DIRTY RSW   VIRTUAL (low)      VIRTUAL (high)     PHYSICAL (low)     PHYSICAL (high)  TRAVERSAL-ERROR\n");
    }
    machine_debug_puts("====================================================== END PAGE TABLE STATE ======================================================\n");
    machine_debug_mark_end();
}
