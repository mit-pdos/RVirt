use arrayvec::{ArrayString, ArrayVec};
use byteorder::{BigEndian, ByteOrder};
use core::slice;

const FDT_BEGIN_NODE: u32 = 0x01;
const FDT_END_NODE: u32 = 0x02;
const FDT_PROP: u32 = 0x03;
const FDT_NOP: u32 = 0x04;
const FDT_END: u32 = 0x09;

#[derive(Default)]
struct AddressMap(ArrayVec<[u64; Self::MAX_LEN]>);
impl AddressMap {
    const MAX_LEN: usize = 16;
    fn index_of(&mut self, value: u64) -> usize {
        for i in 0..self.0.len() {
            if value == self.0[i] {
                return i;
            }
        }

        self.0.push(value);
        self.0.len() - 1
    }
}


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UartType {
    Ns16550a,
    SiFive,
}

#[derive(Clone, Debug)]
pub struct Device {
    pub base_address: u64,
    pub size: u64,
    pub irq: u64,
}

#[derive(Clone, Debug)]
pub struct Hart {
    pub hartid: u64,
    pub plic_context: u64,
}

#[derive(Clone, Debug, Default)]
pub struct MachineMeta {
    pub physical_memory_offset: u64,
    pub physical_memory_size: u64,

    pub harts: ArrayVec<[Hart; 16]>,

    pub uart_type: Option<UartType>,
    pub uart_address: u64,

    pub plic_address: u64,
    pub clint_address: u64,

    pub virtio: ArrayVec<[Device; 16]>,

    pub bootargs: ArrayString<[u8; 256]>,

    pub initrd_start: u64,
    pub initrd_end: u64,
}

#[repr(C)]
struct FdtHeader {
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

pub struct Fdt<'a>{
    header: &'a mut FdtHeader,
    strings: &'a [u8],
    nodes: &'a mut [u8],
}

#[allow(unused)]
impl<'a> Fdt<'a> {
    pub unsafe fn new(addr: u64) -> Self {
        let header = &mut *(addr as *mut FdtHeader);
        let total_size = header.total_size.swap_bytes() as usize;

        let off_dt_strings = header.off_dt_strings.swap_bytes() as u64;
        let size_dt_strings = header.size_dt_strings.swap_bytes() as usize;
        assert!(off_dt_strings as usize + size_dt_strings <= total_size);

        let off_dt_struct = header.off_dt_struct.swap_bytes() as u64;
        let size_dt_struct = header.size_dt_struct.swap_bytes() as usize;
        assert!(off_dt_struct as usize + size_dt_struct <= total_size);

        let strings = slice::from_raw_parts_mut((addr + off_dt_strings) as *mut u8, size_dt_strings);
        let nodes = slice::from_raw_parts_mut((addr + off_dt_struct) as *mut u8, size_dt_struct);

        Self {
            header,
            strings,
            nodes,
        }
    }

    pub fn magic_valid(&self) -> bool {
        self.header.magic == 0xedfe0dd0
    }
    pub fn total_size(&self) -> u32 { self.header.total_size.swap_bytes() }
    pub fn off_dt_struct(&self) -> u32 { self.header.off_dt_struct.swap_bytes() }
    pub fn off_dt_strings(&self) -> u32 { self.header.off_dt_strings.swap_bytes() }
    pub fn off_mem_rsvmap(&self) -> u32 { self.header.off_mem_rsvmap.swap_bytes() }
    pub fn version(&self) -> u32 { self.header.version.swap_bytes() }
    pub fn last_comp_version(&self) -> u32 { self.header.last_comp_version.swap_bytes() }
    pub fn boot_cpuid_phys(&self) -> u32 { self.header.boot_cpuid_phys.swap_bytes() }
    pub fn size_dt_strings(&self) -> u32 { self.header.size_dt_strings.swap_bytes() }
    pub fn size_dt_struct(&self) -> u32 { self.header.size_dt_struct.swap_bytes() }

