use arrayvec::ArrayVec;
use byteorder::{ByteOrder, LittleEndian};
use crate::memory_region::MemoryRegion;
use core::ops::Range;

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

    pub const VIRTIO_NET_RX_QUEUE: u32 = 0;
    pub const VIRTIO_NET_TX_QUEUE: u32 = 1;

    pub const VIRTQ_AVAIL_F_NO_INTERRUPT: u16 = 1;
    pub const VIRTQ_USED_F_NO_NOTIFY: u16 = 1;

    pub const VIRTIO_INT_STATUS_USED_BUFFER: u32 = 0x1;

    pub const MAX_QUEUES: usize = 4;
}
pub use constants::*;

pub trait Driver: Sized {
    const DEVICE_ID: u32;
    const FEATURES: u64;
    const QUEUE_NUM_MAX: u32;
    const MAX_CONTEXTS: u64;

    /// Returns the id of the guest that should process this interrupt.
    fn demux_interrupt(&mut self) -> u64;
    fn interrupt(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion) -> bool;
    fn doorbell(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion, queue: u32);

    fn read_config_u8(&mut self, local: &mut LocalContext, offset: u64) -> u8;
    fn read_config_u32(&mut self, local: &mut LocalContext, offset: u64) -> u32 {
        u32::from_le_bytes([
            self.read_config_u8(local, offset),
            self.read_config_u8(local, offset+1),
            self.read_config_u8(local, offset+2),
            self.read_config_u8(local, offset+3),
        ])
    }
    fn write_config_u8(&mut self, local: &mut LocalContext, offset: u64, value: u8);
    fn write_config_u32(&mut self, local: &mut LocalContext, offset: u64, value: u32) {
        self.write_config_u8(local, offset, value.to_le_bytes()[0]);
        self.write_config_u8(local, offset+1, value.to_le_bytes()[1]);
        self.write_config_u8(local, offset+2, value.to_le_bytes()[2]);
        self.write_config_u8(local, offset+3, value.to_le_bytes()[3]);
    }

    fn reset_context(&mut self, local: &mut LocalContext);

    fn new_context(&mut self, guestid: u64) -> LocalContext {
        assert!(guestid < Self::MAX_CONTEXTS);
        LocalContext {
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
        }
    }

    fn read_u8(&mut self, local: &mut LocalContext, _guest_memory: &mut MemoryRegion, offset: u64) -> u8 {
        if offset >= 0x100 {
            let value = self.read_config_u8(local, offset - 0x100);
            value
        } else {
            0
        }
    }

    fn read_u32(&mut self, local: &mut LocalContext, _guest_memory: &mut MemoryRegion, offset: u64) -> u32 {
        if offset % 4 != 0 {
            return 0;
        }

        if offset >= 0x100 {
            return self.read_config_u32(local, offset - 0x100);
        }

        let value = match offset {
            REG_MAGIC_VALUE => MAGIC_VALUE,
            REG_VERSION => 1,
            REG_DEVICE_ID => Self::DEVICE_ID,
            REG_VENDOR_ID => VENDOR_ID,
            REG_HOST_FEATURES if local.host_features_sel == 0 => (Self::FEATURES & 0xffffffff) as u32,
            REG_HOST_FEATURES if local.host_features_sel == 1 => ((Self::FEATURES >> 32) & 0xffffffff) as u32,
            REG_HOST_FEATURES => 0,
            REG_HOST_FEATURES_SEL => local.host_features_sel,
            REG_GUEST_FEATURES => 0,
            REG_GUEST_FEATURES_SEL => local.guest_features_sel,
            REG_GUEST_PAGE_SIZE => local.guest_page_size,
            REG_QUEUE_SEL => local.queue_sel,
            REG_QUEUE_NUM_MAX => Self::QUEUE_NUM_MAX,
            REG_QUEUE_NUM => local.queue_num[local.queue_sel as usize],
            REG_QUEUE_ALIGN => local.queue_align[local.queue_sel as usize],
            REG_QUEUE_PFN => local.queue_pfn[local.queue_sel as usize],
            REG_QUEUE_NOTIFY => 0,
            REG_INTERRUPT_STATUS => local.interrupt_status,
            REG_INTERRUPT_ACK => 0,
            REG_STATUS => local.status,
            _ => 0,
        };
        value
    }

    fn write_u8(&mut self, local: &mut LocalContext, _guest_memory: &mut MemoryRegion, offset: u64, value: u8)  {
        if offset >= 0x100 {
            self.write_config_u8(local, offset - 0x100, value);
        }
    }

