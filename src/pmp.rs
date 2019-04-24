
use crate::machdebug::*;

#[link_section = ".text.init"]
pub unsafe fn write_pmp_config(entry: u8, config: u8) {
    machine_debug_assert(0 <= entry && entry <= 15, "entry out of range");
    let shift = (entry & 7) * 8;
    if entry < 8 {
        csrc!(pmpcfg0, (0xFF as u64) << shift);
        csrs!(pmpcfg0, (config as u64) << shift);
    } else {
        csrc!(pmpcfg2, (0xFF as u64) << shift);
        csrs!(pmpcfg2, (config as u64) << shift);
    }
}

#[link_section = ".text.init"]
pub fn read_pmp_config(entry: u8) -> u8 {
    machine_debug_assert(0 <= entry && entry <= 15, "entry out of range");
    let shift = (entry & 7) * 8;
    let reg = if entry < 8 {
        csrr!(pmpcfg0)
    } else {
        csrr!(pmpcfg2)
    };
    (reg >> shift) as u8
}

#[link_section = ".text.init"]
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
        _ => { machine_debug_abort("entry out of range"); 0 }
    }
}

#[link_section = ".text.init"]
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
        _ => { machine_debug_abort("entry out of range"); },
    }
}

// note: these updates are not atomic. don't let interrupts happen during them!
#[link_section = ".text.init"]
pub unsafe fn install_pmp(entry: u8, config: u8, address: u64) {
    write_pmp_address(entry, address);
    machine_debug_assert(read_pmp_address(entry) == address, "could not change PMP address entry");
    write_pmp_config(entry, config);
    machine_debug_assert(read_pmp_config(entry) == config, "could not change PMP config entry");
}

#[link_section = ".text.init"]
pub unsafe fn install_pmp_napot(entry: u8, config: u8, address: u64, size: u64) {
    if (address & 3) != 0 {
        machine_debug_abort("addresses must be 4-byte aligned");
    }
    if size == 4 {
        install_pmp(entry, config | MODE_NA4, address >> 2);
    } else {
        if !size.is_power_of_two() {
            machine_debug_abort("attempt to install not-power-of-two napot value");
        }
        if (address & (size - 1)) != 0 {
            machine_debug_abort("attempt to install unnaturally-aligned address");
        }
        if size < 8 {
            machine_debug_abort("attempt to install too-small napot value");
        }
        install_pmp(entry, config | MODE_NAPOT, (address >> 2) + (size / 8 - 1));
    }
}

// returns (bits, remaining).
#[link_section = ".text.init"]
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
#[link_section = ".text.init"]
pub fn decode_pmp_range(config: u8, address: u64, lastconfig: u8, lastaddress: u64) -> (u64, u64) {
    match (config >> PMP_A_SHIFT) & 3 {
        PMP_A_OFF => (0, 0),
        PMP_A_TOR => (lastaddress << 2, address << 2),
        PMP_A_NA4 => (address << 2, (address << 2) + 4),
        PMP_A_NAPOT => {
            let (bits, address) = extract_napot_bits(address);
            (address << 2, (address << 2) + (8 << bits))
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
pub const MODE_OFF: u8 = PMP_A_OFF << PMP_A_SHIFT;
pub const MODE_TOR: u8 = PMP_A_TOR << PMP_A_SHIFT;
pub const MODE_NA4: u8 = PMP_A_NA4 << PMP_A_SHIFT;
pub const MODE_NAPOT: u8 = PMP_A_NAPOT << PMP_A_SHIFT;
pub const RESERVED1: u8 = 0x20;
pub const RESERVED2: u8 = 0x40;
pub const LOCK: u8 = 0x80;

/** prints out as much information on the PMP state as possible in M-mode */
#[link_section = ".text.init"]
pub fn debug_pmp() {
    machine_debug_mark_begin();
    let hart = csrr!(mhartid);
    machine_debug_puts("=========== PMP CONFIGURATION STATE (hart ");
    machine_debug_putint(hart);
    machine_debug_puts(") ==========\n");
    machine_debug_puts("          R W X AMODE RES1 RES2 LOCK   ADDRESS (raw)      ADDRESS (low)      ADDRESS (high)\n");
    let mut lastconfig= 0;
    let mut lastaddress = 0;
    for entry in 0..16 {
        let config = read_pmp_config(entry);
        let address = read_pmp_address(entry);
        machine_debug_puts("pmp");
        machine_debug_putint(entry as u64);
        if entry < 10 { machine_debug_puts(" "); }
        machine_debug_puts(" ==> ");
        if config & READ != 0 {
            machine_debug_puts("R ");
        } else {
            machine_debug_puts("- ");
        }
        if config & WRITE != 0 {
            machine_debug_puts("W ");
        } else {
            machine_debug_puts("- ");
        }
        if config & EXEC != 0 {
            machine_debug_puts("X ");
        } else {
            machine_debug_puts("- ");
        }
        let mode = (config >> PMP_A_SHIFT) & 3;
        match mode {
            PMP_A_OFF => machine_debug_puts(" OFF  "),
            PMP_A_TOR => machine_debug_puts(" TOR  "),
            PMP_A_NA4 => machine_debug_puts(" NA4  "),
            PMP_A_NAPOT => machine_debug_puts("NAPOT "),
            _ => unreachable!()
        };
        if config & RESERVED1 != 0 {
            machine_debug_puts("res1 ");
        } else {
            machine_debug_puts("     ");
        }
        if config & RESERVED2 != 0 {
            machine_debug_puts("res2 ");
        } else {
            machine_debug_puts("     ");
        }
        if config & LOCK != 0 {
            machine_debug_puts("lock ");
        } else {
            machine_debug_puts("     ");
        }
        machine_debug_puthex64(address);
        if mode != PMP_A_OFF {
            let (low, high) = decode_pmp_range(config, address, lastconfig, lastaddress);
            machine_debug_puts(" ");
            machine_debug_puthex64(low);
            machine_debug_puts(" ");
            machine_debug_puthex64(high - 1);
        }
        machine_debug_puts("\n");
        lastconfig = config;
        lastaddress = address;
    }
    machine_debug_puts("=============== END CONFIGURATION STATE ===============\n");
    machine_debug_mark_end();
}