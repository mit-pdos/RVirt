use crate::fdt::MachineMeta;
use crate::context::Context;
use crate::constants::SYMBOL_PA2VA_OFFSET;
use crate::memory_region::{MemoryRegion, PageTableRegion};
use crate::riscv;
use arr_macro::arr;
use arrayvec::ArrayVec;
use core::ptr;
use riscv_decode::types::RType;

const PAGE_SIZE: u64 = 4096;
const HPAGE_SIZE: u64 = 2 * 1024 * 1024;

#[allow(unused)]
mod segment_layout {
    pub const HART_SEGMENT_SIZE: u64 = 1 << 30; // 1 GB
    pub const DATA_OFFSET: u64 = 0;
    pub const DATA_SIZE: u64 = 2 << 20;
    pub const STACK_OFFSET: u64 = DATA_OFFSET + DATA_SIZE;
    pub const STACK_SIZE: u64 = 2 << 20;
    pub const HEAP_OFFSET: u64 = STACK_OFFSET + STACK_SIZE;
    pub const HEAP_SIZE: u64 = 28 << 20;
    pub const PT_REGION_OFFSET: u64 = HEAP_OFFSET + HEAP_SIZE;
    pub const PT_REGION_SIZE: u64 = 32 << 20;
    pub const VM_RESERVATION_SIZE: u64 = PT_REGION_OFFSET + PT_REGION_SIZE; // 64MB
}
pub use segment_layout::*;

#[allow(unused)]
pub mod pte_flags {
    pub const PTE_VALID: u64 = 0x1;
    pub const PTE_READ: u64 = 0x2;
    pub const PTE_WRITE: u64 = 0x4;
    pub const PTE_EXECUTE: u64 = 0x8;
    pub const PTE_USER: u64 = 0x10;
    pub const PTE_GLOBAL: u64 = 0x20;
    pub const PTE_ACCESSED: u64 = 0x40;
    pub const PTE_DIRTY: u64 = 0x80;
    pub const PTE_RSV_MASK: u64 = 0x300;

    pub const PTE_AD: u64 = PTE_ACCESSED | PTE_DIRTY;
    pub const PTE_RWV: u64 = PTE_READ | PTE_WRITE | PTE_VALID;
    pub const PTE_RXV: u64 = PTE_READ | PTE_EXECUTE | PTE_VALID;
    pub const PTE_RWXV: u64 = PTE_READ | PTE_WRITE | PTE_EXECUTE | PTE_VALID;
}
pub use pte_flags::*;

mod page_table_constants {
    pub const DIRECT_MAP_PT_INDEX: u64 = 0xf80;
    pub const DIRECT_MAP_OFFSET: u64 = DIRECT_MAP_PT_INDEX << 27 | ((!0) << 39);
    pub const DIRECT_MAP_PAGES: u64 = 8; // Uses 1 GB pages
}
pub use page_table_constants::*;

/// Make a minimal page table to boot into S mode. See [1] for FU540 errata related to mixing huge
/// pages and PMP.
///
/// [1] https://github.com/riscv/riscv-isa-manual/issues/347
pub const fn make_boot_page_table(base_pa: u64) -> [u64; 1024] {
    const fn pte(base_pa: u64, i: u64) -> u64 {
        // Horrible hack to get around limitations of const functions (see issue #57563). For this
        // to work, at most one of the conditions in `index` can be true.
        let possible_values = [
            0,
            ((base_pa + 4096) >> 2) | 0x01,
            0x20000000 | 0xcb,
            (0x20000000 + (i.wrapping_sub(512) << 19)) | 0xc7,
            ((i - DIRECT_MAP_PT_INDEX/8) << 28) | PTE_AD | PTE_RWXV,
        ];

        let index =
            1 * (i == 511) as usize +
            2 * (i == 512) as usize +
            3 * (i >= 513) as usize +
            4 * (i >= DIRECT_MAP_PT_INDEX/8) as usize * (i < DIRECT_MAP_PT_INDEX/8 + DIRECT_MAP_PAGES) as usize;

        possible_values[index]
    }

    let mut i = 0;
    arr![pte(base_pa, {i += 1; i - 1}); 1024]
}

