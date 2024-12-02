use std::collections::HashMap;

use session::driver::{memory::MemoryDriver, SessionDriver};
pub use session::Session;

pub mod session;

fn main() {
    let memory = MemoryDriver::default();
}
