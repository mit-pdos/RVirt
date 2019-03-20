use core::sync::atomic::AtomicU64;
use core::ops::Index;

pub struct MemoryRegion {
    ptr: *mut u64,
    len: u64,
}

unsafe impl Send for MemoryRegion {}

impl MemoryRegion {
    pub const unsafe fn new(address: u64, length: u64) -> Self {
        Self {
            ptr: address as *mut u64,
            len: length,
        }
    }

    pub fn get_atomicu64(&mut self, index: u64) -> &AtomicU64 {
        assert!(index/8 < self.len);
        unsafe { &*((self.ptr.add(index as usize)) as *mut AtomicU64) }
    }
}

impl Index<u64> for MemoryRegion {
    type Output = u64;
    fn index(&self, index: u64) -> &u64 {
        assert!(index/8 < self.len);
        unsafe { &*(self.ptr.add(index as usize)) }
    }
}

pub struct PageTableRegion {
    region: MemoryRegion,
}
impl PageTableRegion {
    pub fn new(region: MemoryRegion) -> Self {
        assert_eq!((region.ptr as u64) % 4096, 0);
        assert_eq!(region.len % 4096, 0);

        Self { region }
    }

}
