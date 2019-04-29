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
