#![no_std]
#![feature(asm)]
#![feature(const_fn)]
#![feature(const_slice_len)]
#![feature(const_str_len)]
#![feature(global_asm)]
#![feature(lang_items)]
#![feature(linkage)]
#![feature(naked_functions)]
#![feature(proc_macro_hygiene)]
#![feature(ptr_offset_from)]
#![feature(start)]
#![feature(try_blocks)]

use rvirt::*;

// mandatory rust environment setup
#[lang = "eh_personality"] extern fn eh_personality() {}
#[panic_handler] fn panic(info: &::core::panic::PanicInfo) -> ! { println!("{}", info); loop {}}
#[start] fn start(_argc: isize, _argv: *const *const u8) -> isize {0}
#[no_mangle] fn abort() -> ! { println!("Abort!"); loop {}}

static GUEST_DTB: &'static [u8] = include_bytes!("guest.dtb");

#[link_section = ".initrd"]
#[cfg(feature = "embed_guest_kernel")]
static GUEST_KERNEL: [u8; include_bytes!(env!("RVIRT_GUEST_KERNEL")).len()] =
    *include_bytes!(env!("RVIRT_GUEST_KERNEL"));

#[cfg(not(feature = "embed_guest_kernel"))]
static GUEST_KERNEL: [u8; 0] = [];

global_asm!(include_str!("scode.S"));

extern {
    fn hart_entry();
    fn panic_trap_handler();
}

//#[naked]
#[no_mangle]
#[inline(never)]
unsafe fn sstart2(hartid: u64, device_tree_blob: u64, shared_segments_shift: u64) {
    csrci!(sstatus, riscv::bits::STATUS_SIE);
    if !SHARED_STATICS.hart_lottery.swap(false,  Ordering::SeqCst) {
        csrw!(stvec, hart_entry as u64);
        csrw!(sscratch, hartid);
        csrw!(sie, 0x222);
        csrsi!(sstatus, riscv::bits::STATUS_SIE);
        loop {
            riscv::wfi();
        }
    }

    csrw!(stvec, panic_trap_handler as *const () as u64);

    // Read and process host FDT.
    let mut fdt = Fdt::new(pa2va(device_tree_blob));
    assert!(fdt.magic_valid());
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    assert!(fdt.total_size() < 64 * 1024);
    let machine = fdt.parse();

    // Initialize UART
    if let Some(ty) = machine.uart_type {
        SHARED_STATICS.uart_writer.lock().init(machine.uart_address, ty);
    }

    // Do some sanity checks now that the UART is initialized and we have a better chance of
    // successfully printing output.
    assert!(machine.initrd_end <= machine.physical_memory_offset + pmap::HART_SEGMENT_SIZE);
    assert!(machine.initrd_end - machine.initrd_start <= pmap::HEAP_SIZE);
    assert!(machine.harts.iter().any(|h| h.hartid == hartid));
    if !cfg!(feature = "embed_guest_kernel") && machine.initrd_end == 0 {
        println!("WARN: No guest kernel provided. Make sure to pass one with `-initrd or compile with --features embed_guest_kernel`");
    }

    // Do not allow the __SHARED_STATICS_IMPL symbol to be optimized out.
    assert_eq!(&__SHARED_STATICS_IMPL as *const _ as u64, constants::SUPERVISOR_SHARED_STATIC_ADDRESS);

    // Program PLIC priorities
    for i in 1..127 {
        *(pa2va(machine.plic_address + i*4) as *mut u32) = 1;
    }

    let mut guest_harts = machine.harts.clone();
    let single_hart = guest_harts.len() == 1;
    if !single_hart {
        guest_harts.retain(|h| h.hartid != hartid);
    }
    let single_guest = guest_harts.len() == 1;
    assert!(guest_harts.len() != 0);

    assert!(1 + guest_harts.len() as u64 <= (machine.physical_memory_size >> 30));

    let mut guestid = 1;
    for hart in guest_harts {
        let hart_base_pa = machine.physical_memory_offset + pmap::HART_SEGMENT_SIZE * guestid;

        let mut irq_mask = 0;
        for j in 0..4 {
            let index = ((guestid-1) * 4 + j) as usize;
            if index < machine.virtio.len() {
                let irq = machine.virtio[index].irq;
                assert!(irq < 32);
                irq_mask |= 1u32 << irq;
            }
        }

        *(pa2va(machine.plic_address + 0x200000 + 0x1000 * hart.plic_context) as *mut u32) = 0;
        *(pa2va(machine.plic_address + 0x2000 + 0x80 * hart.plic_context) as *mut u32) = irq_mask;
        *(pa2va(machine.plic_address + 0x2000 + 0x80 * hart.plic_context + 4) as *mut u32) = 0;

        (*(pa2va(hart_base_pa) as *mut [u64; 1024])) = pmap::make_boot_page_table(hart_base_pa);
        for i in 512..1024 {
            *(pa2va(hart_base_pa + i * 8) as *mut u64) += shared_segments_shift >> 2;
        }

        core::ptr::copy(pa2va(device_tree_blob) as *const u8,
                        pa2va(hart_base_pa + 4096*2) as *mut u8,
                        fdt.total_size() as usize);
        if machine.initrd_start == machine.initrd_end {
            core::ptr::copy(&GUEST_KERNEL as *const _ as *const u8,
                            pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *mut u8,
                            GUEST_KERNEL.len());
        } else {
            core::ptr::copy(pa2va(machine.initrd_start) as *const u8,
                            pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *mut u8,
                            (machine.initrd_end - machine.initrd_start) as usize);
        }

        let reason = IpiReason::TriggerHartEntry {
            a0: hart.hartid,
            a1: hart_base_pa + 4096*2,
            a2: shared_segments_shift,
            a3: hart_base_pa,
            a4: if !single_guest { guestid as u64 } else { u64::max_value() },
            sp: hart_base_pa + (4<<20) + pmap::DIRECT_MAP_OFFSET,
            satp: 8 << 60 | (hart_base_pa >> 12),
        };

        *SHARED_STATICS.ipi_reason_array[hart.hartid as usize].lock() = Some(reason);
        if single_hart {
            hart_entry2(hartid);
        } else {
            riscv::sbi::send_ipi_to_hart(hart.hartid);
        }

        guestid += 1;
    }

    loop {}
}