// conversions between machine-physical addresses and supervisor-virtual address
#[allow(unused)]
pub fn pa2sa(pa: u64) -> u64 {
    if pa < 0x80000000 && pa >= 0xc0000000 {
        panic!("pa2sa given invalid address");
    }
    pa + SYMBOL_PA2VA_OFFSET
}
pub fn sa2pa(sa: u64) -> u64 {
    if sa < 0xffffffffc0000000 {
        panic!("pa2sa given invalid address");
    }
    sa - SYMBOL_PA2VA_OFFSET
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PageTableRoot {
    UVA,
    KVA,
    MVA,
    MPA,
}
use PageTableRoot::*;

const NULL_PAGE_PTR: u64 = 2;

pub struct PageTables {
    region: PageTableRegion,
    root_page_tables: [u64; 4],
    free_list_head: u64,
}
impl PageTables {
    /// Create a set of page tables from a memory region.
    ///
    /// The `initrd_start` and `initrd_end` parameters are an unfortunate implementation detail: the
    /// bootloader might have placed the init RAM disk inside our page table region. If this
    /// happened, we must make sure not to mark those pages as free until we're done using it.
    pub fn new(region: MemoryRegion, initrd_start: u64, initrd_end: u64) -> Self {
        let start = region.base();
        let end = start + region.len();
        let region = PageTableRegion::new(region);

        let mut ret = Self {
            region,
            root_page_tables: [0, 0, 0, 0],
            free_list_head: NULL_PAGE_PTR,
        };

        // initialize free list
        assert_eq!(start % PAGE_SIZE, 0);
        let mut addr = start;
        while addr < end {
            if addr + PAGE_SIZE <= initrd_start || addr >= initrd_end {
                ret.free_page(addr);
            }

            addr += PAGE_SIZE;
        }

        // initialize root page tables
        for i in 0..4 {
            ret.root_page_tables[i] = ret.alloc_page();
        }

        ret
    }

    pub fn root_pa(&self, root: PageTableRoot) -> u64 {
        let i = match root {
            MPA => 0,
            UVA => 1,
            KVA => 2,
            MVA => 3,
        };
        self.root_page_tables[i]
    }

    pub fn install_root(&self, root: PageTableRoot) {
        let new_satp = (8 << 60) | (self.root_pa(root) >> 12);
        if csrr!(satp) != new_satp {
            unsafe { csrw!(satp, new_satp) }
            riscv::sfence_vma();
        }
    }

    pub fn rmw_mapping(&mut self, root: PageTableRoot, va: u64, pte: u64) -> u64 {
        if va >= DIRECT_MAP_OFFSET {
            panic!("Guest attempted to access reserved virtual address: {:x}", va);
        }

        let pte_addr = self.pte_for_addr(root, va);
        let old = self.region[pte_addr];
        self.region.set_leaf_pte(pte_addr, pte);
        old
    }

    // Returns the physical address of the pte for a given virtual address.
    fn pte_for_addr(&mut self, root: PageTableRoot, va: u64) -> u64 {
        // These ranges use huge pages...
        assert!(va < DIRECT_MAP_OFFSET);
        assert!(is_sv39(va));
        assert!(root != PageTableRoot::MPA);

        let mut page_table = self.root_pa(root);
        for level in 0..2 {
            let pte_index = (va >> (30 - 9 * level)) & 0x1ff;
            let pte_addr = page_table + pte_index * 8;
            let pte = self.region[pte_addr];

            if pte & PTE_VALID != 0 {
                assert_eq!(pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE), 0);
                page_table = (pte >> 10) << 12;
            } else {
                let page = self.alloc_page();
                self.region.set_nonleaf_pte(pte_addr, (page >> 2) | PTE_VALID);
                page_table = page;
            }
        }
        page_table + ((va >> 12) & 0x1ff) * 8
    }

    pub fn clear_page_table(&mut self, pa: u64) {
        self.clear_page_table_range(pa, 0, 512);
    }
    pub fn clear_page_table_range(&mut self, pa: u64, start_index: u64, end_index: u64) {
        assert!(start_index <= end_index);
        assert!(end_index <= 512);

        for i in start_index..end_index {
            let pte = self.region[pa + i * 8];
            if pte & PTE_RWXV == PTE_VALID {
                let page = (pte >> 10) << 12;
                self.clear_page_table(page);
                self.free_page(page);
            }
            self.region.set_invalid_pte(pa + i * 8, 0);
        }
    }

    fn alloc_page(&mut self) -> u64 {
        if self.free_list_head == NULL_PAGE_PTR {
            panic!("Out of hypervisor memory for page tables");
        }

        let free = self.free_list_head;
        self.free_list_head = self.region[free];

        let mut addr = free;
        while addr < free + PAGE_SIZE {
            self.region.set_invalid_pte(addr, 0);
            addr += 8;
        }

        free
    }

    fn free_page(&mut self, page: u64) {
        self.region.set_invalid_pte(page, self.free_list_head);
        self.free_list_head = page;
    }
}

