// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::sync::atomic::{AtomicBool, Ordering};
use embassy_time::Duration;

#[derive(Clone)]
pub struct CancellationToken {
    cancelled: &'static AtomicBool,
}

impl CancellationToken {
    pub fn new() -> Self {
        static CANCELLED: AtomicBool = AtomicBool::new(false);
        Self {
            cancelled: &CANCELLED,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub async fn wait(&self) {
        while !self.is_cancelled() {
            embassy_time::Timer::after(Duration::from_millis(10)).await;
        }
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}
