
use rvirt::*;
use crate::pagedebug::PageWalkError::{ErrUnmapped};
use crate::riscv::bits::STATUS_SUM;

pub const MODE_NONE: u8 = 0;
pub const MODE_SV39: u8 = 8;
pub const MODE_SV48: u8 = 9;
pub const MODE_SV57_RES: u8 = 10;
pub const MODE_SV64_RES: u8 = 11;

global_asm!(include_str!("loadaddress.S"));

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

fn sign_extend(v: u64, bits: u8) -> u64 {
    (((v << bits) as i64) >> bits) as u64
}

// TODO: handle getting blocked by PMP
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

unsafe fn walk_page_table<Data>(root: u64, cb: PageWalkerCallback<Data>, data: &mut Data) {
    walk_page_table_iter(root, LEVELS_SV39 - 1, 0, cb, data);
}

fn flag(flags: u8, f: &str, flag: u8) {
    let mut spaces = 1;
    if (flags & flag) == flag {
        print!("{}", f);
    } else {
        spaces += f.len();
    }
    for _ in 0..spaces {
        print!(" ");
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

fn compression_walk<Data>(flags: u8, rsw: u8, va: u64, pa: u64, len: u64, err: PageWalkError, walker: &mut CompressionWalker<Data>) {
    if walker.haslast && (flags != walker.lastflags || rsw != walker.lastrsw || va != walker.endva || (pa != walker.endpa && err != ErrUnmapped) || err != walker.lasterr) {
        /*if flags != walker.lastflags {
            print!("FLAGS\n");
        }
        if rsw != walker.lastrsw {
            print!("RSW\n");
        }
        if va != walker.endva {
            print!("VA\n");
        }
        if pa != walker.endpa && err != ErrUnmapped {
            print!("PA\n");
        }
        if err != walker.lasterr {
            print!("ERR\n");
        }
        print!("RETIRED\n");*/
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
    print!(" {} {:#018x}-{:#018x}", rsw, va, va + len - 1);
    if err != ErrUnmapped {
        print!(" {:#018x}-{:#018x} ", pa, pa + len - 1);
    } else {
        print!("                                       ")
    }
    println!("{}", pwe_to_str(err));
}

#[inline(never)]
pub fn debug_paging() {
    let hart = csrr!(mhartid);
    println!("==================================================== PAGE TABLE STATE (hart {}) ===================================================", hart);
    let (mode, asid, ppn) = parse_satp(csrr!(satp));
    let root = ppn << PAGE_BITS;

    println!("Paging mode: {}", mode_to_str(mode));
    println!("ASID: {}", asid);
    println!("Page table address: {:#x}", root);

    if (csrr!(sstatus) & STATUS_SUM) != 0 {
        println!("Supervisor: can access user memory");
    } else {
        println!("Supervisor: limited to supervisor memory");
    }

    if mode != MODE_SV39 {
        println!("debugging not implemented for this paging mode.")
    } else {
        println!("VALID R W X USER GLOBAL ACC DIRTY RSW   VIRTUAL (low)      VIRTUAL (high)     PHYSICAL (low)     PHYSICAL (high)  TRAVERSAL-ERROR");

        unsafe {
            let debug_walk_ptr: u64;
            asm!("lla $0, debug_walk" : "=r"(debug_walk_ptr));
            walk_page_table_compressed(root, core::mem::transmute(debug_walk_ptr), &mut ());
        }
        println!("VALID R W X USER GLOBAL ACC DIRTY RSW   VIRTUAL (low)      VIRTUAL (high)     PHYSICAL (low)     PHYSICAL (high)  TRAVERSAL-ERROR");
    }
    println!("====================================================== END PAGE TABLE STATE ======================================================");
}
