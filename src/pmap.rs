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
const PTE_RWX: u64 = PTE_READ | PTE_WRITE | PTE_EXECUTE;

const GUEST_PPN_OFFSET: u64 = fdt::VM_RESERVATION_SIZE as u64 / PAGE_SIZE;

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

#[repr(transparent)]
#[derive(Copy, Clone)]
struct PageTableEntry(u64);

#[repr(align(4096))]
struct PageTable([PageTableEntry; 512]);

#[repr(transparent)]
struct Page([u8; PAGE_SIZE as usize]);

pub struct PageTableDescriptor {
    root: PageTable,
    asid: u16,
}
impl PageTableDescriptor {
    pub const fn new(asid: u16) -> Self {
        Self {
            root: PageTable([PageTableEntry(0); 512]),
            asid,
        }
    }

    pub fn pte_for_addr(&mut self, addr: Vaddr) -> *mut PageTableEntry {
        let mut page_table = &mut self.root;
        unsafe {
            for level in 0..3 {
                let pte_index = ((addr.0 >> (39 - PAGE_TABLE_SHIFT * level)) & 0x1ff) as usize;
                let pte = page_table.0[pte_index].0;

                if pte & PTE_VALID != 0 {
                    assert_eq!(pte & PTE_RWX, 0);
                    page_table = &mut *(Paddr(pte >> 12).to_virtual().to_pointer_mut());
                } else {
                    let page = alloc_page();
                    page_table.0[pte_index].0 = (page as u64) | PTE_VALID;
                    page_table = &mut *(page as *mut PageTable);
                }
            }

        }

        &page_table.0[((addr.0 >> 12) & 0x1ff) as usize] as *const PageTableEntry as *mut _
    }

    pub unsafe fn map_region(&mut self, Vaddr(va): Vaddr, Paddr(pa): Paddr, len: u64, perm: u64) {
        assert_eq!(len % PAGE_SIZE, 0);
        assert_eq!(va % PAGE_SIZE, 0);
        assert_eq!(pa % PAGE_SIZE, 0);

        let npages = len / PAGE_SIZE;
        for p in 0..npages  {
            let pte = self.pte_for_addr(Vaddr(va + p * PAGE_SIZE));
            (*pte).0 = (pa + p * PAGE_SIZE) | perm | PTE_VALID;
        }
    }
}

const fn make_identity_page_table() -> PageTable {
    let mut t = [PageTableEntry(0); 512];
    t[0] = PageTableEntry(0xcf);
    PageTable(t)
}
static IDENTITY_PAGE_TABLE: PageTable = make_identity_page_table();

static USER_PAGE_TABLE: PageTableDescriptor = PageTableDescriptor::new(0);
static KERNEL_PAGE_TABLE: PageTableDescriptor = PageTableDescriptor::new(1);
static MIXED_PAGE_TABLE: PageTableDescriptor = PageTableDescriptor::new(2);
static PHYSICAL_PAGE_TABLE: PageTableDescriptor = PageTableDescriptor::new(3);

// struct SyncPtr<T>(*mut T);
// unsafe impl<T> Send for SyncPtr<T> {}
// unsafe impl<T> Sync for SyncPtr<T> {}

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

pub fn init(machine: &MachineMeta) {
    let mut addr = MAX_TSTACK_ADDR;
    while addr < machine.hpm_offset as usize + fdt::VM_RESERVATION_SIZE {
        free_page(addr as *mut Page);
        addr += PAGE_SIZE as usize;
    }
}

pub fn handle_sfence_vma(state: &mut ShadowState, instruction: Instruction) {
    unimplemented!()
}
