use spin::Mutex;
use riscv_decode::Instruction;
use crate::context::{Context, CONTEXT};
use crate::fdt::MachineMeta;
use crate::plic::PlicState;
use crate::{csr, pfault, pmap, print, sum, virtio};

#[allow(unused)]
pub mod constants {
    pub const TVEC_MODE: u64 = 0x3;
    pub const TVEC_BASE: u64 = !TVEC_MODE;

    pub const STATUS_UIE: u64 = 1 << 0;
    pub const STATUS_SIE: u64 = 1 << 1;
    pub const STATUS_UPIE: u64 = 1 << 4;
    pub const STATUS_SPIE: u64 = 1 << 5;
    pub const STATUS_SPP: u64 = 1 << 8;
    pub const STATUS_FS: u64 = 3 << 13;
    pub const STATUS_XS: u64 = 3 << 15;
    pub const STATUS_SUM: u64 = 1 << 18;
    pub const STATUS_MXR: u64 = 1 << 19;
    pub const STATUS_SD: u64 = 1 << 63;

    pub const STATUS_MPP_M: u64 = 3 << 11;
    pub const STATUS_MPP_S: u64 = 1 << 11;
    pub const STATUS_MPP_U: u64 = 0 << 11;

    // Mask of writable bits in sstatus.
    pub const SSTATUS_WRITABLE_MASK: u64 =
        STATUS_MXR |
        STATUS_SUM |
        STATUS_FS |
        STATUS_SPP |
        STATUS_SPIE |
        STATUS_SIE;
    pub const SSTATUS_DYNAMIC_MASK: u64 = STATUS_SD | STATUS_FS;

    pub const IP_SSIP: u64 = 1 << 1;
    pub const IP_STIP: u64 = 1 << 5;
    pub const IP_SEIP: u64 = 1 << 9;

    pub const IE_SSIE: u64 = 1 << 1;
    pub const IE_STIE: u64 = 1 << 5;
    pub const IE_SEIE: u64 = 1 << 9;

    pub const SATP_MODE: u64 = 0xf << 60;
    pub const SATP_ASID: u64 = 0xffff << 44;
    pub const SATP_PPN: u64 = 0xfff_ffffffff;

    pub const SSTACK_BASE: u64 = 0xffffffffc0400000 - 32*8;
}
use self::constants::*;

pub const MAX_STACK_PADDR: u64 = 0x80400000;

pub const CLINT_ADDRESS: u64 = 0x2000000;
pub const CLINT_MTIMECMP0_OFFSET: u64 = 0x4000;
pub const CLINT_MTIME_OFFSET: u64 = 0x0000BFF8;