    pub fn get_string(strings: &[u8], offset: usize) -> &str {
        let mut end = offset;
        while end < strings.len() && strings[end] != 0 {
            end += 1;
        }

        core::str::from_utf8(&strings[offset..end]).expect("FDT contained invalid string")
    }

    pub fn print(&mut self) {
        self.walk(|path, unit_addresses, v| match v {
            FdtVisit::Property { name, prop } => {
                if path != "/" {
                    let mut depth = 0;
                    for ch in path.chars() {
                        if ch == '/' {
                            if let Some(a) = unit_addresses[depth] {
                                print!("@{:x}", unit_addresses[depth].unwrap());
                            }
                            depth += 1;
                        }
                        print!("{}", ch)
                    }
                    if let Some(unit_address) = unit_addresses[depth] {
                        print!("@{:x}", unit_address)
                    }
                    print!(":{}", name);
                } else {
                    print!("{}", name);
                }

                if prop.len() == 4 || prop.len() == 8 {
                    println!("={:#x}", prop.read_int());
                } else if prop.len() == 16 {
                    let range = prop.read_range();
                    println!("={:x}:{:x}", range.0, range.1);
                } else if prop.len() != 0 {
                    if let Some(value) = prop.value_str() {
                        println!("=\"{}\"", value);
                    } else {
                        println!(" (value_len={})", prop.len());
                    }
                } else {
                    println!("");
                }
            }
            FdtVisit::Node { .. } => {}
        });
    }

    pub fn parse(&mut self) -> MachineMeta {
        let mut initrd_start: Option<u64> = None;
        let mut initrd_end: Option<u64> = None;
        let mut plic: Option<u64> = None;
        let mut clint: Option<u64> = None;

        let mut meta = MachineMeta::default();

        let mut virtio_address_map = AddressMap::default();
        let mut virtio = [(None, None); AddressMap::MAX_LEN];

        // (hartid, phandle)
        let mut cpus = [(None, None); AddressMap::MAX_LEN];
        let mut cpu_address_map = AddressMap::default();

        // hart phandle for each plic S-mode context
        let mut plic_context_phandles = [None; 64];

        self.walk(|path, unit_addresses, v| {
            match v {
                FdtVisit::Property { name, prop } => match (path, name) {
                    ("/chosen", "linux,initrd-end") => initrd_end = Some(prop.read_int()),
                    ("/chosen", "linux,initrd-start") => initrd_start = Some(prop.read_int()),
                    ("/chosen", "bootargs") => meta.bootargs.push_str(prop.value_str().unwrap()),
                    ("/memory", "reg") => {
                        let region = prop.read_range();
                        meta.physical_memory_offset = region.0;
                        meta.physical_memory_size = region.1;
                    }
                    ("/uart", "reg") |
                    ("/soc/uart", "reg") |
                    ("/soc/serial", "reg") => if meta.uart_address == 0 {
                        meta.uart_address = prop.read_range().0
                    }
                    ("/uart", "compatible") |
                    ("/soc/uart", "compatible") |
                    ("/soc/serial", "compatible") => if meta.uart_type.is_none() {
                        match prop.value_str().map(|s| s.trim_end_matches('\0')) {
                            Some("ns16550a") => meta.uart_type = Some(UartType::Ns16550a),
                            Some("sifive,uart0") => meta.uart_type = Some(UartType::SiFive),
                            _ => {},
                        }
                    }
                    ("/soc/clint", "reg") => clint = Some(prop.read_range().0),
                    ("/soc/interrupt-controller", "reg") => plic = Some(prop.read_range().0),
                    ("/soc/interrupt-controller", "interrupts-extended") => {
                        let cells = prop.cells();
                        for i in (0..cells).step_by(2) {
                            let irq = prop.read_cell(i + 1);
                            if irq == 9 {
                                plic_context_phandles[i/2] = Some(prop.read_cell(i));
                            }
                        }
                    }
                    ("/virtio_mmio", "reg") => {
                        let index = virtio_address_map.index_of(unit_addresses[1].unwrap_or(0));
                        virtio[index].0 = Some(prop.read_range());
                    }
                    ("/virtio_mmio", "interrupts") => {
                        let index = virtio_address_map.index_of(unit_addresses[1].unwrap_or(0));
                        virtio[index].1 = Some(prop.read_int());
                    }
                    ("/cpus/cpu", "reg") => {
                        let index = virtio_address_map.index_of(unit_addresses[2].unwrap_or(0));
                        cpus[index].0 = Some(prop.read_int());
                    }
                    ("/cpus/cpu/interrupt-controller", "phandle") => {
                        let index = virtio_address_map.index_of(unit_addresses[2].unwrap_or(0));
                        cpus[index].1 = Some(prop.read_int());
                    }
                    _ => {},
                }
                FdtVisit::Node { .. } => {}
            }
        });

        if initrd_start.is_some() && initrd_end.is_some() {
            meta.initrd_start = initrd_start.unwrap();
            meta.initrd_end = initrd_end.unwrap();
        }

        meta.plic_address = plic.unwrap();
        meta.clint_address = clint.unwrap();

        for &c in cpus.iter() {
            if let (Some(hartid), Some(phandle)) = c {
                if let Some(plic_context) = plic_context_phandles.iter().position(|&p| p == Some(phandle as u32)) {
                    meta.harts.push(Hart {
                        hartid,
                        plic_context: plic_context as u64,
                    })
                }
            }
        }
        meta.harts.sort_unstable_by_key(|h|h.hartid);

        for &v in virtio.iter().rev() {
            if let (Some((base_address, size)), Some(irq)) = v {
                meta.virtio.push(Device {
                    base_address,
                    size,
                    irq
                })
            }
        }
        meta.virtio.sort_unstable_by_key(|v| v.base_address);

        meta
    }

