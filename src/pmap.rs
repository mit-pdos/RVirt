use crate::fdt::{self, MachineMeta};
use crate::context::Context;
use crate::memory_region::{MemoryRegion, PageTableRegion};
use crate::trap::MAX_STACK_PADDR;
use crate::sum;
use core::ops::{Index, IndexMut};
use core::ptr;
use riscv_decode::Instruction;
use spin::Mutex;

const PAGE_SIZE: u64 = 4096;
const HPAGE_SIZE: u64 = 2 * 1024 * 1024;

const PAGE_TABLE_SHIFT: u32 = 9;

pub static mut MAX_GUEST_PHYSICAL_ADDRESS: u64 = 0;

pub const SV39_MASK: u64 = !((!0) << 39);

#[allow(unused)]
mod pte_flags {
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
    pub const PTE_RWXV: u64 = PTE_READ | PTE_WRITE | PTE_EXECUTE | PTE_VALID;
}
pub use pte_flags::*;

mod page_table_constants {
    pub const BOOT_PAGE_TABLE: u64 = 0x80017000;

    pub const DIRECT_MAP_PT_INDEX: u64 = 0xf80;
    pub const DIRECT_MAP_OFFSET: u64 = DIRECT_MAP_PT_INDEX << 27 | ((!0) << 39);
}
pub use page_table_constants::*;

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum PageTableRoot {
//    ROOT = 0,
//    HVA = 1,
    UVA = 2,
    KVA = 3,
    MVA = 4,
    MPA = 5,
//    HPA = 6,
}
pub use PageTableRoot::*;

impl PageTableRoot {
    const fn index(&self) -> u64 {
        const INDEXES: [u64; 7] = [0, 511, 2, 3, 4, 5, 1];
        INDEXES[*self as usize]
    }

    #[inline(always)]
    pub const fn satp(&self) -> u64 {
        const MODES: [u64; 7] = [9, 8, 8, 8, 8, 8, 8];
        let mode = MODES[*self as usize];
        let asid = *self as u64;
        let ppn = self.pa();

        mode << 60 | (asid << 44) | (ppn >> 12)
    }

    const fn pa(&self) -> u64 {
        const PHYSICAL_ADDRESSES: [u64; 7] = [
            0x80010000,
            0x80011000,
            0x80012000,
            0x80013000,
            0x80014000,
            0x80015000,
            0x80016000,
        ];
        PHYSICAL_ADDRESSES[*self as usize]
    }

    fn va(&self) -> u64 {
        pa2va(self.pa())
    }

    // pub fn address_to_pointer<T>(&self, addr: u64) -> *mut T {
    //     if *self == ROOT {
    //         assert!(is_sv48(addr));
    //         addr as *mut T
    //     } else {
    //         assert!(is_sv39(addr));
    //         ((addr & SV39_MASK) + self.offset()) as *mut T
    //     }
    // }

    pub fn get_pte(&self, addr: u64) -> *mut u64 {
        unsafe {
            pte_for_addr(self.va() as *mut u64, addr)
        }
    }
}

impl Index<u64> for PageTableRoot {
    type Output = u64;

    fn index(&self, i: u64) -> &Self::Output {
        assert!(i < 512);
        unsafe {
            &*((self.va() + i*8) as *const u64)
        }
    }
}
impl IndexMut<u64> for PageTableRoot {
    fn index_mut(&mut self, i: u64) -> &mut Self::Output {
        assert!(i < 512);
        unsafe {
            &mut *((self.va() + i*8) as *mut u64)
        }
    }
}

