use super::driver::SessionDriver;

pub struct SessionLayer<S, D: SessionDriver> {
    inner: S,
    driver: D
}
