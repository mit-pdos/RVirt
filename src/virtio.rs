use riscv_decode::Instruction;
use crate::context::Context;
use crate::{trap, pmap};

pub const MAX_QUEUES: usize = 4;
pub const MAX_DEVICES: usize = 4;

#[derive(Copy, Clone)]
pub struct Queue {
    /// Address guest thinks queue is mapped at
    guest_pa: u64,
    /// Address queue is actually mapped at
    host_pa: u64,
    /// Number of entries in queue
    size: u64,
}

#[derive(Copy, Clone)]
pub struct Device {
    /// Virtual Queue Index, offset=0x30
    queue_sel: u32,
    queues: [Queue; MAX_QUEUES],
    /// Base host physical address of this device
    host_base_address: u64,
}
impl Device {
    pub fn new(host_base_address: u64) -> Self {
        Self {
            queue_sel: 0,
            queues: [Queue {guest_pa: 0, host_pa: 0, size: 0}; MAX_QUEUES],
            host_base_address,
        }
    }
}

#[inline(always)]
pub fn is_device_access(state: &mut Context, guest_pa: u64) -> bool {
    guest_pa >= 0x10001000 && guest_pa < 0x10001000 + 0x1000 * state.virtio.devices.len() as u64
}

pub unsafe fn handle_device_access(state: &mut Context, guest_pa: u64, pc: u64) -> bool {
    let device = ((guest_pa - 0x10001000) / 0x1000) as usize;
    let offset = guest_pa & 0xfff;
    let host_pa = state.virtio.devices[device].host_base_address + offset;
    // println!("VIRTIO: {:x} -> {:x}", guest_pa, host_pa);

    let read_u32 = |pa: u64| {
        let mut value = *(crate::pmap::pa2va(pa) as *const u32);
        if offset == 0x10 {
            value = value & !(1 << 28); // No VIRTIO_F_INDIRECT_DESC
        } else if offset == 0x34 {
           value = value.min(256); // ensure queues take up at most one page
        }
        value
    };

    let (instruction, decoded, len) = trap::decode_instruction_at_address(state, pc);
    match decoded {
        Some(Instruction::Lw(i)) => {
            let value = read_u32(host_pa);
            // println!("VIRTIO: Read value {:#x} at address {:#x}", value, host_pa);
            trap::set_register(state, i.rd(), value as u64)
        }
        Some(Instruction::Lb(i)) => {
            assert!(offset >= 0x100);
            let value = read_u32(host_pa & !0x3);
            let value = (value >> (8*(host_pa & 0x3))) & 0xff;
            // println!("VIRTIO: Read byte {:#x} at address {:#x}", value, host_pa);
            trap::set_register(state, i.rd(), value as u64)
        }
        Some(Instruction::Sw(i)) => {
            let mut value = trap::get_register(state, i.rs2()) as u32;
            if offset == 0x30 { // QueueSel
                assert!(value < 4);
                state.virtio.devices[device].queue_sel = value;
            } else if offset == 0x38 { // QueueNum
                let queue_sel = state.virtio.devices[device].queue_sel as usize;
                let queue = &mut state.virtio.devices[device].queues[queue_sel];
                queue.size = value as u64;

                // TODO: support changing queue sizes (is this ever done?)
                assert_eq!(queue.host_pa, 0);
            } else if offset == 0x40 { // QueuePFN
                let queue_sel = state.virtio.devices[device].queue_sel as usize;
                let queue = &mut state.virtio.devices[device].queues[queue_sel];

                // TODO: support releasing queues and remove this assert.
                assert_eq!(queue.host_pa, 0);

                if value != 0 {
                    queue.guest_pa = (value as u64) << 12;
                    value += (state.guest_shift >> 12) as u32;
                    queue.host_pa = (value as u64) << 12;
                } else {
                    unimplemented!();
                }

                // Sad, but necessary because we don't know all the places this page is mapped.
                pmap::flush_shadow_page_table(&mut state.shadow_page_tables);

                state.virtio.queue_guest_pages.push(queue.guest_pa);

                let va = pmap::pa2va(queue.host_pa);
                for i in 0..queue.size {
                    let ptr = (va + i * 16) as *mut u64;
                    let value = *ptr;
                    if value != 0 {
                        *ptr = value.wrapping_add(state.guest_shift);
                    }
                }
            }
            *(crate::pmap::pa2va(host_pa) as *mut u32) = value;
        }
        Some(instr) => {
            println!("VIRTIO: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, host_pa, pc);
            loop {}
        }
        None => {
            println!("Unrecognized instruction targetting VIRTIO {:#x} at {:#x}!", instruction, pc);
            loop {}
        }
    }
    csrw!(sepc, pc + len);
    true
}

pub fn is_queue_access(state: &mut Context, guest_page: u64) -> bool {
    for i in 0..state.virtio.queue_guest_pages.len() {
        if state.virtio.queue_guest_pages[i] == guest_page {
            return true;
        }
    }
    false
}

pub unsafe fn handle_queue_access(state: &mut Context, guest_pa: u64, host_pa: u64, pc: u64) -> bool {
    let mut hit_queue = false;
    for d in &state.virtio.devices {
        for q in &d.queues {
            if guest_pa >= q.guest_pa && guest_pa < q.guest_pa + q.size * 16 && guest_pa & 0xf < 8{
                hit_queue = true;
            }
        }
    }

    let (instruction, decoded, len) = trap::decode_instruction_at_address(state, pc);
    if decoded.is_none() {
        println!("Unrecognized instruction targetting VQUEUE {:#x} at {:#x}!", instruction, pc);
        loop {}
    }

    if hit_queue {
        match decoded.unwrap() {
            Instruction::Ld(i) => {
                trap::set_register(state, i.rd(), (*(pmap::pa2va(host_pa) as *const u64)).wrapping_sub(state.guest_shift));
            }
            Instruction::Sd(i) => {
                let value = trap::get_register(state, i.rs2());
                if value == 0 {
                    *(pmap::pa2va(host_pa) as *mut u64) = 0;
                } else if state.guest_memory.in_region(value) {
                    *(pmap::pa2va(host_pa) as *mut u64) = value.wrapping_add(state.guest_shift);
                } else {
                    loop {}
                }
            }
            instr => {
                println!("VQUEUE: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, host_pa, pc);
                loop {}
            }
        }
    } else {
        match decoded.as_ref().unwrap() {
            Instruction::Ld(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const u64)),
            Instruction::Lwu(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const u32) as u64),
            Instruction::Lhu(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const u16) as u64),
            Instruction::Lbu(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const u8) as u64),
            Instruction::Lw(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const i32) as i64 as u64),
            Instruction::Lh(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const i16) as i64 as u64),
            Instruction::Lb(i) => trap::set_register(state, i.rd(), *(pmap::pa2va(host_pa) as *const i8) as i64 as u64),
            Instruction::Sd(i) => *(pmap::pa2va(host_pa) as *mut u64) = trap::get_register(state, i.rs2()),
            Instruction::Sw(i) => *(pmap::pa2va(host_pa) as *mut u32) = trap::get_register(state, i.rs2()) as u32,
            Instruction::Sh(i) => *(pmap::pa2va(host_pa) as *mut u16) = trap::get_register(state, i.rs2()) as u16,
            Instruction::Sb(i) => *(pmap::pa2va(host_pa) as *mut u8) = trap::get_register(state, i.rs2()) as u8,
            instr => {
                println!("VQUEUE: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, host_pa, pc);
                loop {}
            }
        }
    }

    csrw!(sepc, pc + len);
    true
}
