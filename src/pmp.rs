
use rvirt::*;

pub unsafe fn write_pmp_config(entry: u8, config: u8) {
    assert!(entry <= 15, "entry out of range");

    let shift = (entry & 7) * 8;
    if entry < 8 {
        csrc!(pmpcfg0, (0xFF as u64) << shift);
        csrs!(pmpcfg0, (config as u64) << shift);
    } else {
        csrc!(pmpcfg2, (0xFF as u64) << shift);
        csrs!(pmpcfg2, (config as u64) << shift);
    }
}

pub fn read_pmp_config(entry: u8) -> u8 {
    assert!(entry <= 15, "entry out of range");

    let shift = (entry & 7) * 8;
    let reg = if entry < 8 {
        csrr!(pmpcfg0)
    } else {
        csrr!(pmpcfg2)
    };
    (reg >> shift) as u8
}

pub fn read_pmp_address(entry: u8) -> u64 {
    // come up with a better solution to this
    // (though apparently CSR instructions are hard-coded by CSR, so that might be hard?)
    match entry {
        0 => csrr!(pmpaddr0),
        1 => csrr!(pmpaddr1),
        2 => csrr!(pmpaddr2),
        3 => csrr!(pmpaddr3),
        4 => csrr!(pmpaddr4),
        5 => csrr!(pmpaddr5),
        6 => csrr!(pmpaddr6),
        7 => csrr!(pmpaddr7),
        8 => csrr!(pmpaddr8),
        9 => csrr!(pmpaddr9),
        10 => csrr!(pmpaddr10),
        11 => csrr!(pmpaddr11),
        12 => csrr!(pmpaddr12),
        13 => csrr!(pmpaddr13),
        14 => csrr!(pmpaddr14),
        15 => csrr!(pmpaddr15),
        _ => unreachable!("entry out of range"),
    }
}

pub unsafe fn write_pmp_address(entry: u8, address: u64) {
    // come up with a better solution to this
    // (though apparently CSR instructions are hard-coded by CSR, so that might be hard?)
    match entry {
        0 => csrw!(pmpaddr0, address),
        1 => csrw!(pmpaddr1, address),
        2 => csrw!(pmpaddr2, address),
        3 => csrw!(pmpaddr3, address),
        4 => csrw!(pmpaddr4, address),
        5 => csrw!(pmpaddr5, address),
        6 => csrw!(pmpaddr6, address),
        7 => csrw!(pmpaddr7, address),
        8 => csrw!(pmpaddr8, address),
        9 => csrw!(pmpaddr9, address),
        10 => csrw!(pmpaddr10, address),
        11 => csrw!(pmpaddr11, address),
        12 => csrw!(pmpaddr12, address),
        13 => csrw!(pmpaddr13, address),
        14 => csrw!(pmpaddr14, address),
        15 => csrw!(pmpaddr15, address),
        _ => unreachable!("entry out of range"),
    }
}

// note: these updates are not atomic. don't let interrupts happen during them!
pub unsafe fn install_pmp(entry: u8, config: u8, address: u64) {
    assert!((read_pmp_config(entry) & LOCK) == 0, "attempt to modify locked PMP entry");
    write_pmp_address(entry, address);
    write_pmp_config(entry, config);
}

pub unsafe fn install_pmp_napot(entry: u8, config: u8, address: u64, size: u64) {
    assert_eq!(address & 3, 0, "addresses must be 4-byte aligned");

    if size == 4 {
        install_pmp(entry, config | MODE_NA4, address >> 2);
    } else {
        assert!(size.is_power_of_two(), "attempt to install not-power-of-two napot value");
        assert_eq!(address & (size - 1), 0, "attempt to install unnaturally-aligned address");
        assert!(size >= 8, "attempt to install too-small napot value");

        install_pmp(entry, config | MODE_NAPOT, (address >> 2) + (size / 8 - 1));
    }
}

// cover everything in memory
pub unsafe fn install_pmp_allmem(entry: u8, config: u8) {
    // 0xFFFFFFFFFFFFFFFF is reserved as of priv-1.10, but fixed in an unreleased spec, and QEMU
    // interprets it correctly, so we're just going to go with it.
    install_pmp(entry, config | MODE_NAPOT, 0xFFFFFFFF_FFFFFFFF);
}