    pub fn initialize_guest(&mut self, guest_memory_size: u64, bootargs: &str) {
        self.walk(|path, unit_addresses, v| match v {
            FdtVisit::Property { name, prop } => match (path, name) {
                ("/chosen", "bootargs") => {
                    let s = prop.value_slice();
                    assert!(s.len() >= bootargs.len());

                    for i in 0..bootargs.len() {
                        s[i] = bootargs.as_bytes()[i];
                    }
                }
                ("/memory", "reg") => {
                    let region = prop.read_range();
                    let mut new_region = [0; 16];
                    BigEndian::write_u64(&mut new_region, region.0);
                    BigEndian::write_u64(&mut new_region[8..], guest_memory_size);
                    prop.set(&new_region);
                }
                _ => {},
            }
            FdtVisit::Node { .. } => {}
        });
    }

    // Mask out entries from FDT and return some information about the machine.
    fn walk<F>(&mut self, mut visit: F) where
        F: FnMut(&str, &[Option<u64>], FdtVisit),
    {
        let mut mask_node = 0;

        let mut path = ArrayString::<[_; 1024]>::new();
        let mut unit_addresses = ArrayVec::<[Option<u64>; 32]>::new();

        let mut i = 0;
        while i < self.nodes.len() {
            let old_i = i;
            assert_eq!(i % 4, 0);
            match BigEndian::read_u32(&self.nodes[i..]) {
                FDT_END => {
                    break;
                }
                FDT_BEGIN_NODE => {
                    i += 4;

                    // Root node is weird: name will be empty so its children should not prepend
                    // another slash.
                    if path.len() != 1 {
                        path.push('/');
                    }

                    let mut full_name = ArrayString::<[_;48]>::new();
                    while self.nodes[i] != 0 {
                        full_name.push(self.nodes[i] as char);
                        i += 1;
                    }
                    i = round4(i);

                    let mut name_parts = full_name.split('@');
                    path.push_str(name_parts.next().unwrap_or(""));
                    unit_addresses.push(name_parts.next().and_then(|a| u64::from_str_radix(a, 16).ok()));

                    if mask_node > 0 {
                        mask_node += 1;
                    } else {
                        let mut mask = false;
                        visit(&path, &unit_addresses, FdtVisit::Node { mask: &mut mask });
                        if mask {
                            mask_node = 1;
                        }
                    }
                }
                FDT_END_NODE => {
                    if mask_node > 0 {
                        BigEndian::write_u32(&mut self.nodes[i..], FDT_NOP);
                        mask_node = mask_node - 1;
                    }

                    let mut index = path.rfind('/').unwrap();
                    if index == 0 && path.len() > 1 {
                        index = 1;
                    }
                    path.truncate(index);
                    unit_addresses.pop();
                    i += 4;
                }
                FDT_PROP => {
                    let mut prop = Property::from_slice(&mut self.nodes[i..]).0;
                    let prop_name = Self::get_string(self.strings, prop.name_offset());
                    i += 12 + round4(prop.len());
                    visit(&path, &unit_addresses, FdtVisit::Property{ name: prop_name, prop: &mut prop });
                }
                FDT_NOP | _ => {
                    i += 4;
                }
            }

            if mask_node > 0 {
                for j in (old_i..i).step_by(4) {
                    BigEndian::write_u32(&mut self.nodes[j..], FDT_NOP);
                }
            }
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct MemoryRegion([u8; 16]);
impl MemoryRegion {
    pub fn offset(&self) -> u64 {
        BigEndian::read_u64(&self.0)
    }
    pub fn size(&self) -> u64 {
        BigEndian::read_u64(&self.0[8..])
    }
    pub fn set_size(&mut self, size: u64) {
        BigEndian::write_u64(&mut self.0[8..], size)
    }
}

#[repr(C)]
pub struct Property<'a>(&'a mut [u8]);
impl<'a> Property<'a> {
    pub fn from_slice(s: &'a mut [u8]) -> (Self, &mut [u8]) {
        assert_eq!(BigEndian::read_u32(s), FDT_PROP);

        let len = 12 + round4(BigEndian::read_u32(&s[4..]) as usize);
        let split = s.split_at_mut(len as usize);

        (Self(split.0), split.1)
    }

    pub fn len(&self) -> usize {
        BigEndian::read_u32(&self.0[4..][..4]) as usize
    }
    pub fn name_offset(&self) -> usize {
        BigEndian::read_u32(&self.0[8..][..4]) as usize
    }

    pub fn read_int(&self) -> u64 {
        match self.len() {
            4 => BigEndian::read_u32(&self.0[12..][..4]) as u64,
            8 => BigEndian::read_u64(&self.0[12..][..8]),
            _ => unreachable!(),
        }
    }
    pub fn read_range(&self) -> (u64, u64) {
        assert_eq!(self.len(), 16);

        (BigEndian::read_u64(&self.0[12..20]), BigEndian::read_u64(&self.0[20..28]))
    }
    pub fn mask(&mut self) {
        for i in (0..self.0.len()).step_by(4) {
            BigEndian::write_u32(&mut self.0[i..], FDT_NOP);
        }
    }
    pub fn value_str(&mut self) -> Option<&str> {
        if self.len() == 0 { return Some(""); }

        for i in 0..(self.len() - 1) {
            let c = self.0[12 + i];
            if c < 32 || c > 126 {
                return None;
            }
        }
        Some(core::str::from_utf8(&self.0[12..][..(self.len() - 1)]).unwrap())
    }
    pub fn value_slice(&mut self) -> &mut [u8] {
        &mut self.0[12..]
    }

    pub fn cells(&self) -> usize {
        self.len() / 4
    }
    pub fn read_cell(&self, i: usize) -> u32 {
        BigEndian::read_u32(&self.0[(12 + 4*i)..])
    }

    pub fn set(&mut self, value: &[u8]) {
        assert_eq!(value.len(), self.len());
        self.0[12..].copy_from_slice(value);
    }
}

enum FdtVisit<'a> {
    Node { mask: &'a mut bool },
    Property {
        name: &'a str,
        prop: &'a mut Property<'a>,
    }
}

/// Round up to the next multiple of 4
const fn round4(i: usize) -> usize {
    4 * ((i + 3) / 4)
}