pub fn pa2va(pa: u64) -> u64 { pa + DIRECT_MAP_OFFSET }
pub fn va2pa(va: u64) -> u64 {
     // Must be in HPA region.
    assert!(va >= DIRECT_MAP_OFFSET);
//    assert!(va < DIRECT_MAP_OFFSET + (1u64<<39));
    va - DIRECT_MAP_OFFSET
}
pub fn mpa2pa(mpa: u64) -> Option<u64> {
    if mpa >= 0x80000000 && mpa < unsafe {MAX_GUEST_PHYSICAL_ADDRESS} {
        return Some(mpa + fdt::VM_RESERVATION_SIZE as u64);
    }

    // if mpa < 0x30000000 { // DEBUG, MROM, TEST, CLINT, PLIC, and UART0 inaccessible
    //     None
    // } else if mpa < 0x80000000 { // VIRTIO and PCIe accessible
    //     Some(mpa)
    // } else { // High memory inacessible
        None
    // }
}

/// Returns whether va is a sign extended 39 bit address
pub fn is_sv39(va: u64) -> bool {
    let shifted = va >> 38;
    shifted == 0 || shifted == 0x3ffffff
}
/// Returns whether va is a sign extended 48 bit address
pub fn is_sv48(va: u64) -> bool {
    let shifted = va >> 47;
    shifted == 0 || shifted == 0x1ffff
}

#[allow(unused)]
pub enum AccessType {
    Read,
    Write,
    Execute,
}

#[repr(transparent)]
struct Page([u8; PAGE_SIZE as usize]);

#[repr(align(4096))]
struct FreePage(Option<&'static FreePage>);
static FREE_LIST: Mutex<Option<&'static FreePage>> = Mutex::new(None);

fn alloc_page() -> *mut Page {
    let mut free_list = FREE_LIST.lock();
    let free = free_list.take();
    let next = free.as_ref().expect("Out of Hypervisor Memory").0;
    *free_list = next;
    let page = free.unwrap() as *const FreePage as *mut Page;

    let mut addr = page as u64;
    while addr < page as u64 + 4096 {
        unsafe { *(addr as *mut u64) = 0 };
        addr += 8;
    }

    page
}
fn free_page(page: *mut Page) {
    let mut free_list = FREE_LIST.lock();
    let mut free_page: &mut FreePage = unsafe { &mut *(page as *mut FreePage) };
    free_page.0 = free_list.take();
    *free_list = Some(free_page)
}

unsafe fn clear_page_table(pa: u64) {
    let va = pa2va(pa) as *mut u64;
    for i in 0..512 {
        let pte = va.add(i);
        if *pte & PTE_RWXV == PTE_VALID {
            let page = (*pte >> 10) << 12;
            clear_page_table(page);
            free_page(pa2va(page) as *mut Page);
        }
        *pte = 0;
    }
}

unsafe fn pte_for_addr(mut page_table: *mut u64, addr: u64) -> *mut u64 {
    // These ranges use huge pages...
    assert!(addr < DIRECT_MAP_OFFSET);
    assert!(is_sv39(addr));

    // let mut page_table = ROOT.va() as *mut u64;
    for level in 0..2 {
        let pte_index = ((addr >> (30 - PAGE_TABLE_SHIFT * level)) & 0x1ff) as usize;
        let pte = *page_table.add(pte_index);

        if pte & PTE_VALID != 0 {
            assert_eq!(pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE), 0);
            page_table = pa2va((pte >> 10) << 12) as *mut u64;
        } else {
            let page = alloc_page();
            *page_table.add(pte_index) = (va2pa(page as u64) >> 2) | PTE_VALID;
            page_table = page as *mut u64;
        }
    }
    page_table.add((addr as usize >> 12) & 0x1ff)
}

pub struct AddressTranslation {
    pub pte_value: u64,
    pub pte_addr: u64,
    pub guest_pa: u64,
}

