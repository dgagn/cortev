use std::collections::HashMap;

pub use session::Session;

pub mod session;

fn main() {
    let session = Session::builder("woow")
        .with_data(HashMap::new())
        .build();
}
