#![allow(unused)]

use arrayvec::ArrayVec;
use byteorder::{ByteOrder, LittleEndian};
use crate::memory_region::MemoryRegion;

pub mod macb;

#[allow(unused)]
mod constants {
    pub const VENDOR_ID: u32 = 0x1af4;
    pub const MAGIC_VALUE: u32 = 0x74726976;

    pub const REG_MAGIC_VALUE: u64 = 0x0;
    pub const REG_VERSION: u64 = 0x004;
    pub const REG_DEVICE_ID: u64 = 0x008;
    pub const REG_VENDOR_ID: u64 = 0x00c;
    pub const REG_HOST_FEATURES: u64 = 0x010;
    pub const REG_HOST_FEATURES_SEL: u64 = 0x014;
    pub const REG_GUEST_FEATURES: u64 = 0x020;
    pub const REG_GUEST_FEATURES_SEL: u64 = 0x024;
    pub const REG_GUEST_PAGE_SIZE: u64 = 0x028;
    pub const REG_QUEUE_SEL: u64 = 0x030;
    pub const REG_QUEUE_NUM_MAX: u64 = 0x034;
    pub const REG_QUEUE_NUM: u64 = 0x038;
    pub const REG_QUEUE_ALIGN: u64 = 0x03c;
    pub const REG_QUEUE_PFN: u64 = 0x040;
    pub const REG_QUEUE_NOTIFY: u64 = 0x050;
    pub const REG_INTERRUPT_STATUS: u64 = 0x060;
    pub const REG_INTERRUPT_ACK: u64 = 0x064;
    pub const REG_STATUS: u64 = 0x070;

    pub const STATUS_ACKNOWLEDGE: u32 = 1;
    pub const STATUS_DRIVER: u32 = 2;
    pub const STATUS_FAILED: u32 = 128;
    pub const STATUS_FEATURES_OK: u32 = 8;
    pub const STATUS_DRIVER_OK: u32 = 4;
    pub const STATUS_NEEDS_RESET: u32 = 64;

    pub const VIRTIO_NET_F_MTU: u64 = 1 << 3;
    pub const VIRTIO_NET_F_MAC: u64 = 1 << 5;

    pub const VIRTQ_DESC_F_NEXT: u16 = 1;
    pub const VIRTQ_DESC_F_WRITE: u16 = 2;

    pub const MAX_QUEUES: usize = 4;
}
pub use constants::*;

pub trait Driver: Sized {
    const DEVICE_ID: u32;
    const FEATURES: u64;
    const QUEUE_NUM_MAX: u32;

    fn interrupt(device: &mut GuestDevice<Self>, guest_memory: &mut MemoryRegion) -> bool;
    fn doorbell(device: &mut GuestDevice<Self>, guest_memory: &mut MemoryRegion, queue: u32);

    fn read_config_u8(device: &GuestDevice<Self>, guest_memory: &mut MemoryRegion, offset: u64) -> u8;
    fn read_config_u32(device: &GuestDevice<Self>, guest_memory: &mut MemoryRegion, offset: u64) -> u32 {
        u32::from_le_bytes([
            Self::read_config_u8(device, guest_memory, offset),
            Self::read_config_u8(device, guest_memory, offset+1),
            Self::read_config_u8(device, guest_memory, offset+2),
            Self::read_config_u8(device, guest_memory, offset+3),
        ])
    }
    fn write_config_u8(device: &mut GuestDevice<Self>, guest_memory: &mut MemoryRegion, offset: u64, value: u8);
    fn write_config_u32(device: &mut GuestDevice<Self>, guest_memory: &mut MemoryRegion, offset: u64, value: u32) {
        Self::write_config_u8(device, guest_memory, offset, value.to_le_bytes()[0]);
        Self::write_config_u8(device, guest_memory, offset+1, value.to_le_bytes()[1]);
        Self::write_config_u8(device, guest_memory, offset+2, value.to_le_bytes()[2]);
        Self::write_config_u8(device, guest_memory, offset+3, value.to_le_bytes()[3]);
    }

    fn reset(device: &mut GuestDevice<Self>, guest_memory: &mut MemoryRegion);
}

pub struct DescriptorTable<'a> {
    desc: &'a [u8],
    avail: &'a [u8],
    used: &'a mut [u8],
    queue_size: usize,
}
#[allow(unused)]
impl<'a> DescriptorTable<'a> {
    fn desc_addr(&self, index: usize) -> u64 { LittleEndian::read_u64(&self.desc[16*index..]) }
    fn desc_len(&self, index: usize) -> u32 { LittleEndian::read_u32(&self.desc[8+16*index..]) }
    fn desc_flags(&self, index: usize) -> u16 { LittleEndian::read_u16(&self.desc[12+16*index..]) }
    fn desc_next(&self, index: usize) -> u16 { LittleEndian::read_u16(&self.desc[14+16*index..]) }

