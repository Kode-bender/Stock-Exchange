use std::sync::OnceLock;
use std::time::Instant;

use crate::core::types::Timestamp;

#[derive(Debug, Default)]
pub struct LogicalClock {
    current: Timestamp,
}

impl LogicalClock {
    pub fn next(&mut self) -> Timestamp {
        self.current = self.current.wrapping_add(1);
        self.current
    }

    pub fn current(&self) -> Timestamp {
        self.current
    }

    pub fn reset(&mut self) {
        self.current = 0;
    }
}

pub fn steady_timestamp_now() -> Timestamp {
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    EPOCH.get_or_init(Instant::now).elapsed().as_nanos() as Timestamp
}

pub fn now_ns() -> Timestamp {
    steady_timestamp_now()
}
