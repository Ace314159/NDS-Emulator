#[macro_use]
pub extern crate log;
use num_traits as num;
pub use simplelog;

mod arm;
mod hw;

pub mod nds;

pub use nds::NDS;
