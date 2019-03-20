use riscv_decode::Instruction;
use crate::context::Context;
use crate::{trap, pmap};

pub const MAX_QUEUES: usize = 4;
pub const MAX_DEVICES: usize = 8;

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
}
impl Device {
    pub const fn new() -> Self {
        Self {
            queue_sel: 0,
            queues: [Queue {guest_pa: 0, host_pa: 0, size: 0}; MAX_QUEUES],
        }
    }
}

#[inline(always)]
pub fn is_device_access(guest_pa: u64) -> bool {
    guest_pa >= 0x10001000 && guest_pa < 0x10001000 + 0x1000 * MAX_DEVICES as u64
}

pub unsafe fn handle_device_access(state: &mut Context, guest_pa: u64, pc: u64) -> bool {
    let device = ((guest_pa - 0x10001000) / 0x1000) as usize;
    let offset = guest_pa & 0xfff;

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
            let value = read_u32(guest_pa);
            // println!("VIRTIO: Read value {:#x} at address {:#x}", value, guest_pa);
            trap::set_register(i.rd(), value as u64)
        }
        Some(Instruction::Lb(i)) => {
            assert!(offset >= 0x100);
            let value = read_u32(guest_pa & !0x3);
            let value = (value >> (8*(guest_pa & 0x3))) & 0xff;
            // println!("VIRTIO: Read byte {:#x} at address {:#x}", value, guest_pa);
            trap::set_register(i.rd(), value as u64)
        }
        Some(Instruction::Sw(i)) => {
            let mut value = trap::get_register(i.rs2()) as u32;
            if offset == 0x30 { // QueueSel
                assert!(value < 4);
                state.virtio.devices[device].queue_sel = value;
            } else if offset == 0x38 { // QueueNum
                let queue_sel = state.virtio.devices[device].queue_sel as usize;
                let queue = &mut state.virtio.devices[device].queues[queue_sel];
                queue.size = value as u64;

                // TODO: support changing queue sizes (is this ever done?)
                assert_eq!(queue.guest_pa, 0);
            } else if offset == 0x40 { // QueuePFN
                let queue_sel = state.virtio.devices[device].queue_sel as usize;
                let queue = &mut state.virtio.devices[device].queues[queue_sel];

                // TODO: support releasing queues and remove this assert.
                assert_eq!(queue.guest_pa, 0);

                if value != 0 {
                    queue.guest_pa = (value as u64) << 12;
                    value += (crate::fdt::VM_RESERVATION_SIZE >> 12) as u32;
                    queue.host_pa = (value as u64) << 12;
                } else {
                    unimplemented!();
                }

                // Sad, but necessary because we don't know all the pages this page is mapped.
                pmap::flush_shadow_page_table();

                let index = state.virtio.num_queue_guest_pages;
                assert!(index < state.virtio.queue_guest_pages.len());
                state.virtio.queue_guest_pages[index] = queue.guest_pa;
                state.virtio.num_queue_guest_pages += 1;

                let va = pmap::pa2va(queue.host_pa);
                for i in 0..queue.size {
                    let ptr = (va + i * 16) as *mut u64;
                    let value = *ptr;
                    if value != 0 {
                        *ptr = value.wrapping_add(crate::fdt::VM_RESERVATION_SIZE);
                    }
                    // println!("VQUEUE(create): [{}] addr={:#x} len={:#x} flags={:#x} next={:#x}",
                    //          i, *((va + i * 16) as *mut u64),
                    //          *((va + i * 16 + 8) as *mut u32),
                    //          *((va + i * 16 + 12) as *mut u16),
                    //          *((va + i * 16 + 14) as *mut u16),
                    // );
                }
            // } else if offset == 0x50 {
                // let queue_sel = state.virtio.devices[device].queue_sel as usize;
                // let queue = &mut state.virtio.devices[device].queues[queue_sel];
                // let va = pmap::pa2va(queue.host_pa);
                // println!("VQUEUE: queue={}, host_pa={:#x}, guest_pa={:#x}",
                //          queue_sel, queue.host_pa, queue.guest_pa);
                // for i in 0..queue.size {
                //     println!("VQUEUE(post): [{}] addr={:#x} len={:#x} flags={:#x} next={:#x}",
                //              i, *((va + i * 16) as *mut u64),
                //              *((va + i * 16 + 8) as *mut u32),
                //              *((va + i * 16 + 12) as *mut u16),
                //              *((va + i * 16 + 14) as *mut u16),
                //     );
                // }
            }
            // println!("VIRTIO: Writing {:#x} to address {:#x}", value, guest_pa);
            *(crate::pmap::pa2va(guest_pa) as *mut u32) = value;
        }
        Some(instr) => {
            println!("VIRTIO: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, guest_pa, pc);
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
    for i in 0..state.virtio.num_queue_guest_pages {
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
        const OFFSET:u64 = crate::fdt::VM_RESERVATION_SIZE;
        match decoded.unwrap() {
            Instruction::Ld(i) => {
                trap::set_register(i.rd(), (*(pmap::pa2va(host_pa) as *const u64)).wrapping_sub(OFFSET));
                // println!("VQUEUE: ld {:#x}, ({:#x})", trap::get_register(i.rd()), host_pa);
            }
            Instruction::Sd(i) => {
                let value = trap::get_register(i.rs2());
                if value == 0 {
                    *(pmap::pa2va(host_pa) as *mut u64) = 0;
                } else if value >= 0x80000000 && value < pmap::MAX_GUEST_PHYSICAL_ADDRESS {
                    *(pmap::pa2va(host_pa) as *mut u64) = value.wrapping_add(OFFSET);
                } else {
                    println!("VQUEUE: sd {:#x}, ({:#x}) Failed", value, host_pa);
                    loop {}
                }
                // println!("VQUEUE: sd {:#x}, ({:#x})", value, host_pa);
            }
            instr => {
                println!("VQUEUE: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, host_pa, pc);
                loop {}
            }
        }
    } else {
        // let mut wrote = false;
        match decoded.as_ref().unwrap() {
            Instruction::Ld(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const u64)),
            Instruction::Lwu(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const u32) as u64),
            Instruction::Lhu(i) => {
                // wrote = true;
                trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const u16) as u64);
                // println!("VQUEUE: lhu {:#x}, ({:#x})", trap::get_register(i.rd()) as u32, host_pa);
            }
            Instruction::Lbu(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const u8) as u64),
            Instruction::Lw(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const i32) as i64 as u64),
            Instruction::Lh(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const i16) as i64 as u64),
            Instruction::Lb(i) => trap::set_register(i.rd(), *(pmap::pa2va(host_pa) as *const i8) as i64 as u64),
            Instruction::Sd(i) => *(pmap::pa2va(host_pa) as *mut u64) = trap::get_register(i.rs2()),
            Instruction::Sw(i) => {
                // wrote = true;
                // println!("VQUEUE: sw {:#x}, ({:#x})", trap::get_register(i.rs2()) as u32, host_pa);
                *(pmap::pa2va(host_pa) as *mut u32) = trap::get_register(i.rs2()) as u32
            }
            Instruction::Sh(i) => {
                // wrote = true;
                // println!("VQUEUE: sh {:#x}, ({:#x})", trap::get_register(i.rs2()) as u16, host_pa);
                *(pmap::pa2va(host_pa) as *mut u16) = trap::get_register(i.rs2()) as u16
            }
            Instruction::Sb(i) => *(pmap::pa2va(host_pa) as *mut u8) = trap::get_register(i.rs2()) as u8,
            instr => {
                println!("VQUEUE: Instruction {:?} used to target addr {:#x} from pc {:#x}", instr, host_pa, pc);
                loop {}
            }
        }

        // if !wrote {
        //     println!("VQUEUE: Instruction {:?} used to target addr {:#x} from pc {:#x}",
        //              decoded.as_ref().unwrap(), host_pa, pc);
        // }
    }

    csrw!(sepc, pc + len);
    true
}
