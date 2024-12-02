pub mod session;

fn main() {
    let session = session::Session::default();
    let session = session.insert("key", 3);

    println!("{:?}", session);
}
