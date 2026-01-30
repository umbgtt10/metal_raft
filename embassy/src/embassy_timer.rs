// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use embassy_time::{Duration, Instant};
use raft_core::clock::Clock;
use raft_core::timer_service::{ExpiredTimers, TimerKind, TimerService};

const ELECTION_TIMEOUT_MIN_MS: u64 = 300;
const ELECTION_TIMEOUT_MAX_MS: u64 = 600;
const HEARTBEAT_TIMEOUT_MS: u64 = 100;

#[derive(Clone, Copy)]
pub struct EmbassyClock;

impl Clock for EmbassyClock {
    type Instant = raft_core::clock::Instant;

    fn now(&self) -> Self::Instant {
        let embassy_now = Instant::now();
        raft_core::clock::Instant::from_millis(embassy_now.as_millis())
    }
}

pub struct EmbassyTimer {
    election_deadline: Option<Instant>,
    heartbeat_deadline: Option<Instant>,
    entropy_state: u64,
}

impl EmbassyTimer {
    pub fn new() -> Self {
        Self {
            election_deadline: None,
            heartbeat_deadline: None,
            entropy_state: Instant::now().as_ticks(),
        }
    }

    fn random_election_timeout(&mut self) -> Duration {
        let now_ticks = Instant::now().as_ticks();

        self.entropy_state = self
            .entropy_state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(now_ticks);

        let range = ELECTION_TIMEOUT_MAX_MS - ELECTION_TIMEOUT_MIN_MS;
        let offset = self.entropy_state % (range + 1);
        Duration::from_millis(ELECTION_TIMEOUT_MIN_MS + offset)
    }
}

impl TimerService for EmbassyTimer {
    fn reset_election_timer(&mut self) {
        self.election_deadline = Some(Instant::now() + self.random_election_timeout());
    }

    fn reset_heartbeat_timer(&mut self) {
        self.heartbeat_deadline =
            Some(Instant::now() + Duration::from_millis(HEARTBEAT_TIMEOUT_MS));
    }

    fn stop_timers(&mut self) {
        self.election_deadline = None;
        self.heartbeat_deadline = None;
    }

    fn check_expired(&self) -> ExpiredTimers {
        let mut expired = ExpiredTimers::new();
        let now = Instant::now();

        if let Some(deadline) = self.election_deadline {
            if now >= deadline {
                expired.push(TimerKind::Election);
            }
        }

        if let Some(deadline) = self.heartbeat_deadline {
            if now >= deadline {
                expired.push(TimerKind::Heartbeat);
            }
        }

        expired
    }
}

impl Default for EmbassyTimer {
    fn default() -> Self {
        Self::new()
    }
}
