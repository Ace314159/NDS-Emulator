use super::{AccessType, HW, MemoryValue};

impl HW {
    pub fn arm7_read<T: MemoryValue>(&self, addr: u32) -> T {
        todo!()
    }

    pub fn arm7_write<T: MemoryValue>(&mut self, addr: u32, value: T) {
        todo!()
    }

    pub fn arm7_get_access_time<T: MemoryValue>(&mut self, access_type: AccessType, addr: u32) -> usize {
        todo!()
    }
}