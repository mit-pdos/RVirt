
use crate::constants::{mstatic, MAX_HOST_HARTS, SYMBOL_PA2VA_OFFSET};
use crate::machdebug;
use spin::Mutex;

#[derive(Copy, Clone, Debug)]
pub enum Reason {
    EnterSupervisor {
        a0: u64,
        a1: u64,
        a2: u64,
        a3: u64,
        sp: u64,
        satp: u64,
        mepc: u64,
    }
}

const MR: Mutex<Option<Reason>> = Mutex::new(None);
type ReasonArray = [Mutex<Option<Reason>>; MAX_HOST_HARTS];
pub static REASON_ARRAY: ReasonArray = [
    MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR, MR,
];

#[link_section = ".text.init"]
#[no_mangle]
pub unsafe fn handle_ipi() {
    let hartid = csrr!(mhartid);
    let reason = { mstatic(&REASON_ARRAY).get_unchecked(hartid as usize).lock().take() };

    match reason {
        Some(Reason::EnterSupervisor{ a0, a1, a2, a3, sp, satp, mepc}) => {
            csrw!(mepc, mepc);
            csrw!(satp, satp);
            asm!("mv a0, $0
                  mv a1, $1
                  mv a2, $2
                  mv a3, $3
                  mv sp, $4
                  mret" :: "r"(a0), "r"(a1), "r"(a2), "r"(a3), "r"(sp) : "a0", "a1", "a2", "a3", "sp" : "volatile");
        }
        None => {
            machdebug::machine_debug_abort("Got IPI but reason wasn't specified?");
        }
    }
}
