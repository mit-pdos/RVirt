use crate::fdt::{self, MachineMeta};
use crate::trap::{ShadowState, MAX_TSTACK_ADDR};
use core::{mem, ptr};
use riscv_decode::Instruction;
use spin::Mutex;

const PAGE_SIZE: u64 = 4096;

const PAGE_TABLE_SHIFT: u32 = 9;

const PTE_VALID: u64 = 0x1;
const PTE_READ: u64 = 0x2;
const PTE_WRITE: u64 = 0x4;
const PTE_EXECUTE: u64 = 0x8;
const PTE_USER: u64 = 0x10;
const PTE_GLOBAL: u64 = 0x20;
const PTE_ACCESSED: u64 = 0x40;
const PTE_DIRTY: u64 = 0x80;
const PTE_RSV_MASK: u64 = 0x300;

const PTE_AD: u64 = PTE_ACCESSED | PTE_DIRTY;
const PTE_RWXV: u64 = PTE_READ | PTE_WRITE | PTE_EXECUTE | PTE_VALID;

const GUEST_PPN_OFFSET: u64 = fdt::VM_RESERVATION_SIZE as u64 / PAGE_SIZE;

pub const ROOT: u64 = 0x80010000;
pub const HVA_ROOT: u64 = 0x80011000;
pub const UVA_ROOT: u64 = 0x80012000;
pub const KVA_ROOT: u64 = 0x80013000;
pub const MVA_ROOT: u64 = 0x80014000;
pub const MPA_ROOT: u64 = 0x80015000;
pub const BOOT_PAGE_TABLE: u64 = 0x80016000;

pub const HVA_INDEX: u64 = 0;
pub const HPA_INDEX: u64 = 1;
pub const UVA_INDEX: u64 = 2;
pub const KVA_INDEX: u64 = 3;
pub const MVA_INDEX: u64 = 4;
pub const MPA_INDEX: u64 = 5;

pub const HVA_OFFSET: u64 = HVA_INDEX << 39;
pub const HPA_OFFSET: u64 = HPA_INDEX << 39;
pub const UVA_OFFSET: u64 = UVA_INDEX << 39;
pub const KVA_OFFSET: u64 = KVA_INDEX << 39;
pub const MVA_OFFSET: u64 = MVA_INDEX << 39;
pub const MPA_OFFSET: u64 = MPA_INDEX << 39;

pub const HYPERVISOR_HOLE: u64 = 0xffffffff_c0000000;
pub const HVA_TO_XVA: u64 = HYPERVISOR_HOLE - 0x40000000;

/// Host physical address
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Paddr(u64);
impl Paddr {
    // TODO: Fix this mapping
    fn to_virtual(self) -> Vaddr { Vaddr(self.0) }
}

/// Guest physical address
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Gaddr(u64);

/// Guest virtual address
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct Vaddr(u64);
impl Vaddr {
    fn to_pointer<T>(self) -> *const T { self.0 as usize as *const T }
    fn to_pointer_mut<T>(self) -> *mut T { self.0 as usize as *mut T }
}

fn pa2va(pa: u64) -> u64 { pa + HPA_OFFSET }

#[repr(transparent)]
#[derive(Copy, Clone)]
struct PageTableEntry(u64);

#[repr(align(4096))]
struct PageTable([PageTableEntry; 512]);

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
    free.unwrap() as *const FreePage as *mut Page
}
fn free_page(page: *mut Page) {
    let mut free_list = FREE_LIST.lock();
    let mut free_page: &mut FreePage = unsafe { &mut *(page as *mut FreePage) };
    free_page.0 = free_list.take();
    *free_list = Some(free_page)
}

