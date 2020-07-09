#[macro_use] extern crate log;
use num_traits as num;
pub use simplelog;

mod arm7;
mod arm9;
mod hw;

pub mod nds;

pub use nds::NDS;