pub fn pa2va(pa: u64) -> u64 { pa + DIRECT_MAP_OFFSET }
pub fn va2pa(va: u64) -> u64 {
     // Must be in HPA region.
    assert!(va >= DIRECT_MAP_OFFSET);
    assert!(va < DIRECT_MAP_OFFSET + (DIRECT_MAP_PAGES<<30));
    va - DIRECT_MAP_OFFSET
}

pub struct Pte {
    pub addr: u64,
    pub value: u64,
    pub level: PageTableLevel,
}
pub struct PageTableWalk {
    pub path: ArrayVec<[Pte; 3]>,
    pub pa: u64,
}
pub fn walk_page_table<R: Fn(u64) -> Option<u64>>(root: u64, va: u64, read_pte: R) -> Option<PageTableWalk> {
    if !is_sv39(va) || root % PAGE_SIZE != 0 {
        return None;
    }

    let mut path = ArrayVec::new();
    let mut page_table = root;
    for level in 0..3 {
        let pte_index = (va >> (30 - 9 * level)) & 0x1ff;
        let pte_addr = page_table + pte_index * 8;
        let pte = read_pte(pte_addr)?;
        let level = match level {
            0 => PageTableLevel::Level1GB,
            1 => PageTableLevel::Level2MB,
            2 => PageTableLevel::Level4KB,
            _ => unreachable!(),
        };

        path.push(Pte {addr: pte_addr, value: pte, level});

        if pte & PTE_VALID == 0 || ((pte & PTE_WRITE) != 0 && (pte & PTE_READ) == 0) {
            return None;
        } else if pte & (PTE_READ | PTE_EXECUTE) != 0 {
            let pa = match level {
                PageTableLevel::Level4KB => ((pte >> 10) << 12) | (va & 0xfff),
                PageTableLevel::Level2MB => ((pte >> 19) << 21) | (va & 0x1fffff),
                PageTableLevel::Level1GB => ((pte >> 28) << 30) | (va & 0x3fffffff),
            };
            return Some(PageTableWalk{path, pa});
        } else {
            page_table = (pte >> 10) << 12;
        }
    }
    return None;
}