// Returns the guest physical address associated with a given guest virtual address, by walking
// guest page tables.
pub fn translate_guest_address(guest_memory: &MemoryRegion, root_page_table: u64, addr: u64) -> Option<AddressTranslation> {
    if !is_sv39(addr) {
        return None;
    }
    if root_page_table > unsafe{MAX_GUEST_PHYSICAL_ADDRESS} || root_page_table % PAGE_SIZE != 0 {
        return None;
    }

    let mut page_table = root_page_table;
    for level in 0..3 {
        let pte_index = ((addr >> (30 - 9 * level)) & 0x1ff);
        let pte_addr = page_table + pte_index * 8;
        let pte = guest_memory[pte_addr];

        if pte & PTE_VALID == 0 || ((pte & PTE_WRITE) != 0 && (pte & PTE_READ) == 0) {
            return None;
        } else if pte & (PTE_READ | PTE_EXECUTE) != 0 {
            let guest_pa = match level {
                2 => ((pte >> 10) << 12) | (addr & 0xfff),
                1 => ((pte >> 19) << 21) | (addr & 0x1fffff),
                0 => ((pte >> 28) << 30) | (addr & 0x3fffffff),
                _ => unreachable!(),
            };
            return Some(AddressTranslation { guest_pa, pte_addr, pte_value: pte });
        } else {
            page_table = (pte >> 10) << 12;
            if page_table > unsafe { MAX_GUEST_PHYSICAL_ADDRESS } {
                return None;
            }
        }
    }

    None
}


