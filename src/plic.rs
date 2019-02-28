
const MAX_HARTS: usize = 8;

pub struct PlicState {
    base: u64,
    source_priority: [u32; 512],
    pending: [u32; 16],
    enable: [[u32; 32]; MAX_HARTS],
    thresholds: [u32; MAX_HARTS],
    claim_complete: [u32; MAX_HARTS],
}

impl PlicState {
    pub const fn new() -> Self {
        Self {
            base: 0x0c000000,
            source_priority: [0; 512],
            pending: [0; 16],
            enable: [[0; 32]; MAX_HARTS],
            thresholds: [0; MAX_HARTS],
            claim_complete: [0; MAX_HARTS],
        }
    }

    pub fn read_u32(&mut self, addr: u64) -> u32 {
        let offset = addr.wrapping_sub(self.base);
        if offset <= 0x800 {
            self.source_priority[offset as usize >> 2]
        } else if offset >= 0x1000 && offset <= 0x1014 {
            self.pending[offset as usize >> 2]
        } else if offset >= 0x1000 && offset < 0x1000 + 0x1000 * MAX_HARTS as u64 {
            let hart = (offset - 0x1000) / 0x1000;
            let index = ((offset - 0x1000) & 0xfff) >> 2;
            if index <= 14 {
                self.enable[hart as usize][index as usize]
            } else {
                0
            }
        } else if offset >= 0x200000 && offset < 0x200000 + 0x1000 * MAX_HARTS as u64 {
            let hart = (offset - 0x200000) / 0x1000;
            let index = ((offset - 0x200000) & 0xfff) >> 2;
            if index == 0 {
                self.thresholds[hart as usize]
            } else if index == 1 {
                self.claim_complete[hart as usize]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn write_u32(&mut self, addr: u64, value: u32) {
        let offset = addr.wrapping_sub(self.base);
        if offset <= 0x800 {
            self.source_priority[offset as usize >> 2] = value;
        } else if offset >= 0x1000 && offset <= 0x1014 {
            self.pending[offset as usize >> 2] = value;
        } else if offset >= 0x1000 && offset < 0x1000 + 0x1000 * MAX_HARTS as u64 {
            let hart = (offset - 0x1000) / 0x1000;
            let index = ((offset - 0x1000) & 0xfff) >> 2;
            if index <= 14 {
                self.enable[hart as usize][index as usize] = value;
            }
        } else if offset >= 0x200000 && offset < 0x200000 + 0x1000 * MAX_HARTS as u64 {
            let hart = (offset - 0x200000) / 0x1000;
            let index = ((offset - 0x200000) & 0xfff) >> 2;
            if index == 0 {
                self.thresholds[hart as usize] = value;
            } else if index == 1 {
                if self.claim_complete[hart as usize] == value {
                    // TODO
                    self.claim_complete[hart as usize] = 0;
                }
            }
        }
    }
}