/// Returns whether va is a sign extended 39 bit address
pub fn is_sv39(va: u64) -> bool {
    let shifted = va >> 38;
    shifted == 0 || shifted == 0x3ffffff
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PageTableLevel {
    Level4KB,
    Level2MB,
    Level1GB,
}

pub struct AddressTranslation {
    pub pte_value: u64,
    pub pte_addr: u64,
    pub guest_pa: u64,
    pub level: PageTableLevel,
}

pub fn translate_guest_address(guest_memory: &MemoryRegion, root_page_table: u64, addr: u64)
                               -> Option<AddressTranslation> {
    walk_page_table(root_page_table, addr, |pa| guest_memory.get(pa)).map(|t| {
        AddressTranslation {
            pte_value: t.path[t.path.len() - 1].value,
            pte_addr: t.path[t.path.len() - 1].addr,
            level: t.path[t.path.len() - 1].level,
            guest_pa: t.pa,
        }
    })
}
pub fn translate_host_address(addr: u64) -> Option<PageTableWalk> {
    // The currently installed page table should always have all of its pages mapped in the direct
    // map region, thus deferencing pointers during a page table walk should always be safe.
    let root_page_table = (csrr!(satp) & riscv::bits::SATP_PPN) << 12;
    walk_page_table(root_page_table, addr, |pa| Some(unsafe { *(pa2va(pa) as *const u64) }))
}

pub unsafe fn init(hart_base_pa: u64, shared_segments_shift: u64, machine: &MachineMeta) -> (PageTables, MemoryRegion, u64) {
    assert_eq!(hart_base_pa % HART_SEGMENT_SIZE, 0);

    let gpm_offset = machine.physical_memory_offset;
    let gpm_size = HART_SEGMENT_SIZE.checked_sub(VM_RESERVATION_SIZE).unwrap();
    let guest_shift = VM_RESERVATION_SIZE + hart_base_pa.checked_sub(machine.physical_memory_offset).unwrap();
    assert_eq!(gpm_offset, 0x80000000);
    assert!(gpm_size > 64 * 1024 * 1024);

    // Create guest memory region
    let guest_memory = MemoryRegion::with_base_address(pa2va(gpm_offset + guest_shift), machine.physical_memory_offset, gpm_size);

    // Create shadow page tables
    let memory_region = MemoryRegion::new(pa2va(hart_base_pa + PT_REGION_OFFSET), PT_REGION_SIZE);
    let mut shadow_page_tables = PageTables::new(memory_region, machine.initrd_start, machine.initrd_end);

    let sshift = shared_segments_shift >> 2;

    // Initialize shadow page tables
    for &root in &[MPA, UVA, KVA, MVA] {
        let va = pa2va(shadow_page_tables.root_pa(root));
        ptr::write_bytes(va as *mut u8, 0, PAGE_SIZE as usize);

        *((va + DIRECT_MAP_PT_INDEX + 0 * 8) as *mut u64) = (0 << 28) | PTE_AD | PTE_RWV;
        *((va + DIRECT_MAP_PT_INDEX + 1 * 8) as *mut u64) = (1 << 28) | PTE_AD | PTE_RWV;
        *((va + DIRECT_MAP_PT_INDEX + (hart_base_pa >> 30) * 8) as *mut u64) = (hart_base_pa >> 2) | PTE_AD | PTE_RWV;

        // Hypervisor code + data
        let hp = 2 << 18;
        let page = shadow_page_tables.alloc_page();
        *((va + 0xff8) as *mut u64) = (page >> 2) | PTE_VALID;
        shadow_page_tables.region.set_pte_unchecked(
            page, (0x20000000+sshift) | PTE_AD | PTE_RXV);       // Code + read only data
        shadow_page_tables.region.set_pte_unchecked(
            page+8, (0x20000000+sshift+hp) | PTE_AD | PTE_RWV);  // Shared data
        shadow_page_tables.region.set_pte_unchecked(
            page+16, ((hart_base_pa>>2)) | PTE_AD | PTE_RWV);    // Data
        shadow_page_tables.region.set_pte_unchecked(
            page+32, ((hart_base_pa>>2)+hp) | PTE_AD | PTE_RWV); // Stack
    }
    shadow_page_tables.install_root(MPA);

    // Map guest physical memory
    assert_eq!(gpm_size % HPAGE_SIZE, 0);
    let root_pa = shadow_page_tables.root_pa(MPA);
    let npages = gpm_size / HPAGE_SIZE;
    for p in 0..npages  {
        let va = gpm_offset + p * HPAGE_SIZE;
        let pa = va + guest_shift;

        let pte_index = va >> 30;
        let pte_addr = root_pa + pte_index * 8;
        let pte = shadow_page_tables.region[pte_addr];
        let page_table = if pte & PTE_VALID != 0 {
            assert_eq!(pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE), 0);
            (pte >> 10) << 12
        } else {
            let page = shadow_page_tables.alloc_page();
            shadow_page_tables.region.set_nonleaf_pte(pte_addr, (page >> 2) | PTE_VALID);
            page
        };
        shadow_page_tables.region.set_leaf_pte(page_table + ((va >> 21) & 0x1ff) * 8,
                                               (pa >> 2) | PTE_AD | PTE_USER | PTE_RWXV);
    }

    (shadow_page_tables, guest_memory, guest_shift)
}

