// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::cmp::Ordering;
use core::ops::{Add, Sub};

pub trait Clock {
    type Instant: Copy + Ord + Add<u64, Output = Self::Instant> + Sub<Output = u64>;

    fn now(&self) -> Self::Instant;

    fn has_elapsed(&self, instant: Self::Instant, duration_millis: u64) -> bool {
        let elapsed = self.now() - instant;
        elapsed >= duration_millis
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instant(u64);

impl Instant {
    pub const fn from_millis(millis: u64) -> Self {
        Self(millis)
    }

    pub const fn as_millis(&self) -> u64 {
        self.0
    }
}

impl PartialOrd for Instant {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Instant {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Add<u64> for Instant {
    type Output = Self;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Sub for Instant {
    type Output = u64;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.saturating_sub(rhs.0)
    }
}
