// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use raft_core::clock::{Clock, Instant};
use raft_core::timer_service::{ExpiredTimers, TimerService};

pub struct FrozenTimer;

impl TimerService for FrozenTimer {
    fn reset_election_timer(&mut self) {}

    fn reset_heartbeat_timer(&mut self) {}

    fn stop_timers(&mut self) {}

    fn check_expired(&self) -> ExpiredTimers {
        ExpiredTimers::new()
    }
}

#[derive(Clone, Copy)]
pub struct FrozenClock;

impl Clock for FrozenClock {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        Instant::from_millis(0)
    }
}
