
use crate::constants::MAX_GUEST_HARTS;

/// Number of contexts for the PLIC. Value is twice the max number of harts because each hart will
/// have one M-mode context and one S-mode context.
const MAX_CONTEXTS: usize = MAX_GUEST_HARTS * 2;

pub struct PlicState {
    base: u64,
    source_priority: [u32; 512],
    pending: [u32; 16],
    enable: [[u32; 32]; MAX_CONTEXTS],
    thresholds: [u32; MAX_CONTEXTS],
    claim_complete: [u32; MAX_CONTEXTS],
}

impl PlicState {
    pub const fn new() -> Self {
        Self {
            base: 0x0c000000,
            source_priority: [0; 512],
            pending: [0; 16],
            enable: [[0; 32]; MAX_CONTEXTS],
            thresholds: [0; MAX_CONTEXTS],
            claim_complete: [0; MAX_CONTEXTS],
        }
    }

    pub fn read_u32(&mut self, addr: u64) -> u32 {
        let offset = addr.wrapping_sub(self.base);
        if offset <= 0x800 {
            self.source_priority[offset as usize >> 2]
        } else if offset >= 0x1000 && offset <= 0x1014 {
            self.pending[offset as usize >> 2]
        } else if offset >= 0x2000 && offset < 0x2000 + 0x80 * MAX_CONTEXTS as u64 {
            let hart = (offset - 0x2000) / 0x80;
            let index = ((offset - 0x2000) & 0x7f) >> 2;
            if index <= 32 {
                self.enable[hart as usize][index as usize]
            } else {
                0
            }
        } else if offset >= 0x200000 && offset < 0x200000 + 0x1000 * MAX_CONTEXTS as u64 {
            let hart = ((offset - 0x200000) / 0x1000) as usize;
            let index = ((offset - 0x200000) & 0xfff) >> 2;
            if index == 0 {
                self.thresholds[hart]
            } else if index == 1 {
                if self.claim_complete[hart] == 0 {
                    let threshold = self.thresholds[hart];
                    let mut max_priority = threshold;
                    for i in 0..self.pending.len() {
                        if self.pending[i] == 0 {
                            continue;
                        }

                        for j in 0..32 {
                            if self.pending[i] & (1 << j) != 0 {
                                let interrupt = i*32 + j;
                                if self.source_priority[interrupt] > max_priority {
                                    max_priority = self.source_priority[interrupt];
                                    self.claim_complete[hart] = interrupt as u32;
                                }
                            }
                        }
                    }
                }
                self.set_pending(self.claim_complete[hart], false);
                self.claim_complete[hart]
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn write_u32(&mut self, addr: u64, value: u32, clear_seip: &mut bool) {
        let offset = addr.wrapping_sub(self.base);
        if offset <= 0x800 {
            self.source_priority[offset as usize >> 2] = value;
        } else if offset >= 0x1000 && offset <= 0x1014 {
            self.pending[offset as usize >> 2] = value;
        } else if offset >= 0x2000 && offset < 0x2000 + 0x80 * MAX_CONTEXTS as u64 {
            let hart = (offset - 0x2000) / 0x80;
            let index = ((offset - 0x2000) & 0x7f) >> 2;

            if index <= 32 {
                self.enable[hart as usize][index as usize] = value;
            }
        } else if offset >= 0x200000 && offset < 0x200000 + 0x1000 * MAX_CONTEXTS as u64 {
            let hart = (offset - 0x200000) / 0x1000;
            let index = ((offset - 0x200000) & 0xfff) >> 2;
            if index == 0 {
                self.thresholds[hart as usize] = value;
            } else if index == 1 {
                if self.claim_complete[hart as usize] == value {
                    self.set_pending(value, false);
                    self.claim_complete[hart as usize] = 0;
                    *clear_seip = true;
                }
            }
        }
    }

    pub fn set_pending(&mut self, interrupt: u32, value: bool) {
        let index = (interrupt / 32) as usize;
        let mask = 1 << (interrupt % 32);

        if value {
            self.pending[index] |= mask;
        } else {
            self.pending[index] &= !mask;
        }
    }

    pub fn interrupt_pending(&self) -> bool {
        const CONTEXT: usize = 1; // TODO: shouldn't be a constant

        let threshold = self.thresholds[CONTEXT];
        for i in 0..self.pending.len() {
            if self.pending[i] == 0 {
                continue;
            }

            for j in 0..32 {
                if self.pending[i] & (1 << j) != 0 {
                    if self.source_priority[i*32 + j] > threshold {
                        return true;
                    }
                }
            }
        }

        false
    }
}