    fn avail_flags(&self) -> u16 { LittleEndian::read_u16(&self.avail) }
    fn avail_idx(&self) -> u16 { LittleEndian::read_u16(&self.avail[2..]) }
    fn avail_ring(&self, index: usize) -> u16 { LittleEndian::read_u16(&self.avail[4+2*index..]) }

    fn used_flags(&self) -> u16 { LittleEndian::read_u16(&self.used) }
    fn used_idx(&self) -> u16 { LittleEndian::read_u16(&self.used[2..]) }
    fn used_ring_id(&self, index: usize) -> u32 { LittleEndian::read_u32(&self.used[4+8*index..]) }
    fn used_ring_len(&self, index: usize) -> u32 { LittleEndian::read_u32(&self.used[8+8*index..]) }

    fn set_used_flags(&mut self, value: u16) { LittleEndian::write_u16(&mut self.used, value) }
    fn set_used_idx(&mut self, value: u16) { LittleEndian::write_u16(&mut self.used[2..], value) }
    fn set_used_ring_id(&mut self, index: usize, value: u32) { LittleEndian::write_u32(&mut self.used[4+8*index..], value) }
    fn set_used_ring_len(&mut self, index: usize, value: u32) { LittleEndian::write_u32(&mut self.used[8+8*index..], value) }
}

pub struct GuestDevice<D: Driver> {
    host_features_sel: u32,

    guest_features_sel: u32,
    guest_features: u64,

    guest_page_size: u32,

    queue_sel: u32,
    queue_num: [u32; MAX_QUEUES],
    queue_align: [u32; MAX_QUEUES],
    queue_pfn: [u32; MAX_QUEUES],

    interrupt_status: u32,
    status: u32,

    host_driver: D,
}

impl<D: Driver> GuestDevice<D> {
    pub fn new(host_driver: D) -> Self {
        Self {
            host_features_sel: 0,
            guest_features_sel: 0,
            guest_features: 0,
            guest_page_size: 4096,
            queue_sel: 0,
            queue_num: [0; MAX_QUEUES],
            queue_align: [0; MAX_QUEUES],
            queue_pfn: [0; MAX_QUEUES],
            interrupt_status: 0,
            status: 0,
            host_driver,
        }
    }

    pub fn read_u8(&mut self, guest_memory: &mut MemoryRegion, offset: u64) -> u8 {
        if offset > 0x100 {
            D::read_config_u8(self, guest_memory, offset)
        } else {
            0
        }
    }

    pub fn read_u32(&mut self, guest_memory: &mut MemoryRegion, offset: u64) -> u32 {
        if offset % 4 != 0 {
            return 0;
        }

        if offset > 0x100 {
            return D::read_config_u32(self, guest_memory, offset);
        }

        match offset {
            REG_MAGIC_VALUE => MAGIC_VALUE,
            REG_VERSION => 1,
            REG_DEVICE_ID => D::DEVICE_ID,
            REG_VENDOR_ID => VENDOR_ID,
            REG_HOST_FEATURES if self.host_features_sel == 0 => (D::FEATURES & 0xffffffff) as u32,
            REG_HOST_FEATURES if self.host_features_sel == 1 => ((D::FEATURES >> 32) & 0xffffffff) as u32,
            REG_HOST_FEATURES => 0,
            REG_HOST_FEATURES_SEL => self.host_features_sel,
            REG_GUEST_FEATURES => 0,
            REG_GUEST_FEATURES_SEL => self.guest_features_sel,
            REG_GUEST_PAGE_SIZE => self.guest_page_size,
            REG_QUEUE_SEL => self.queue_sel,
            REG_QUEUE_NUM_MAX => D::QUEUE_NUM_MAX,
            REG_QUEUE_NUM => self.queue_num[self.queue_sel as usize],
            REG_QUEUE_ALIGN => self.queue_align[self.queue_sel as usize],
            REG_QUEUE_PFN => self.queue_pfn[self.queue_sel as usize],
            REG_QUEUE_NOTIFY => 0,
            REG_INTERRUPT_STATUS => 0,
            REG_INTERRUPT_ACK => 0,
            REG_STATUS => self.status,
            _ => 0,
        }
    }

    pub fn write_u8(&mut self, guest_memory: &mut MemoryRegion, offset: u64, value: u8)  {
        if offset > 0x100 {
            D::write_config_u8(self, guest_memory, offset, value);
        }
    }