unsafe fn pte_for_addr(addr: u64) -> *mut PageTableEntry {
    // These ranges use huge pages...
    assert!(addr >> 39 != HVA_INDEX);

    let mut page_table = &mut *(ROOT as *mut PageTable);
    for level in 0..3 {
        let pte_index = ((addr >> (39 - PAGE_TABLE_SHIFT * level)) & 0x1ff) as usize;
        let pte = page_table.0[pte_index].0;

        if pte & PTE_VALID != 0 {
            assert_eq!(pte & (PTE_READ | PTE_WRITE | PTE_EXECUTE), 0);
            page_table = &mut *(((pte >> 10) << 12) as *mut PageTable);
        } else {
            let page = alloc_page();
            page_table.0[pte_index].0 = ((page as u64) >> 2) | PTE_VALID;
            page_table = &mut *(page as *mut PageTable);
        }
    }
    &page_table.0[((addr >> 12) & 0x1ff) as usize] as *const PageTableEntry as *mut _
}

pub unsafe fn map_region(va: u64, pa: u64, len: u64, perm: u64) {
    assert_eq!(len % PAGE_SIZE, 0);
    assert_eq!(va % PAGE_SIZE, 0);
    assert_eq!(pa % PAGE_SIZE, 0);

    let npages = len / PAGE_SIZE;
    for p in 0..npages  {
        let pte = pte_for_addr(va + p * PAGE_SIZE);
        (*pte).0 = (pa + p * PAGE_SIZE) | perm | PTE_VALID;
    }
}

pub fn init(machine: &MachineMeta) {
    let mut i = 0;
    let mut addr = MAX_TSTACK_ADDR;
    while addr < machine.hpm_offset as usize + fdt::VM_RESERVATION_SIZE {
        free_page(addr as *mut Page);
        addr += PAGE_SIZE as usize;
        i += 1;
    }

    unsafe {
        // Zero out page tables
        ptr::write_bytes(ROOT as *mut u8, 0, (BOOT_PAGE_TABLE - ROOT) as usize);

        // Root page table
        *((ROOT + 0x00) as *mut u64) = (HVA_ROOT >> 2) | PTE_VALID;
        *((ROOT + 0x08) as *mut u64) = PTE_AD | PTE_RWXV;
        *((ROOT + 0x10) as *mut u64) = (UVA_ROOT >> 2) | PTE_VALID;
        *((ROOT + 0x18) as *mut u64) = (KVA_ROOT >> 2) | PTE_VALID;
        *((ROOT + 0x20) as *mut u64) = (MVA_ROOT >> 2) | PTE_VALID;
        *((ROOT + 0x28) as *mut u64) = (MPA_ROOT >> 2) | PTE_VALID;

        *((HVA_ROOT + 0x00) as *mut u64) = 0x00000000 | PTE_AD | PTE_RWXV;
        *((HVA_ROOT + 0x08) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((HVA_ROOT + 0x10) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((HVA_ROOT + 0x18) as *mut u64) = 0x30000000 | PTE_AD | PTE_RWXV;

        *((MPA_ROOT + 0x00) as *mut u64) = 0x00000000 | PTE_AD | PTE_USER | PTE_RWXV;
        *((MPA_ROOT + 0x08) as *mut u64) = 0x10000000 | PTE_AD | PTE_USER | PTE_RWXV;
        map_region(MPA_OFFSET + 0x80000000,
                   machine.guest_shift + 0x80000000,
                   machine.gpm_size,
                   PTE_AD | PTE_RWXV);

        // Map hypervisor into all address spaces at same location.
        // TODO: Make sure this address in compatible with Linux.
        *((ROOT + 0xff8) as *mut u64) = (HVA_ROOT >> 2) | PTE_VALID;
        *((HVA_ROOT + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((UVA_ROOT + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((KVA_ROOT + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((MVA_ROOT + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
        *((MPA_ROOT + 0xff8) as *mut u64) = 0x20000000 | PTE_AD | PTE_RWXV;
    }

    csrw!(satp, 9 << 60 | (ROOT >> 12) as usize);
}

pub fn handle_sfence_vma(state: &mut ShadowState, instruction: Instruction) {
    unimplemented!()
}