#[no_mangle]
unsafe fn hart_entry2(hartid: u64) {
    let reason = { SHARED_STATICS.ipi_reason_array.get_unchecked(hartid as usize).lock().take() };
    if let Some(IpiReason::TriggerHartEntry { a0, a1, a2, a3, a4, sp, satp }) = reason {
        csrw!(satp, satp);
        hart_entry3(a0, a1, a2, a3, a4, sp);
    } else {
        unreachable!();
    }
}

#[naked]
#[no_mangle]
#[inline(never)]
unsafe fn hart_entry3(hartid: u64, device_tree_blob: u64, shared_segments_shift: u64,
                      hart_base_pa: u64, guestid: u64, stack_pointer: u64) {
    asm!("mv sp, a5
          j hart_entry4" :::: "volatile");
}

#[no_mangle]
unsafe fn hart_entry4(hartid: u64, device_tree_blob: u64, shared_segments_shift: u64,
                      hart_base_pa: u64, guestid: u64) {
    csrw!(stvec, trap::strap_entry as *const () as u64);
    csrw!(sie, 0x222);
    csrs!(sstatus, riscv::bits::STATUS_SUM);
    csrc!(sstatus, riscv::bits::STATUS_SPP);
    riscv::sbi::clear_ipi();

    let guestid = if guestid == u64::max_value() {
        None
    } else {
        Some(guestid)
    };

    // Read and process host FDT.
    let mut fdt = Fdt::new(pa2va(device_tree_blob));
    assert!(fdt.magic_valid());
    assert!(fdt.version() >= 17 && fdt.last_comp_version() <= 17);
    let machine = fdt.parse();

    // Initialize memory subsystem.
    let (shadow_page_tables, guest_memory, guest_shift) =
        pmap::init(hart_base_pa, shared_segments_shift, &machine);

    // Load guest binary
    let (entry, max_addr) = sum::access_user_memory(||{
        elf::load_elf(pa2va(hart_base_pa + pmap::HEAP_OFFSET) as *const u8,
                      machine.physical_memory_offset as *mut u8)
    });
    let guest_dtb = (max_addr | 0x1fffff) + 1;
    csrw!(sepc, entry);

    // Load guest FDT.
    let guest_machine = sum::access_user_memory(||{
        core::ptr::copy(GUEST_DTB.as_ptr(),
                        guest_dtb as *mut u8,
                        GUEST_DTB.len());
        let mut guest_fdt = Fdt::new(guest_dtb);
        guest_fdt.initialize_guest(guest_memory.len(), &machine.bootargs);
        guest_fdt.parse()
    });

    // Initialize context
    context::initialize(&machine, &guest_machine, shadow_page_tables, guest_memory, guest_shift, hartid, guestid);

    // Jump into the guest kernel.
    asm!("mv a1, $0 // dtb = guest_dtb

          li ra, 0
          li sp, 0
          li gp, 0
          li tp, 0
          li t0, 0
          li t1, 0
          li t2, 0
          li s0, 0
          li s1, 0
          li a0, 0  // hartid = 0
          li a2, 0
          li a3, 0
          li a4, 0
          li a5, 0
          li a6, 0
          li a7, 0
          li s2, 0
          li s3, 0
          li s4, 0
          li s5, 0
          li s6, 0
          li s7, 0
          li s8, 0
          li s9, 0
          li s10, 0
          li s11, 0
          li t3, 0
          li t4, 0
          li t5, 0
          li t6, 0
          sret" :: "r"(guest_dtb) : "memory" : "volatile");

    unreachable!();
}

#[no_mangle]
fn panic_trap_handler2() {
    println!("scause={}", csrr!(scause) as isize);
    println!("sepc={:x}", csrr!(sepc));
    panic!("Got unexpected trap, panicking...");
}