    pub fn write_u32(&mut self, guest_memory: &mut MemoryRegion, offset: u64, value: u32) {
        if offset % 4 != 0 {
            return;
        }

        if offset > 0x100 {
            D::write_config_u32(self, guest_memory, offset, value);
            return;
        }

        match offset {
            REG_HOST_FEATURES_SEL => self.host_features_sel = value,
            REG_GUEST_FEATURES if self.guest_features_sel == 0 => self.guest_features = (self.guest_features & !0xffffffff) | value as u64,
            REG_GUEST_FEATURES if self.guest_features_sel == 1 => self.guest_features = (self.guest_features & 0xffffffff) | ((value as u64) << 32),
            REG_GUEST_FEATURES_SEL => self.guest_features_sel = value,
            REG_GUEST_PAGE_SIZE => self.guest_page_size = value,
            REG_QUEUE_SEL => self.queue_sel = value,
            REG_QUEUE_NUM => self.queue_num[self.queue_sel as usize] = value,
            REG_QUEUE_ALIGN => self.queue_align[self.queue_sel as usize] = value,
            REG_QUEUE_PFN => self.queue_pfn[self.queue_sel as usize] = value,
            REG_QUEUE_NOTIFY => D::doorbell(self, guest_memory, value),
            REG_INTERRUPT_ACK => self.interrupt_status &= !value,
            REG_STATUS => {
                if value == 0 {
                    self.reset();
                    D::reset(self, guest_memory);
                } else {
                    self.status = value;
                }
            }
            _ => {},
        }
    }

    /// Returns true if the interrupt should be forwarded onto the guest, false otherwise.
    pub fn interrupt(&mut self, guest_memory: &mut MemoryRegion) -> bool {
        D::interrupt(self, guest_memory)
    }

    fn reset(&mut self) {
        self.host_features_sel = 0;
        self.guest_features_sel = 0;
        self.guest_features = 0;
        self.guest_page_size = 4096;

        self.queue_sel = 0;
        self.queue_num = [0; MAX_QUEUES];
        self.queue_align = [0; MAX_QUEUES];
        self.queue_pfn = [0; MAX_QUEUES];

        self.interrupt_status = 0;
    }

    fn with_buffer<F: FnOnce(&[&[u8]]) -> Option<u32>>(&mut self, guest_memory: &mut MemoryRegion, queue: u32, f: F) {
        let dt = self.get_queue(guest_memory, queue);

        if dt.avail_idx() == dt.used_idx() {
            return;
        }

        let mut ranges = ArrayVec::<[(u64, u32); 16]>::new();

        let idx = (dt.used_idx() as usize + 1) % dt.queue_size;
        let id = dt.avail_ring(idx) as usize;

        let mut flags = VIRTQ_DESC_F_NEXT;
        let mut next_id = id;
        while flags & VIRTQ_DESC_F_NEXT != 0 {
            let addr = dt.desc_addr(next_id);
            let len = dt.desc_len(next_id);
            flags = dt.desc_flags(next_id);
            next_id = dt.desc_next(next_id) as usize;

            ranges.push((addr, len));
        }

        // Handling the borrow checker is a bit tricky here. At this point, we let the lifetime of
        // `dt` end so that its borrow of `guest_memory` ends. Then we borrow a bunch of slices from
        // `guest_memory` and pass them to `f`. Once that function returns, we have `buffers` go out
        // of scope so that we can borrow `guest_memory` again to make a DescriptorTable.
        let consume_buffers = {
            let mut buffers = ArrayVec::<[&[u8]; 16]>::new();
            for (addr, len) in ranges {
                buffers.push(guest_memory.slice(addr, len as u64));
            }

            f(&*buffers)
        };

        if let Some(len) = consume_buffers {
            let mut dt = self.get_queue(guest_memory, queue);
            dt.set_used_ring_id(idx, id as u32);
            dt.set_used_ring_len(idx, len);
            dt.set_used_idx(dt.used_idx().wrapping_add(1));
        }
    }

    fn get_queue<'a>(&'a mut self, guest_memory: &'a mut MemoryRegion, queue: u32) -> DescriptorTable<'a> {
        let pfn = self.queue_pfn[queue as usize];
        let queue_size = self.queue_num[queue as usize] as usize;
        let align = self.queue_align[queue as usize] as usize;

        let desc_size = 16 * queue_size;
        let avail_size = 6 + 2 * queue_size;
        let used_size = 6 + 8 * queue_size;

        let used_start = ((desc_size + avail_size + (align - 1)) % align) - align;

        let slice = guest_memory.slice_mut(pfn as u64 * 4096, (used_start + used_size) as u64);
        let (desc, slice) = slice.split_at_mut(desc_size);
        let (avail, slice) = slice.split_at_mut(used_size);
        let (_, used) = slice.split_at_mut(used_start - desc_size - avail_size);

        DescriptorTable {
            desc,
            avail,
            used,
            queue_size
        }
    }
}
