// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKind {
    Election,
    Heartbeat,
}

#[derive(Debug)]
pub struct ExpiredTimers {
    timers: [Option<TimerKind>; 2],
    count: usize,
}

impl ExpiredTimers {
    pub fn new() -> Self {
        Self {
            timers: [None, None],
            count: 0,
        }
    }

    pub fn push(&mut self, kind: TimerKind) {
        if self.count < 2 {
            self.timers[self.count] = Some(kind);
            self.count += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = TimerKind> + '_ {
        self.timers[..self.count].iter().filter_map(|&t| t)
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl Default for ExpiredTimers {
    fn default() -> Self {
        Self::new()
    }
}

pub trait TimerService {
    fn reset_election_timer(&mut self);
    fn reset_heartbeat_timer(&mut self);
    fn stop_timers(&mut self);
    fn check_expired(&self) -> ExpiredTimers;
}
