use super::driver::SessionDriver;

pub struct SessionLayer<D: SessionDriver> {
    driver: D
}

impl<D: SessionDriver> SessionLayer<D> {
    pub fn new(driver: D) -> Self {
        Self { driver }
    }
}

