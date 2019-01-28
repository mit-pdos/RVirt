
use spin::Mutex;

// 0x340 = mscratch
// 0x140 = sscratch

#[naked]
pub unsafe fn mtrap_entry() -> ! {
    asm!(".align 4
          csrw 0x340, sp
          li sp, 0x80100000
          addi sp, sp, -16*4
          sw ra, 0*4(sp)
          sw t0, 1*4(sp)
          sw t1, 2*4(sp)
          sw t2, 3*4(sp)
          sw t3, 4*4(sp)
          sw t4, 5*4(sp)
          sw t5, 6*4(sp)
          sw t6, 7*4(sp)
          sw a0, 8*4(sp)
          sw a1, 9*4(sp)
          sw a2, 10*4(sp)
          sw a3, 11*4(sp)
          sw a4, 12*4(sp)
          sw a5, 13*4(sp)
          sw a6, 14*4(sp)
          sw a7, 15*4(sp)

          jal ra, mtrap

          lw ra, 0*4(sp)
          lw t0, 1*4(sp)
          lw t1, 2*4(sp)
          lw t2, 3*4(sp)
          lw t3, 4*4(sp)
          lw t4, 5*4(sp)
          lw t5, 6*4(sp)
          lw t6, 7*4(sp)
          lw a0, 8*4(sp)
          lw a1, 9*4(sp)
          lw a2, 10*4(sp)
          lw a3, 11*4(sp)
          lw a4, 12*4(sp)
          lw a5, 13*4(sp)
          lw a6, 14*4(sp)
          lw a7, 15*4(sp)
          csrr sp, 0x340
          mret" :::: "volatile");

    unreachable!()
}

#[no_mangle]
pub unsafe fn mtrap() {
    let cause = csrr!(mcause);
    match ((cause as isize) < 0, cause & 0xff) {
        (true, 0...3) => println!("software interrupt"),
        (true, 4...7) => println!("timer interrupt"),
        (true, 8...11) => println!("external interrupt"),
        (true, _) => println!("reserved interrupt"),
        (false, 0) => println!("instruction address misaligned"),
        (false, 1) => {
            println!("instruction access fault @ {:8x}", csrr!(mepc))
        }
        (false, 2) => println!("illegal instruction"),
        (false, 3) => println!("breakpoint"),
        (false, 4) => println!("load address misaligned"),
        (false, 5) => println!("load access fault"),
        (false, 6) => println!("store/AMO address misaligned"),
        (false, 7) => println!("store/AMO access fault"),
        (false, 8...11) => {
            println!("environment call");
            csrw!(mepc, csrr!(mepc) + 4);
            return;
        }
        (false, 12) => println!("instruction page fault"),
        (false, 13) => println!("load page fault"),
        (false, 14) => println!("reserved exception #14"),
        (false, 15) => println!("store/AMO page fault"),
        (false, _) => println!("reserved exception"),
    }

    loop {}
}

#[naked]
pub unsafe fn strap_entry() -> ! {
    asm!(".align 4
          csrw 0x140, sp
          li sp, 0x80100000
          addi sp, sp, -16*4
          sw ra, 0*4(sp)
          sw t0, 1*4(sp)
          sw t1, 2*4(sp)
          sw t2, 3*4(sp)
          sw t3, 4*4(sp)
          sw t4, 5*4(sp)
          sw t5, 6*4(sp)
          sw t6, 7*4(sp)
          sw a0, 8*4(sp)
          sw a1, 9*4(sp)
          sw a2, 10*4(sp)
          sw a3, 11*4(sp)
          sw a4, 12*4(sp)
          sw a5, 13*4(sp)
          sw a6, 14*4(sp)
          sw a7, 15*4(sp)

          jal ra, strap

          lw ra, 0*4(sp)
          lw t0, 1*4(sp)
          lw t1, 2*4(sp)
          lw t2, 3*4(sp)
          lw t3, 4*4(sp)
          lw t4, 5*4(sp)
          lw t5, 6*4(sp)
          lw t6, 7*4(sp)
          lw a0, 8*4(sp)
          lw a1, 9*4(sp)
          lw a2, 10*4(sp)
          lw a3, 11*4(sp)
          lw a4, 12*4(sp)
          lw a5, 13*4(sp)
          lw a6, 14*4(sp)
          lw a7, 15*4(sp)
          csrr sp, 0x140
          sret" :::: "volatile");

    unreachable!()
}

#[derive(Default)]
struct ShadowState {
    sstatus: usize,
    sedeleg: usize, // Hard-wired to zero
    sideleg: usize, // Hard-wired to zero
    sie: usize,
    stvec: usize,
    scounteren: usize,
    sscratch: usize,
    sepc: usize,
    scause: usize,
    stval: usize,
    sip: usize,
    satp: usize,
}
impl ShadowState {
    pub const fn new() -> Self {
        Self {
            sstatus: 0,
            sedeleg: 0,
            sideleg: 0,
            sie: 0,
            stvec: 0,
            scounteren: 0,
            sscratch: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
            sip: 0,
            satp: 0,
        }
    }
}

static SHADOW_STATE: Mutex<ShadowState> = Mutex::new(ShadowState::new());

#[no_mangle]
pub unsafe fn strap() {
    let cause = csrr!(scause);
    match ((cause as isize) < 0, cause & 0xff) {
        (true, 0...3) => println!("software interrupt"),
        (true, 4...7) => println!("timer interrupt"),
        (true, 8...11) => println!("external interrupt"),
        (true, _) => println!("reserved interrupt"),
        (false, 0) => println!("instruction address misaligned"),
        (false, 1) => {
            println!("instruction access fault @ {:8x}", csrr!(mepc))
        }
        (false, 2) => println!("illegal instruction"),
        (false, 3) => println!("breakpoint"),
        (false, 4) => println!("load address misaligned"),
        (false, 5) => println!("load access fault"),
        (false, 6) => println!("store/AMO address misaligned"),
        (false, 7) => println!("store/AMO access fault"),
        (false, 8...11) => {
            println!("environment call");
            csrw!(sepc, csrr!(sepc) + 4);
            return;
        }
        (false, 12) => println!("instruction page fault"),
        (false, 13) => println!("load page fault"),
        (false, 14) => println!("reserved exception #14"),
        (false, 15) => println!("store/AMO page fault"),
        (false, _) => println!("reserved exception"),
    }

    loop {}
}

