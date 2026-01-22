// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::clock::{Clock, Instant};
use raft_core::timer_service::{ExpiredTimers, TimerService};

/// Dummy timer for deterministic testing (no real timers)
pub struct FrozenTimer;

impl TimerService for FrozenTimer {
    fn reset_election_timer(&mut self) {
        // No-op in tests - timers fired manually
    }

    fn reset_heartbeat_timer(&mut self) {
        // No-op in tests
    }

    fn stop_timers(&mut self) {
        // No-op in tests
    }

    fn check_expired(&self) -> ExpiredTimers {
        // Never expires automatically in tests
        ExpiredTimers::new()
    }
}

/// Dummy clock for deterministic testing (time never advances automatically)
#[derive(Clone, Copy)]
pub struct FrozenClock;

impl Clock for FrozenClock {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        // Always return time zero - leases never expire in timeless tests
        Instant::from_millis(0)
    }
}