// returns (bits, remaining).
fn extract_napot_bits(address: u64) -> (u8, u64) {
    let mut bits = 0;
    let mut shifted = address;
    while (shifted & 1) == 1 {
        bits += 1;
        shifted >>= 1;
    }
    (bits, shifted << bits)
}

// if this is the first entry, set lastconfig = lastaddressreg = 0
// return value is [low, high) -- so low is inclusive and high is exclusive
pub fn decode_pmp_range(config: u8, address: u64, _lastconfig: u8, lastaddress: u64) -> (u64, u64) {
    match (config >> PMP_A_SHIFT) & 3 {
        PMP_A_OFF => (0, 0),
        PMP_A_TOR => (lastaddress << 2, address << 2),
        PMP_A_NA4 => (address << 2, (address << 2) + 4),
        PMP_A_NAPOT => {
            if address == 0xFFFFFFFF_FFFFFFFF {
                // covers everything, both per latest unreleased spec and QEMU interpretation
                (0, 0)
            } else {
                let (bits, address) = extract_napot_bits(address);
                (address << 2, (address << 2) + (8 << bits))
            }
        }
        _ => unreachable!()
    }
}

pub const READ: u8 = 0x1;
pub const WRITE: u8 = 0x2;
pub const EXEC: u8 = 0x4;
// for decoding
const PMP_A_SHIFT: u8 = 3;
const PMP_A_OFF: u8 = 0x0;
const PMP_A_TOR: u8 = 0x1;
const PMP_A_NA4: u8 = 0x2;
const PMP_A_NAPOT: u8 = 0x3;
// for encoding
#[allow(unused)]
pub const MODE_OFF: u8 = PMP_A_OFF << PMP_A_SHIFT;
#[allow(unused)]
pub const MODE_TOR: u8 = PMP_A_TOR << PMP_A_SHIFT;
pub const MODE_NA4: u8 = PMP_A_NA4 << PMP_A_SHIFT;
pub const MODE_NAPOT: u8 = PMP_A_NAPOT << PMP_A_SHIFT;
pub const RESERVED1: u8 = 0x20;
pub const RESERVED2: u8 = 0x40;
pub const LOCK: u8 = 0x80;

/** prints out as much information on the PMP state as possible in M-mode */
pub fn debug_pmp() {
    let hart = csrr!(mhartid);
    println!("============================== PMP CONFIGURATION STATE (hart {}) =============================", hart);
    println!("          R W X AMODE RES1 RES2 LOCK ADDRESS (raw)    ADDRESS (low)    ADDRESS (high)");
    let mut lastconfig= 0;
    let mut lastaddress = 0;
    for entry in 0..16 {
        let config = read_pmp_config(entry);
        let address = read_pmp_address(entry);
        print!("pmp{: <2}", entry);
        print!(" ==> ");
        if config & READ != 0 {
            print!("R ");
        } else {
            print!("- ");
        }
        if config & WRITE != 0 {
            print!("W ");
        } else {
            print!("- ");
        }
        if config & EXEC != 0 {
            print!("X ");
        } else {
            print!("- ");
        }
        let mode = (config >> PMP_A_SHIFT) & 3;
        match mode {
            PMP_A_OFF => print!(" OFF  "),
            PMP_A_TOR => print!(" TOR  "),
            PMP_A_NA4 => print!(" NA4  "),
            PMP_A_NAPOT => print!("NAPOT "),
            _ => unreachable!()
        };
        if config & RESERVED1 != 0 {
            print!("res1 ");
        } else {
            print!("     ");
        }
        if config & RESERVED2 != 0 {
            print!("res2 ");
        } else {
            print!("     ");
        }
        if config & LOCK != 0 {
            print!("lock ");
        } else {
            print!("     ");
        }
        print!("{:016x}", address);
        if mode != PMP_A_OFF {
            let (low, high) = decode_pmp_range(config, address, lastconfig, lastaddress);
            print!(" {:016x} {:016x}", low, high.wrapping_sub(1));
        }
        println!("");
        lastconfig = config;
        lastaddress = address;
    }
    println!("================================== END CONFIGURATION STATE ==================================");
}