pub trait U64Bits {
    fn get(&self, mask: Self) -> bool;
    fn set(&mut self, mask: Self, value: bool);
}
impl U64Bits for u64 {
    #[inline(always)]
    fn get(&self, mask: Self) -> bool {
        *self & mask != 0
    }
    #[inline(always)]
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
#[no_mangle]
#[link_section = ".text.strap_entry"]
pub unsafe fn strap_entry() -> ! {
    asm!(".align 4
          csrw 0x140, sp      // Save stack pointer in sscratch
          li sp, $0           // Set stack pointer

          // Save registers
          sd ra, 1*8(sp)
          sd gp, 3*8(sp)
          sd tp, 4*8(sp)
          sd t0, 5*8(sp)
          sd t1, 6*8(sp)
          sd t2, 7*8(sp)
          sd s0, 8*8(sp)
          sd s1, 9*8(sp)
          sd a0, 10*8(sp)
          sd a1, 11*8(sp)
          sd a2, 12*8(sp)
          sd a3, 13*8(sp)
          sd a4, 14*8(sp)
          sd a5, 15*8(sp)
          sd a6, 16*8(sp)
          sd a7, 17*8(sp)
          sd s2, 18*8(sp)
          sd s3, 19*8(sp)
          sd s4, 20*8(sp)
          sd s5, 21*8(sp)
          sd s6, 22*8(sp)
          sd s7, 23*8(sp)
          sd s8, 24*8(sp)
          sd s9, 25*8(sp)
          sd s10, 26*8(sp)
          sd s11, 27*8(sp)
          sd t3, 28*8(sp)
          sd t4, 29*8(sp)
          sd t5, 30*8(sp)
          sd t6, 31*8(sp)

          jal ra, strap       // Call `strap`
          li sp, $0           // Reset stack pointer, just to be safe

          // Restore registers
          ld ra, 1*8(sp)
          ld gp, 3*8(sp)
          ld tp, 4*8(sp)
          ld t0, 5*8(sp)
          ld t1, 6*8(sp)
          ld t2, 7*8(sp)
          ld s0, 8*8(sp)
          ld s1, 9*8(sp)
          ld a0, 10*8(sp)
          ld a1, 11*8(sp)
          ld a2, 12*8(sp)
          ld a3, 13*8(sp)
          ld a4, 14*8(sp)
          ld a5, 15*8(sp)
          ld a6, 16*8(sp)
          ld a7, 17*8(sp)
          ld s2, 18*8(sp)
          ld s3, 19*8(sp)
          ld s4, 20*8(sp)
          ld s5, 21*8(sp)
          ld s6, 22*8(sp)
          ld s7, 23*8(sp)
          ld s8, 24*8(sp)
          ld s9, 25*8(sp)
          ld s10, 26*8(sp)
          ld s11, 27*8(sp)
          ld t3, 28*8(sp)
          ld t4, 29*8(sp)
          ld t5, 30*8(sp)
          ld t6, 31*8(sp)

          // Restore stack pointer and return
          csrr sp, 0x140
          sret" :: "i"(SSTACK_BASE) : "memory" : "volatile");

    unreachable!()
}

#[no_mangle]
pub unsafe fn strap() {
    let cause = csrr!(scause);
    let status = csrr!(sstatus);

    if status.get(STATUS_SPP) {
        println!("Trap from within hypervisor?!");
        println!("sepc = {:#x}", csrr!(sepc));
        println!("stval = {:#x}", csrr!(stval));
        println!("cause = {}", cause);
        loop {}
    }

    let mut state = CONTEXT.lock();
    let mut state = (&mut *state).as_mut().unwrap();

    if (cause as isize) < 0 {
        handle_interrupt(&mut state, cause);
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == 12 || cause == 13 || cause == 15 {
        let pc = csrr!(sepc);
        if pfault::handle_page_fault(&mut state, cause, pc) {
            maybe_forward_interrupt(&mut state, pc);
        } else {
            forward_exception(&mut state, cause, pc);
        }
    } else if cause == 2 && state.smode {
        let pc = csrr!(sepc);
        let (instruction, decoded, len) = decode_instruction_at_address(&mut state, pc);
        let mut advance_pc = true;
        match decoded {
            Some(Instruction::Sret) => {
                if !state.csrs.sstatus.get(STATUS_SIE) && state.csrs.sstatus.get(STATUS_SPIE) {
                    state.no_interrupt = false;
                }
                state.csrs.pop_sie();
                state.smode = state.csrs.sstatus.get(STATUS_SPP);
                state.csrs.sstatus.set(STATUS_SPP, false);
                csrw!(sepc, state.csrs.sepc);
                advance_pc = false;

                if !state.smode {
                    state.no_interrupt = false;
                }
            }
            Some(fence @ Instruction::SfenceVma(_)) => pmap::handle_sfence_vma(&mut state, fence),
            Some(Instruction::Csrrw(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let value = get_register(state, i.rs1());
                state.set_csr(i.csr(), value);
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrs(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = get_register(state, i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrc(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = get_register(state, i.rs1());
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrwi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                state.set_csr(i.csr(), i.zimm() as u64);
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrsi(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev | mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(Instruction::Csrrci(i)) => if let Some(prev) = state.get_csr(i.csr()) {
                let mask = i.zimm() as u64;
                if mask != 0 {
                    state.set_csr(i.csr(), prev & !mask);
                }
                set_register(state, i.rd(), prev);
            }
            Some(decoded) => {
                println!("Unrecognized instruction! {:?} @ pc={:#x}", decoded, pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
            None => {
                println!("Unrecognized instruction {:#x} @ pc={:#x}", instruction, pc);
                forward_exception(&mut state, cause, pc);
                advance_pc = false;
            }
        }

        if advance_pc {
            csrw!(sepc, pc + len);
        }
        maybe_forward_interrupt(&mut state, csrr!(sepc));
    } else if cause == 8 && state.smode {
        match get_register(state, 17) {
            0 => {
                state.csrs.sip.set(IP_STIP, false);
                state.csrs.mtimecmp = get_register(state, 10);
                set_mtimecmp0(state.csrs.mtimecmp);
            }
            1 => print::guest_putchar(get_register(state, 10) as u8),
            5 => asm!("fence.i" :::: "volatile"),
            6 | 7 => pmap::handle_sfence_vma(&mut state,
                                             Instruction::SfenceVma(riscv_decode::types::RType(0)) /* TODO */),
            i => {
                println!("Got ecall from guest function={}!", i);
                loop {}
            }
        }
        csrw!(sepc, csrr!(sepc) + 4);
    } else {
        if cause != 8 { // no need to print anything for guest syscalls...
            println!("Forward exception (cause = {}, smode={})!", cause, state.smode);
        } else {
            // println!("system call: {}({:#x}, {:#x}, {:#x}, {:#x})",
            //          syscall_name(get_register(state, 17)),
            //          get_register(state, 10), get_register(state, 11),
            //          get_register(state, 12), get_register(state, 13)
            // );
            // if syscall_name(get_register(state, 17)) == "write" {
            //     let fd = get_register(state, 10);
            //     let ptr = get_register(state, 11);
            //     let len = get_register(state, 12);
            //     if fd == 1 {
            //         print!("data = ");
            //         for i in 0..len {
            //             print::guest_putchar(*((ptr + i) as *const u8));
            //         }
            //     }
            // }
        }
        forward_exception(&mut state, cause, csrr!(sepc));
    }

    state.shadow_page_tables.install_root(state.shadow());
}

fn handle_interrupt(state: &mut Context, cause: u64) {
    let interrupt = cause & 0xff;
    match interrupt {
        0x1 => {
            // Software
            unimplemented!();
        }
        0x5 => {
            // Timer
            csrc!(sip, 1 << interrupt);
            assert_eq!(csrr!(sip) & (1 << interrupt), 0);

            let time = get_mtime();
            crate::context::Uart::timer(state, time);
            if state.csrs.mtimecmp <= time {
                state.csrs.sip |= IP_STIP;
                state.no_interrupt = false;
            }

            let mut next = 0xffffffff;
            if state.uart.next_interrupt_time > time {
                next = next.min(state.uart.next_interrupt_time);
            }
            if state.csrs.mtimecmp > time {
                next = next.min(state.csrs.mtimecmp);
            }
            if next < 0xffffffff {
                set_mtimecmp0(next);
            }
        }
        0x9 => unsafe {
            // External
            let claim = *(pmap::pa2va(0x0c201004) as *mut u32);
            asm!("" :::: "volatile");
            *(pmap::pa2va(0x0c201004) as *mut u32) = claim;
            state.plic.set_pending(claim, true);

            // Guest might have masked out this interrupt
            if state.plic.interrupt_pending() {
                state.no_interrupt = false;
                state.csrs.sip |= IP_SEIP;
            } else {
                assert_eq!(state.csrs.sip & IP_SEIP, 0);
                println!("Guest masked external interrupt");
            }

        }
        i => {
            println!("Got interrupt #{}", i);
            unreachable!()
        }
    }
}

fn maybe_forward_interrupt(state: &mut Context, sepc: u64) {
    if state.no_interrupt {
        return;
    }

    if !state.csrs.sip.get(IP_SEIP) && state.plic.interrupt_pending() {
        state.csrs.sip.set(IP_SEIP, true);
    }

    if (!state.smode || state.csrs.sstatus.get(STATUS_SIE)) && (state.csrs.sie & state.csrs.sip != 0) {
        let cause = if state.csrs.sip.get(IP_SEIP) {
            9
        } else if state.csrs.sip.get(IP_STIP) {
            5
        } else if state.csrs.sip.get(IP_SSIP) {
            1
        } else {
            unreachable!()
        };

        // println!("||> Forwarding timer interrupt! (state.smode={}, sepc={:#x})", state.smode, sepc);
        // forward interrupt
        state.csrs.push_sie();
        state.csrs.sepc = sepc;
        state.csrs.scause = (1 << 63) | cause;
        state.csrs.sstatus.set(STATUS_SPP, state.smode);
        state.csrs.stval = 0;
        state.smode = true;

        match state.csrs.stvec & TVEC_MODE {
            0 => csrw!(sepc, state.csrs.stvec & TVEC_BASE),
            1 => csrw!(sepc, (state.csrs.stvec & TVEC_BASE) + 4 * cause),
            _ => unreachable!(),
        }
    } else {
        state.no_interrupt = true;
    }
}

fn forward_exception(state: &mut Context, cause: u64, sepc: u64) {
    // println!("||> Forward exception sepc={:#x}", sepc);
    state.csrs.push_sie();
    state.csrs.sepc = sepc;
    state.csrs.scause = cause;
    state.csrs.sstatus.set(STATUS_SPP, state.smode);
    state.csrs.stval = csrr!(stval);
    state.smode = true;
    csrw!(sepc, state.csrs.stvec & TVEC_BASE);
}

pub fn set_register(state: &mut Context, reg: u32, value: u64) {
    match reg {
        0 => {},
        1 | 3..=31 => state.saved_registers[reg as u64 * 8] = value,
        2 => csrw!(sscratch, value),
        _ => unreachable!(),
    }
}
pub fn get_register(state: &mut Context, reg: u32) -> u64 {
    match reg {
        0 => 0,
        1 | 3..=31 => state.saved_registers[reg as u64 * 8],
        2 => csrr!(sscratch),
        _ => unreachable!(),
    }
}

pub fn get_mtime() -> u64 {
    unsafe { *(pmap::pa2va(CLINT_ADDRESS + CLINT_MTIME_OFFSET) as *const u64) }
}
pub fn set_mtimecmp0(value: u64) {
    unsafe { *(pmap::pa2va(CLINT_ADDRESS + CLINT_MTIMECMP0_OFFSET) as *mut u64) = value; }
}

pub unsafe fn decode_instruction_at_address(state: &mut Context, guest_va: u64) -> (u32, Option<Instruction>, u64) {
    let pc_ptr = guest_va as *const u16;
    let (len, instruction) = sum::access_user_memory(||{
        let il: u16 = *pc_ptr;
        match riscv_decode::instruction_length(il) {
            2 => (2, il as u32),
            4 => (4, il as u32 | ((*pc_ptr.offset(1) as u32) << 16)),
            _ => unreachable!(),
        }
    });
    (instruction, riscv_decode::decode(instruction).ok(), len as u64)
}

fn syscall_name(number: u64) -> &'static str {
    match number {
        0 => "io_setup",
        1 => "io_destroy",
        2 => "io_submit",
        3 => "io_cancel",
        4 => "io_getevents",
        5 => "setxattr",
        6 => "lsetxattr",
        7 => "fsetxattr",
        8 => "getxattr",
        9 => "lgetxattr",
        10 => "fgetxattr",
        11 => "listxattr",
        12 => "llistxattr",
        13 => "flistxattr",
        14 => "removexattr",
        15 => "lremovexattr",
        16 => "fremovexattr",
        17 => "getcwd",
        18 => "lookup_dcookie",
        19 => "eventfd2",
        20 => "epoll_create1",
        21 => "epoll_ctl",
        22 => "epoll_pwait",
        23 => "dup",
        24 => "dup3",
        25 => "fcntl",
        26 => "inotify_init1",
        27 => "inotify_add_watch",
        28 => "inotify_rm_watch",
        29 => "ioctl",
        30 => "ioprio_set",
        31 => "ioprio_get",
        32 => "flock",
        33 => "mknodat",
        34 => "mkdirat",
        35 => "unlinkat",
        36 => "symlinkat",
        37 => "linkat",
        38 => "renameat",
        39 => "umount2",
        40 => "mount",
        41 => "pivot_root",
        42 => "nfsservctl",
        43 => "statfs",
        44 => "fstatfs",
        45 => "truncate",
        46 => "ftruncate",
        47 => "fallocate",
        48 => "faccessat",
        49 => "chdir",
        50 => "fchdir",
        51 => "chroot",
        52 => "fchmod",
        53 => "fchmodat",
        54 => "fchownat",
        55 => "fchown",
        56 => "openat",
        57 => "close",
        58 => "vhangup",
        59 => "pipe2",
        60 => "quotactl",
        61 => "getdents64",
        62 => "lseek",
        63 => "read",
        64 => "write",
        65 => "readv",
        66 => "writev",
        67 => "pread64",
        68 => "pwrite64",
        69 => "preadv",
        70 => "pwritev",
        71 => "sendfile",
        72 => "pselect6",
        73 => "ppoll",
        74 => "signalfd4",
        75 => "vmsplice",
        76 => "splice",
        77 => "tee",
        78 => "readlinkat",
        79 => "fstatat",
        80 => "fstat",
        81 => "sync",
        82 => "fsync",
        83 => "fdatasync",
        84 => "sync_file_range",
        85 => "timerfd_create",
        86 => "timerfd_settime",
        87 => "timerfd_gettime",
        88 => "utimensat",
        89 => "acct",
        90 => "capget",
        91 => "capset",
        92 => "personality",
        93 => "exit",
        94 => "exit_group",
        95 => "waitid",
        96 => "set_tid_address",
        97 => "unshare",
        98 => "futex",
        99 => "set_robust_list",
        100 => "get_robust_list",
        101 => "nanosleep",
        102 => "getitimer",
        103 => "setitimer",
        104 => "kexec_load",
        105 => "init_module",
        106 => "delete_module",
        107 => "timer_create",
        108 => "timer_gettime",
        109 => "timer_getoverrun",
        110 => "timer_settime",
        111 => "timer_delete",
        112 => "clock_settime",
        113 => "clock_gettime",
        114 => "clock_getres",
        115 => "clock_nanosleep",
        116 => "syslog",
        117 => "ptrace",
        118 => "sched_setparam",
        119 => "sched_setscheduler",
        120 => "sched_getscheduler",
        121 => "sched_getparam",
        122 => "sched_setaffinity",
        123 => "sched_getaffinity",
        124 => "sched_yield",
        125 => "sched_get_priority_max",
        126 => "sched_get_priority_min",
        127 => "sched_rr_get_interval",
        128 => "restart_syscall",
        129 => "kill",
        130 => "tkill",
        131 => "tgkill",
        132 => "sigaltstack",
        133 => "rt_sigsuspend",
        134 => "rt_sigaction",
        135 => "rt_sigprocmask",
        136 => "rt_sigpending",
        137 => "rt_sigtimedwait",
        138 => "rt_sigqueueinfo",
        139 => "rt_sigreturn",
        140 => "setpriority",
        141 => "getpriority",
        142 => "reboot",
        143 => "setregid",
        144 => "setgid",
        145 => "setreuid",
        146 => "setuid",
        147 => "setresuid",
        148 => "getresuid",
        149 => "setresgid",
        150 => "getresgid",
        151 => "setfsuid",
        152 => "setfsgid",
        153 => "times",
        154 => "setpgid",
        155 => "getpgid",
        156 => "getsid",
        157 => "setsid",
        158 => "getgroups",
        159 => "setgroups",
        160 => "uname",
        161 => "sethostname",
        162 => "setdomainname",
        163 => "getrlimit",
        164 => "setrlimit",
        165 => "getrusage",
        166 => "umask",
        167 => "prctl",
        168 => "getcpu",
        169 => "gettimeofday",
        170 => "settimeofday",
        171 => "adjtimex",
        172 => "getpid",
        173 => "getppid",
        174 => "getuid",
        175 => "geteuid",
        176 => "getgid",
        177 => "getegid",
        178 => "gettid",
        179 => "sysinfo",
        180 => "mq_open",
        181 => "mq_unlink",
        182 => "mq_timedsend",
        183 => "mq_timedreceive",
        184 => "mq_notify",
        185 => "mq_getsetattr",
        186 => "msgget",
        187 => "msgctl",
        188 => "msgrcv",
        189 => "msgsnd",
        190 => "semget",
        191 => "semctl",
        192 => "semtimedop",
        193 => "semop",
        194 => "shmget",
        195 => "shmctl",
        196 => "shmat",
        197 => "shmdt",
        198 => "socket",
        199 => "socketpair",
        200 => "bind",
        201 => "listen",
        202 => "accept",
        203 => "connect",
        204 => "getsockname",
        205 => "getpeername",
        206 => "sendto",
        207 => "recvfrom",
        208 => "setsockopt",
        209 => "getsockopt",
        210 => "shutdown",
        211 => "sendmsg",
        212 => "recvmsg",
        213 => "readahead",
        214 => "brk",
        215 => "munmap",
        216 => "mremap",
        217 => "add_key",
        218 => "request_key",
        219 => "keyctl",
        220 => "clone",
        221 => "execve",
        222 => "mmap",
        223 => "fadvise64",
        224 => "swapon",
        225 => "swapoff",
        226 => "mprotect",
        227 => "msync",
        228 => "mlock",
        229 => "munlock",
        230 => "mlockall",
        231 => "munlockall",
        232 => "mincore",
        233 => "madvise",
        234 => "remap_file_pages",
        235 => "mbind",
        236 => "get_mempolicy",
        237 => "set_mempolicy",
        238 => "migrate_pages",
        239 => "move_pages",
        240 => "rt_tgsigqueueinfo",
        241 => "perf_event_open",
        242 => "accept4",
        243 => "recvmmsg",
        260 => "wait4",
        261 => "prlimit64",
        262 => "fanotify_init",
        263 => "fanotify_mark",
        264 => "name_to_handle_at",
        265 => "open_by_handle_at",
        266 => "clock_adjtime",
        267 => "syncfs",
        268 => "setns",
        269 => "sendmmsg",
        270 => "process_vm_readv",
        271 => "process_vm_writev",
        272 => "kcmp",
        273 => "finit_module",
        274 => "sched_setattr",
        275 => "sched_getattr",
        276 => "renameat2",
        277 => "seccomp",
        278 => "getrandom",
        279 => "memfd_create",
        280 => "bpf",
        281 => "execveat",
        282 => "userfaultfd",
        283 => "membarrier",
        284 => "mlock2",
        285 => "copy_file_range",
        286 => "preadv2",
        287 => "pwritev2",
        288 => "pkey_mprotect",
        289 => "pkey_alloc",
        290 => "pkey_free",
        291 => "statx",
        292 => "io_pgetevents",
        293 => "rseq",
        294 => "kexec_file_load",
        403 => "clock_gettime64",
        404 => "clock_settime64",
        405 => "clock_adjtime64",
        406 => "clock_getres_time64",
        407 => "clock_nanosleep_time64",
        408 => "timer_gettime64",
        409 => "timer_settime64",
        410 => "timerfd_gettime64",
        411 => "timerfd_settime64",
        412 => "utimensat_time64",
        413 => "pselect6_time64",
        414 => "ppoll_time64",
        416 => "io_pgetevents_time64",
        417 => "recvmmsg_time64",
        418 => "mq_timedsend_time64",
        419 => "mq_timedreceive_time64",
        420 => "semtimedop_time64",
        421 => "rt_sigtimedwait_time64",
        422 => "futex_time64",
        423 => "sched_rr_get_interval_time64",
        424 => "pidfd_send_signal",
        425 => "io_uring_setup",
        426 => "io_uring_enter",
        427 => "io_uring_register",
        _ => "UNKNOWN",
    }
}
