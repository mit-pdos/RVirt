use core::mem;
use core::ops::{Index, IndexMut};
use crate::pmap;

pub struct MemoryRegion<T: Copy = u64> {
    ptr: *mut T,
    base_address: u64,
    length_bytes: u64,
}

unsafe impl<T: Copy + Send> Send for MemoryRegion<T> {}

impl<T: Copy> MemoryRegion<T> {
    pub unsafe fn new(address: u64, length: u64) -> Self {
        assert_eq!(length % mem::size_of::<T>() as u64, 0);
        Self {
            ptr: address as *mut T,
            base_address: pmap::va2pa(address),
            length_bytes: length,
        }
    }

    pub unsafe fn with_base_address(address: u64, base_address: u64, length: u64) -> Self {
        assert_eq!(length % mem::size_of::<T>() as u64, 0);
        Self {
            ptr: address as *mut T,
            base_address,
            length_bytes: length,
        }
    }

    pub fn get(&self, index: u64) -> Option<T> {
        if index % mem::size_of::<T>() as u64 != 0 || index < self.base_address {
            return None;
        }

        let offset = index - self.base_address;
        if offset >= self.length_bytes {
            return None;
        }

        unsafe { Some(*(self.ptr.add(offset as usize / mem::size_of::<T>()))) }
    }

    pub fn base(&self) -> u64 {
        self.base_address
    }

    pub fn len(&self) -> u64 {
        self.length_bytes
    }

    pub fn in_region(&self, addr: u64) -> bool {
        addr >= self.base_address && addr < self.base_address + self.length_bytes
    }

    pub fn slice(&self, index: u64, len: u64) -> &[u8] {
        assert!(index >= self.base_address);

        let offset = index - self.base_address;
        assert!(offset < self.length_bytes);

        assert!(self.length_bytes - offset >= len);
        unsafe {
            core::slice::from_raw_parts((self.ptr as *mut u8).wrapping_add(offset as usize),
                                        len as usize)
        }
    }

    pub fn slice_mut(&mut self, index: u64, len: u64) -> &mut [u8] {
        assert!(index >= self.base_address);

        let offset = index - self.base_address;
        assert!(offset < self.length_bytes);

        assert!(self.length_bytes - offset >= len);
        unsafe {
            core::slice::from_raw_parts_mut((self.ptr as *mut u8).wrapping_add(offset as usize),
                                            len as usize)
        }
    }
}

impl<T: Copy> Index<u64> for MemoryRegion<T> {
    type Output = T;
    /// Return a reference to a u64 index many *bytes* into the memory region. The value of index
    /// must be divisible by sizeof(T).
    fn index(&self, index: u64) -> &T {
        assert_eq!(index % mem::size_of::<T>() as u64, 0);
        assert!(index >= self.base_address);

        let offset = index - self.base_address;
        assert!(offset < self.length_bytes);

        unsafe { &*(self.ptr.add(offset as usize / mem::size_of::<T>())) }
    }
}

impl<T: Copy> IndexMut<u64> for MemoryRegion<T> {
    /// Return a reference to a u64 index many *bytes* into the memory region. The value of index
    /// must be divisible by sizeof(T).
    fn index_mut(&mut self, index: u64) -> &mut T {
        assert_eq!(index % mem::size_of::<T>() as u64, 0);
        assert!(index >= self.base_address);

        let offset = index - self.base_address;
        assert!(offset < self.length_bytes);

        unsafe { &mut *(self.ptr.add(offset as usize / mem::size_of::<T>())) }
    }
}

/// Use to represent a region containing page tables. All addresses are in terms of *physical
/// addresses* to simplify usage.
pub struct PageTableRegion {
    region: MemoryRegion,
    end_pa: u64,
}
impl PageTableRegion {
    pub fn new(region: MemoryRegion) -> Self {
        assert_eq!((region.ptr as u64) % 4096, 0);
        assert_eq!(region.length_bytes % 4096, 0);

        let end_pa = pmap::va2pa(region.ptr as u64) + region.length_bytes;

        Self {
            region,
            end_pa,
        }
    }

    pub unsafe fn set_pte_unchecked(&mut self, pte_address: u64, pte_value: u64) {
        self.region[pte_address] = pte_value;
    }

    pub fn set_leaf_pte(&mut self, pte_address: u64, pte_value: u64) {
        assert!(pte_value & 0xf != 0x1);
        assert!(!self.inside_region(pte_value));
        self.region[pte_address] = pte_value;
    }

    pub fn set_nonleaf_pte(&mut self, pte_address: u64, pte_value: u64) {
        assert_eq!(pte_value & 0xf, 0x1);
        assert!(self.inside_region(pte_value));
        self.region[pte_address] = pte_value;
    }

    pub fn set_invalid_pte(&mut self, pte_address: u64, pte_value: u64) {
        assert_eq!(pte_value & 0x1, 0);
        self.region[pte_address] = pte_value;
    }

    // Returns a conservative answer of whether the pte could map some memory that overlapped this
    // region.
    fn inside_region(&self, pte: u64) -> bool {
        // since we don't know page size (and because we know all mappings will point to physical
        // addresses larger than the end of this region) we only check that the start of the page is
        // beyond the end of this region.
        ((pte >> 10) << 12) < self.end_pa
    }
}

impl Index<u64> for PageTableRegion {
    type Output = u64;
    /// Return a reference to the pte at physical address `address`. This must be divisible by 8 and
    /// inside the memory region.
    fn index(&self, address: u64) -> &u64 {
        &self.region[address]
    }
}