#[allow(unused)]
pub fn print_page_table(page_table_region: &PageTableRegion, pt: u64, level: u8) {
    for i in 0..512 {
        let pte = page_table_region[pt + i*8];
        if pte & PTE_VALID != 0 {
            for _ in 0..(3 - level) {
                print!("  ");
            }
            println!("{:#x}: {:#x}", i * 8, pte);
        }
        if pte & PTE_RWXV == PTE_VALID {
            assert!(level != 0);
            print_page_table(page_table_region, (pte >> 10) << 12, level - 1);
        }
    }
}

#[allow(unused)]
pub fn print_guest_page_table(guest_memory: &MemoryRegion, pt: u64, level: u8, base: u64) {
    if !guest_memory.in_region(pt) {
        println!("[SATP Invalid]");
        return;
    }

    for i in 0..512 {
        let addr = base + (i << (12 + level * 9));
        let pte = guest_memory[pt + i*8];
        if pte == 0 {
            continue;
        }

        for _ in 0..(2 - level) {
            print!("__ ");
        }

        if pte & PTE_RWXV == PTE_VALID {
            assert!(level != 0);
            let child = (pte >> 10) << 12;
            if !guest_memory.in_region(child) {
                println!("{:#x}: {:#x} (bad ppn)", addr, pte);
            } else {
                println!("{:#x}: {:#x}", addr, pte);
                print_guest_page_table(guest_memory, child, level - 1, addr);
                //break;
            }
        } else if pte & PTE_VALID != 0 {
            println!("{:#x} -> {:#x}", addr, (pte >> 10) << 12);
        } else if pte != 0 {
            println!("{:#x}: {:#x} (not valid)", addr, pte);
        }
    }
}

pub fn flush_shadow_page_table(shadow_page_tables: &mut PageTables) {
    for &root in &[UVA, KVA, MVA] {
        shadow_page_tables.clear_page_table_range(shadow_page_tables.root_pa(root), 0, DIRECT_MAP_PT_INDEX/8);
    }

    riscv::sfence_vma();
}

#[inline]
pub fn handle_sfence_vma(state: &mut Context, instruction: RType) {
    if instruction.rs1() == 0 {
        flush_shadow_page_table(&mut state.shadow_page_tables);
    } else {
        let va = state.saved_registers.get(instruction.rs1());
        if va < DIRECT_MAP_OFFSET {
            for &root in &[UVA, KVA, MVA] {
                let pte_addr = state.shadow_page_tables.pte_for_addr(root, va);

                match (state.shadow_page_tables.region[pte_addr] >> 8) & 0x3 {
                    0 => state.shadow_page_tables.region.set_invalid_pte(pte_addr, 0),
                    1 => for i in 0..512 {
                        state.shadow_page_tables.region.set_invalid_pte(
                            (pte_addr & !(PAGE_SIZE - 1)) + i * 8, 0)
                    }
                    _ => state.shadow_page_tables.clear_page_table_range(
                        state.shadow_page_tables.root_pa(root), 0, DIRECT_MAP_PT_INDEX/8),
                }
            }
            riscv::sfence_vma_addr(va);
        }
    }
}

pub fn read64(guest_memory: &MemoryRegion, page_table_ppn: u64, guest_va: u64) -> Option<u64> {
    let guest_page = guest_va & !0xfff;
    if let Some(page_translation) = translate_guest_address(guest_memory, page_table_ppn << 12, guest_page) {
        // assert!(!virtio::is_queue_access(state, page_translation.guest_pa));
        let guest_pa = (page_translation.guest_pa & !0xfff) | (guest_va & 0xfff);
        return guest_memory.get(guest_pa);
    }

    None
}
