use std::time::Duration;

use tokio::time::{sleep, Instant};

#[derive(Debug, Clone)]
pub struct Timebox {
    start_time: Instant,
    duration: Duration,
}

impl Timebox {
    pub fn new(duration: Duration) -> Self {
        Self {
            start_time: Instant::now(),
            duration,
        }
    }

    pub async fn complete(&self) {
        let elapsed_time = self.start_time.elapsed();

        if elapsed_time < self.duration {
            sleep(self.duration - elapsed_time).await;
        }
    }
}
