
// References:
//
// https://github.com/qemu/qemu/blob/d522fba24478474911b0e6e488b6d1dcf1af54f8/hw/net/cadence_gem.c
// https://github.com/torvalds/linux/blob/master/drivers/net/ethernet/cadence/macb_main.c
// https://www.yumpu.com/en/document/view/31739994/gigabit-ethernet-mac-gem-technical-data-sheet-cadence-

use crate::memory_region::MemoryRegion;
use super::*;

const GEM_DMACFG: u64 = 0x00000010;

const GEM_DMACFG_ADDR_64B: u32 = 1 << 30;

const VIRTIO_MTU: u16 = 2048;

#[repr(transparent)]
struct RxDesc([u32; 4]);
#[repr(transparent)]
struct TxDesc([u32; 4]);

/// Driver for the Cadence GEM Ethernet device.
pub struct MacbDriver {
    control_registers: MemoryRegion<u32>,
    mac: [u8; 6],

    rx_buffers: [[u8; 2048]; 8],
    rx_queue: [RxDesc; 8],
    tx_buffers: [[u8; 2048]; 8],
    tx_queue: [TxDesc; 8],
}

impl Driver for MacbDriver {
    const DEVICE_ID: u32 = 1;
    const FEATURES: u64 = VIRTIO_NET_F_MAC | VIRTIO_NET_F_MTU;
    const QUEUE_NUM_MAX: u32 = 2;

    fn interrupt(device: &mut GuestDevice<Self>, _guest_memory: &mut MemoryRegion) -> bool {
        false
    }
    fn doorbell(device: &mut GuestDevice<Self>, _guest_memory: &mut MemoryRegion, queue: u32) {

    }

    fn read_config_u8(device: &GuestDevice<Self>, _guest_memory: &mut MemoryRegion, offset: u64) -> u8 {
        match offset {
            0..=5 => device.host_driver.mac[offset as usize],
            10 => VIRTIO_MTU.to_le_bytes()[0],
            11 => VIRTIO_MTU.to_le_bytes()[1],
            _ => 0
        }
    }
    fn write_config_u8(device: &mut GuestDevice<Self>, _guest_memory: &mut MemoryRegion, offset: u64, value: u8) {
        match offset {
            0..=5 => {
                device.host_driver.mac[offset as usize] = value;
                unimplemented!(); // TODO: set device MAC to updated value
            }
            _ => {}
        }
    }

    fn reset(device: &mut GuestDevice<Self>, _guest_memory: &mut MemoryRegion) {

    }
}
