pub mod mmu;

pub use mmu::{AccessType, MemoryValue};

pub struct HW {

}

impl HW {
    pub fn new() -> Self {
        HW {
            
        }
    }

    pub fn interrupts_requested(&self) -> bool {
        false
    }
}
