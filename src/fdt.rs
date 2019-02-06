
const FDT_BEGIN_NODE: u32 = 0x01000000;
const FDT_END_NODE: u32 = 0x02000000;
const FDT_PROP: u32 = 0x03000000;
const FDT_NOP: u32 = 0x04000000;
const FDT_END: u32 = 0x09000000;

pub const VM_RESERVATION_SIZE: usize = 0x4000000; // 64MB

#[derive(Default)]
pub struct MachineMeta {
    // Host physical memory
    pub hpm_offset: u64,
    pub hpm_size: u64,

    // Guest physical memory
    pub gpm_offset: u64,
    pub gpm_size: u64,

    pub guest_shift: u64,

    pub initrd_start: Option<u64>,
    pub initrd_end: Option<u64>,
}

#[repr(C)]
pub struct Fdt {
    magic: u32,
    total_size: u32,
    off_dt_struct: u32,
    off_dt_strings: u32,
    off_mem_rsvmap: u32,
    version: u32,
    last_comp_version: u32,
    boot_cpuid_phys: u32,
    size_dt_strings: u32,
    size_dt_struct: u32,
}
#[allow(unused)]
impl Fdt {
    pub unsafe fn new(addr: usize) -> &'static Self {
        &*(addr as *const Self)
    }

    pub fn magic_valid(&self) -> bool {
        self.magic == 0xedfe0dd0
    }

    pub fn total_size(&self) -> u32 { self.total_size.swap_bytes() }
    pub fn off_dt_struct(&self) -> u32 { self.off_dt_struct.swap_bytes() }
    pub fn off_dt_strings(&self) -> u32 { self.off_dt_strings.swap_bytes() }
    pub fn off_mem_rsvmap(&self) -> u32 { self.off_mem_rsvmap.swap_bytes() }
    pub fn version(&self) -> u32 { self.version.swap_bytes() }
    pub fn last_comp_version(&self) -> u32 { self.last_comp_version.swap_bytes() }
    pub fn boot_cpuid_phys(&self) -> u32 { self.boot_cpuid_phys.swap_bytes() }
    pub fn size_dt_strings(&self) -> u32 { self.size_dt_strings.swap_bytes() }
    pub fn size_dt_struct(&self) -> u32 { self.size_dt_struct.swap_bytes() }

    pub fn address(&self) -> *const u8 { self as *const _ as *const u8 }

    pub unsafe fn memory_reservations(&self) -> &'static [MemoryRegion] {
        let addr = self.address().offset(self.off_mem_rsvmap() as isize) as *const MemoryRegion;
        let mut entries = 0;
        loop {
            let entry = &*addr.offset(entries as isize);
            if entry.size() == 0 && entry.offset() == 0 {
                break;
            }
            entries += 1;
        }

        core::slice::from_raw_parts(addr, entries)
    }

    pub unsafe fn strings(&self) -> &'static [u8] {
        let addr = self.address().offset(self.off_dt_strings() as isize);
        core::slice::from_raw_parts(addr, self.size_dt_strings() as usize)
    }

    pub unsafe fn get_string(&self, offset: u32) -> &'static str {
        self.str_from_ptr(self.address().offset((self.off_dt_strings() + offset) as isize))
    }
    pub unsafe fn str_from_ptr(&self, start: *const u8) -> &'static str {
        let mut ptr = start;
        while *ptr != 0 {
            ptr = ptr.add(1);
        }

        core::str::from_utf8(core::slice::from_raw_parts(start, ptr.offset_from(start) as usize)).unwrap()
    }

    pub unsafe fn print(&self) {
        // println!("total_size = {}", self.total_size());
        // println!("version = {}", self.version());

        let reservations = self.memory_reservations();
        if reservations.len() > 0 {
            println!("Reservations");
            for r in reservations {
                println!("   addr = {}", r.offset());
                println!("   size = {}", r.size());
            }
        }

        // println!("Strings...");
        // let strings = self.strings();
        // let mut indent = false;
        // for s in strings {
        //     if *s == 0 {
        //         println!("");
        //         indent = false;
        //     } else {
        //         if !indent {
        //             print!("   ");
        //             indent = true;
        //         }
        //         print!("{}", *s as char);
        //     }
        // }

        let mut indent = 0;
        let mut ptr = self.address().offset(self.off_dt_struct() as isize) as *const u32;
        let end = ptr.offset((self.size_dt_struct() as isize + 3) / 4);
        while ptr < end && *ptr != FDT_END {
            match *ptr {
                FDT_BEGIN_NODE => {
                    ptr = ptr.add(1);
                    let string = self.str_from_ptr(ptr as *const u8);

                    ptr = ptr.add(1 + string.len() / 4);

                    for _ in 0..indent { print!(" "); }
                    println!("BeginNode {}", string);

                    indent += 1;
                }
                FDT_END_NODE => {
                    indent -= 1;
                    for _ in 0..indent { print!(" "); }
                    println!("EndNode");
                    ptr = ptr.offset(1);

                }
                FDT_PROP => {
                    for _ in 0..indent { print!(" "); }
                    let prop = &*(ptr.offset(1) as *const Property);
                    let name = self.get_string(prop.name_offset());
                    println!("Property: name={}, name_len={} len={} ", name, name.len(), prop.len());
                    ptr = ptr.offset(3 + (prop.len() as isize + 3) / 4);
                }
                FDT_NOP => {
                    ptr = ptr.offset(1);
                }
                p => {
                    for _ in 0..indent { print!(" "); }
                    println!("Unknown: {:#x}", p);
                    ptr = ptr.offset(1);
                }
            }
        }
    }

    // Mask out entries from FDT and return some information about the machine.
    pub unsafe fn process(&self) -> MachineMeta {
        let mut initrd_start: Option<usize> = None;
        let mut initrd_end: Option<usize> = None;

        let mut meta = MachineMeta {
            guest_shift: VM_RESERVATION_SIZE as u64,
            .. Default::default()
        };

        let mut indent = 0;
        let mut device_name = "";
        let mut ptr = self.address().offset(self.off_dt_struct() as isize) as *const u32;
        let end = ptr.offset((self.size_dt_struct() as isize + 3) / 4);
        while ptr < end && *ptr != FDT_END {
            match *ptr {
                FDT_BEGIN_NODE => {
                    indent += 1;
                    ptr = ptr.add(1);
                    let name = self.str_from_ptr(ptr as *const u8);
                    ptr = ptr.add(1 + name.len() / 4);

                    if indent == 2 {
                        device_name = name.split('@').next().unwrap_or("");
                    }
                }
                FDT_END_NODE => {
                    if indent == 2 {
                        device_name = "";
                    }
                    indent -= 1;
                    ptr = ptr.offset(1);
                }
                FDT_PROP => {
                    let prop = &*(ptr.offset(1) as *const Property);
                    let prop_name = self.get_string(prop.name_offset());
                    ptr = ptr.offset(3 + (prop.len() as isize + 3) / 4);

                    if indent == 2 {
                        match (device_name, prop_name) {
                            ("chosen", "linux,initrd-end") => {
                                meta.initrd_end = Some(prop.read_int());
                                prop.mask();
                            }
                            ("chosen", "linux,initrd-start") => {
                                meta.initrd_start = Some(prop.read_int());
                                prop.mask();
                            }
                            ("memory", "reg") => {
                                // TODO: Handle multiple memory regions
                                assert_eq!(prop.len(), 16);

                                let region = &mut *(prop.address().offset(8) as *const _ as *mut MemoryRegion);
                                meta.hpm_offset = region.offset();
                                meta.hpm_size = region.size();
                                region.size = region.size().checked_sub(VM_RESERVATION_SIZE as u64).unwrap().swap_bytes();
                                // region.offset = (region.offset() + VM_RESERVATION_SIZE as u64).swap_bytes();
                                meta.gpm_offset = region.offset();
                                meta.gpm_size = region.size();
                            }
                            _ => {}
                        }
                    }
                }
                FDT_NOP | _ => ptr = ptr.offset(1),
            }
        }

        meta
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct MemoryRegion {
    offset: u64,
    size: u64,
}
impl MemoryRegion {
    pub fn offset(&self) -> u64 { self.offset.swap_bytes() }
    pub fn size(&self) -> u64 { self.size.swap_bytes() }
}

#[repr(C)]
pub struct Property {
    len: u32,
    name_offset: u32,
}
impl Property {
    pub fn len(&self) -> u32 { self.len.swap_bytes() }
    pub fn name_offset(&self) -> u32 { self.name_offset.swap_bytes() }

    pub fn address(&self) -> *const u8 { self as *const _ as *const u8 }

    pub unsafe fn read_int(&self) -> u64 {
        match self.len() {
            4 => (*(self.address().add(8) as *const u32)).swap_bytes() as u64,
            8 => (*(self.address().add(8) as *const u64)).swap_bytes(),
            _ => unreachable!(),
        }
    }
    pub unsafe fn mask(&self) {
        let length = (self.len() as usize + 3) / 4 + 3;
        let start = self.address().offset(-4) as *const u32 as *mut u32;

        for i in 0..length {
            *(start.add(i)) = FDT_NOP;
        }
    }
}