    fn write_u32(&mut self, local: &mut LocalContext, guest_memory: &mut MemoryRegion, offset: u64, value: u32) {
        if offset % 4 != 0 {
            return;
        }

        if offset >= 0x100 {
            self.write_config_u32(local, offset - 0x100, value);
            return;
        }

        match offset {
            REG_HOST_FEATURES_SEL => local.host_features_sel = value,
            REG_GUEST_FEATURES if local.guest_features_sel == 0 => local.guest_features = (local.guest_features & !0xffffffff) | value as u64,
            REG_GUEST_FEATURES if local.guest_features_sel == 1 => local.guest_features = (local.guest_features & 0xffffffff) | ((value as u64) << 32),
            REG_GUEST_FEATURES_SEL => local.guest_features_sel = value,
            REG_GUEST_PAGE_SIZE => local.guest_page_size = value,
            REG_QUEUE_SEL => local.queue_sel = value,
            REG_QUEUE_NUM => local.queue_num[local.queue_sel as usize] = value,
            REG_QUEUE_ALIGN => local.queue_align[local.queue_sel as usize] = value,
            REG_QUEUE_PFN => local.queue_pfn[local.queue_sel as usize] = value,
            REG_QUEUE_NOTIFY => self.doorbell(local, guest_memory, value),
            REG_INTERRUPT_ACK => local.interrupt_status &= !value,
            REG_STATUS => {
                if value == 0 {
                    self.reset_context(local);
                } else {
                    local.status = value;
                }
            }
            _ => {},
        }
    }
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

pub struct LocalContext {
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
}
impl LocalContext {
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
        self.status = 0;
    }

    fn driver_ok(&self) -> bool {
        self.status & STATUS_DRIVER_OK != 0
    }

    /// Repeatedly call f with the next buffer in the given queue until the queue is empty or the
    /// function returns None.
    fn with_buffers<F>(&mut self, guest_memory: &mut MemoryRegion, queue: u32, mut f: F) where
        F: FnMut(&[&[u8]]) -> Option<u32>
    {
        self.with_ranges(guest_memory, queue, |guest_memory, ranges| {
            let mut buffers = ArrayVec::<[&[u8]; 64]>::new();
            for &Range { start, end } in ranges {
                buffers.push(guest_memory.slice(start, end - start));
            }
            f(&*buffers)
        });
    }

    /// Repeatedly call f with the next buffer in the given queue until the queue is empty or the
    /// function returns None.
    fn with_ranges<F>(&mut self, guest_memory: &mut MemoryRegion, queue: u32, mut f: F) where
        F: FnMut(&mut MemoryRegion, &[Range<u64>]) -> Option<u32>
    {
        loop {
            let dt = self.get_queue(guest_memory, queue);

            if dt.avail_idx() == dt.used_idx() {
                return;
            }

            let mut ranges = ArrayVec::<[Range<u64>; 64]>::new();

            let idx = (dt.used_idx() as usize + 1) % dt.queue_size;
            let id = dt.avail_ring(idx) as usize;

            let mut flags = VIRTQ_DESC_F_NEXT;
            let mut next_id = id;
            while flags & VIRTQ_DESC_F_NEXT != 0 {
                let addr = dt.desc_addr(next_id);
                let len = dt.desc_len(next_id);
                flags = dt.desc_flags(next_id);
                next_id = dt.desc_next(next_id) as usize;

                ranges.push(Range { start: addr, end: addr + len as u64 });
            }

            if let Some(len) = f(guest_memory, &*ranges) {
                let mut dt = self.get_queue(guest_memory, queue);
                dt.set_used_ring_len(idx, len);
                dt.set_used_ring_id(idx, id as u32);
                dt.set_used_idx(dt.used_idx().wrapping_add(1));

                // if dt.avail_flags() & VIRTQ_AVAIL_F_NO_INTERRUPT == 0 {
                    self.interrupt_status |= VIRTIO_INT_STATUS_USED_BUFFER;
                // }
            } else {
                return;
            }
        }
    }

    fn get_queue<'a>(&'a mut self, guest_memory: &'a mut MemoryRegion, queue: u32) -> DescriptorTable<'a> {
        let pfn = self.queue_pfn[queue as usize];
        let queue_size = self.queue_num[queue as usize] as usize;
        let align = self.queue_align[queue as usize] as usize;
        assert!(align.is_power_of_two());

        let desc_size = 16 * queue_size;
        let avail_size = 6 + 2 * queue_size;
        let used_size = 6 + 8 * queue_size;

        let used_start = desc_size + avail_size;
        let used_start = ((used_start - 1) | (align - 1)) + 1;

        let slice = guest_memory.slice_mut(pfn as u64 * 4096, (used_start + used_size) as u64);
        let (desc, slice) = slice.split_at_mut(desc_size);
        let (avail, slice) = slice.split_at_mut(avail_size);
        let (_, used) = slice.split_at_mut(used_start - desc_size - avail_size);

        // println!("queue = {}", queue);
        // println!("  align = {}", align);
        // println!("  queue_size = {}", queue_size);
        // println!("  desc_size = {}", desc_size);
        // println!("  avail_size = {}", avail_size);
        // println!("  desc_size + avail_size - 1 = {}", desc_size + avail_size - 1);
        // println!("  used_start = {}", used_start);

        DescriptorTable {
            desc,
            avail,
            used,
            queue_size
        }
    }
}
