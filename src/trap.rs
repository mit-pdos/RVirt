use spin::Mutex;
use riscv_decode::Instruction;

#[allow(unused)]
mod constants {
    pub const TVEC_MODE: usize = 0x3;
    pub const TVEC_BASE: usize = !TVEC_MODE;

    pub const STATUS_UIE: usize = 1 << 0;
    pub const STATUS_SIE: usize = 1 << 1;
    pub const STATUS_UPIE: usize = 1 << 4;
    pub const STATUS_SPIE: usize = 1 << 5;
    pub const STATUS_SPP: usize = 1 << 8;
    pub const STATUS_FS0: usize = 1 << 13;
    pub const STATUS_FS1: usize = 1 << 14;
    pub const STATUS_XS0: usize = 1 << 15;
    pub const STATUS_XS1: usize = 1 << 16;
    pub const STATUS_SUM: usize = 1 << 18;
    pub const STATUS_MXR: usize = 1 << 19;
    pub const STATUS_SD: usize = 1 << 31; // Only for RV32!

    // pub const IP_SSIP: usize = 1 << 1;
    // pub const IP_STIP: usize = 1 << 5;
    // pub const IP_SEIP: usize = 1 << 9;

    // pub const IE_SSIE: usize = 1 << 1;
    // pub const IE_STIE: usize = 1 << 5;
    // pub const IE_SEIE: usize = 1 << 9;

    pub const MSTACK_BASE: usize = 0x80100000;
    pub const SSTACK_BASE: usize = 0x80200000;
}
use self::constants::*;

trait UsizeBits {
    fn get(&self, mask: Self) -> bool;
    fn set(&mut self, mask: Self, value: bool);
}
impl UsizeBits for usize {
    fn get(&self, mask: Self) -> bool {
        *self & mask != 0
    }
    fn set(&mut self, mask: Self, value: bool) {
        if value {
            *self |= mask;
        } else {
            *self &= !mask;
        }
    }
}

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
        (false, 2) => println!("illegal instruction: {:x}", csrr!(mtval)),
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
          li sp, 0x80200000
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

// struct TrapFrame {
//     ra: u32,
//     t0: u32,
//     t1: u32,
//     t2: u32,
//     t3: u32,
//     t4: u32,
//     t5: u32,
//     t6: u32,
//     a0: u32,
//     a1: u32,
//     a2: u32,
//     a3: u32,
//     a4: u32,
//     a5: u32,
//     a6: u32,
//     a7: u32,
// }

#[derive(Default)]
struct ShadowState {
    // sedeleg: usize, -- Hard-wired to zero
    // sideleg: usize, -- Hard-wired to zero

    sstatus: usize,
    sie: usize,
    // sip: usize, -- checked dynamically on read
    stvec: usize,
    scounteren: usize,
    sscratch: usize,
    sepc: usize,
    scause: usize,
    stval: usize,
    satp: usize,

    smode: bool
}
impl ShadowState {
    pub const fn new() -> Self {
        Self {
            sstatus: 0,
            stvec: 0,
            sie: 0,
            scounteren: 0,
            sscratch: 0,
            sepc: 0,
            scause: 0,
            stval: 0,
            satp: 0,

            smode: false,
        }
    }
    pub fn push_sie(&mut self) {
        self.sstatus.set(STATUS_SPIE, self.sstatus.get(STATUS_SIE));
        self.sstatus.set(STATUS_SIE, false);
    }
    pub fn pop_sie(&mut self) {
        self.sstatus.set(STATUS_SPIE, self.sstatus.get(STATUS_SIE));
        self.sstatus.set(STATUS_SIE, false);
        if self.sstatus & STATUS_SPIE != 0 {
            self.sstatus |= STATUS_SIE;
        } else {
            self.sstatus &= !STATUS_SIE;
        }
        self.sstatus |= STATUS_SPIE;
    }
}

static SHADOW_STATE: Mutex<ShadowState> = Mutex::new(ShadowState::new());

#[no_mangle]
pub unsafe fn strap() {
    let cause = csrr!(scause);
    let status = csrr!(sstatus);
    let mut state = SHADOW_STATE.lock();

    if status.get(STATUS_SPP) {
        println!("Trap from within hypervisor?!");
        loop {}
    }

    if (cause as isize) < 0 {
        let enabled = state.sstatus.get(STATUS_SIE);
        let unmasked = state.sie & (1 << (cause & 0xff)) != 0;
        if (!state.smode || enabled) && unmasked {
            forward_interrupt(&mut state, cause, csrr!(sepc));
        }
    } else if cause == 12 || cause == 13 || cause == 15 {
        // TODO: Handle page fault
    } else if cause == 2 && state.smode {
        // Handle illegal instruction
        let pc = csrr!(sepc);
        let il = *(pc as *const u16);
        let len = riscv_decode::instruction_length(il);
        let instruction = match len {
            2 => il as u32,
            4 => il as u32 + (*((pc + 2) as *const u16) as u32) << 16,
            _ => unreachable!(),
        };

        match riscv_decode::try_decode(instruction) {
            Some(Instruction::Sret) => {
                state.pop_sie();
                state.smode = state.sstatus.get(STATUS_SPP);
                state.sstatus.set(STATUS_SPP, false);
                csrw!(sepc, state.sepc);
            }
            Some(Instruction::SfenceVma(rtype)) => {
                // TODO
            }
            _ => forward_exception(&mut state, cause, pc),
        }
    } else if cause == 8 && state.smode {
        asm!("
          lw a0, 8*4($0)
          lw a1, 9*4($0)
          lw a2, 10*4($0)
          lw a3, 11*4($0)
          lw a4, 12*4($0)
          lw a5, 13*4($0)
          lw a6, 14*4($0)
          lw a7, 15*4($0)
          ecall
          sw a7, 15*4($0)"
             :: "r"(SSTACK_BASE) : "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7": "volatile");
        csrw!(sepc, csrr!(sepc) + 4);
    } else {
        forward_exception(&mut state, cause, csrr!(sepc));
    }
}

fn forward_interrupt(state: &mut ShadowState, cause: usize, sepc: usize) {
    state.push_sie();
    state.sepc = sepc;
    state.scause = cause;
    state.sstatus.set(STATUS_SPP, state.smode);
    state.stval = 0;
    state.smode = true;

    match state.stvec & TVEC_MODE {
        0 => csrw!(sepc, state.stvec & TVEC_BASE),
        1 => csrw!(sepc, (state.stvec & TVEC_BASE) + 4 * cause & 0xff),
        _ => unreachable!(),
    }
}

fn forward_exception(state: &mut ShadowState, cause: usize, sepc: usize) {
    state.push_sie();
    state.sepc = sepc;
    state.scause = cause;
    state.sstatus.set(STATUS_SPP, state.smode);
    state.stval = csrr!(stval);
    state.smode = true;
    csrw!(sepc, state.stvec & TVEC_BASE);
}