pub fn init(machine: &MachineMeta) -> (PageTableRegion, MemoryRegion) {
    unsafe {
        // Zero out page tables
        ptr::write_bytes(UVA.pa() as *mut u8, 0, PAGE_SIZE as usize);
        ptr::write_bytes(KVA.pa() as *mut u8, 0, PAGE_SIZE as usize);
        ptr::write_bytes(MVA.pa() as *mut u8, 0, PAGE_SIZE as usize);
        ptr::write_bytes(MPA.pa() as *mut u8, 0, PAGE_SIZE as usize);

        for &pa in &[MPA.pa(), UVA.pa(), KVA.pa(), MVA.pa()] {
            // Direct map region
            for i in 0..4 {
                *((pa + DIRECT_MAP_PT_INDEX + i * 8) as *mut u64) = (i << 28) | PTE_AD | PTE_RWXV;
            }

            *((pa + 0xf80) as *mut u64) = 0x00000000 | PTE_AD | PTE_RWXV;
            *((pa + 0xf88) as *mut u64) = 0x10000000 | PTE_AD | PTE_RWXV;
            *((pa + 0xf90) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
            *((pa + 0xf98) as *mut u64) = 0x30000000 | PTE_AD | PTE_RWXV;

            // Hypervisor code + data
            *((pa + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        }

        *((MPA.pa() + 0xf80) as *mut u64) = 0x00000000 | PTE_AD | PTE_RWXV;
        *((MPA.pa() + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;

        csrw!(satp, MPA.satp());
        asm!("sfence.vma"
             ::: "memory" : "volatile");


        crate::print::uart::UART = pa2va(crate::print::uart::UART as u64) as *mut _;

        assert_eq!(machine.gpm_offset, 0x80000000);
        MAX_GUEST_PHYSICAL_ADDRESS = machine.gpm_offset + machine.gpm_size;

        let mut addr = MAX_STACK_PADDR;
        while addr < machine.hpm_offset + fdt::VM_RESERVATION_SIZE {
            if addr < machine.initrd_start || addr >= machine.initrd_end {
                 free_page(pa2va(addr) as *mut Page);
            }

            addr += PAGE_SIZE;
        }

        // Map guest physical memory
        assert_eq!(machine.gpm_size % HPAGE_SIZE, 0);
        let npages = machine.gpm_size / HPAGE_SIZE;
        for p in 0..npages  {
            let va = machine.gpm_offset + p * HPAGE_SIZE;
            let pa = va + machine.guest_shift;

            let pte_index = va >> 30;
            let pte_ptr = ((MPA.va() + pte_index * 8) as *mut u64);
            let pte = *pte_ptr;
            let page_table = if pte & PTE_VALID != 0 {
                assert_eq!(pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE), 0);
                pa2va((pte >> 10) << 12) as *mut u64
            } else {
                let page = alloc_page();
                *pte_ptr = (va2pa(page as u64) >> 2) | PTE_VALID;
                page as *mut u64
            };

            *page_table.add((va as usize >> 21) & 0x1ff) =
                (pa >> 2) | PTE_AD | PTE_USER | PTE_RWXV;
        }

        let length = machine.hpm_offset + fdt::VM_RESERVATION_SIZE - MAX_STACK_PADDR;
        let page_table_region = PageTableRegion::new(MemoryRegion::new(pa2va(MAX_STACK_PADDR), length));
        let guest_memory = MemoryRegion::with_base_address(pa2va(machine.gpm_offset + machine.guest_shift), machine.gpm_offset, machine.gpm_size);

        (page_table_region, guest_memory)
    }
}

#[allow(unused)]
pub fn print_page_table(pt: u64, level: u8) {
    unsafe {
        for i in 0..512 {
            let pte = *(pa2va(pt + i*8) as *const u64);
            if pte & PTE_VALID != 0 {
                for _ in 0..(4 - level) {
                    print!("  ");
                }
                println!("{:#x}: {:#x}", i *8, pte);
            }
            if pte & PTE_RWXV == PTE_VALID {
                assert!(level != 0);
                print_page_table((pte >> 10) << 12, level - 1);
            }
        }
    }
}

#[allow(unused)]
pub fn print_guest_page_table(pt: u64, level: u8, base: u64) {
    unsafe {
        if pt >= MAX_GUEST_PHYSICAL_ADDRESS {
            println!("[SATP Invalid]");
            return;
        }

        for i in 0..512 {
            let addr = base + (i << (12 + level * 9));
            let pte: u64 = unimplemented!(); //*MPA.address_to_pointer::<u64>(pt + i*8);
            if pte == 0 {
                continue;
            }

            for _ in 0..(2 - level) {
                print!("__ ");
            }

            if pte & PTE_RWXV == PTE_VALID {
                assert!(level != 0);
                let child = (pte >> 10) << 12;
                if child >= MAX_GUEST_PHYSICAL_ADDRESS {
                    println!("{:#x}: {:#x} (bad ppn)", addr, pte);
                } else {
                    println!("{:#x}: {:#x}", addr, pte);
                    print_guest_page_table(child, level - 1, addr);
                    //break;
                }
            } else if pte & PTE_VALID != 0 {
                println!("{:#x} -> {:#x}", addr, (pte >> 10) << 12);
            } else if pte != 0 {
                println!("{:#x}: {:#x} (not valid)", addr, pte);
            }
        }
    }
}

pub fn flush_shadow_page_table(page_table_region: &mut PageTableRegion) {
    unsafe {
        for &pa in &[UVA.pa(), KVA.pa(), MVA.pa()] {
            let va = pa2va(pa) as *mut u64;
            for i in 0..(DIRECT_MAP_PT_INDEX/8) {
                let pte = va.add(i as usize);
                if *pte & PTE_RWXV == PTE_VALID {
                    let page = (*pte >> 10) << 12;
                    clear_page_table(page);
                    free_page(pa2va(page) as *mut Page);
                }
                *pte = 0;
            }
        }

        asm!("sfence.vma" ::: "memory" : "volatile");
    }
}

pub fn handle_sfence_vma(state: &mut Context, _instruction: Instruction) {
    flush_shadow_page_table(&mut state.page_table_region);
    // println!("sfence.vma");
}

pub fn read64(guest_memory: &MemoryRegion, page_table_ppn: u64, guest_va: u64) -> Option<u64> {
    let guest_page = guest_va & !0xfff;
    if let Some(page_translation) = translate_guest_address(guest_memory, page_table_ppn << 12, guest_page) {
        if mpa2pa(page_translation.guest_pa).is_some() {
            // assert!(!virtio::is_queue_access(state, page_translation.guest_pa));

            let guest_pa = (page_translation.guest_pa & !0xfff) | (guest_va & 0xfff);
            return guest_memory.get(guest_pa);
        }
    }

    None
}
