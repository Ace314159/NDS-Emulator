pub struct CP15 {

}

impl CP15 {
    pub fn new() -> Self {
        CP15 {

        }
    }

    pub fn read(&self, n: u32, m: u32, p: u32) -> u32 {
        println!("Reading from  C{}, C{}, {}", n, m, p);
        0
    }

    pub fn write(&mut self, n: u32, m: u32, p: u32, value: u32) {
        println!("Writing 0b{:b} to C{}, C{}, {}", value, n, m, p);
    }
}
