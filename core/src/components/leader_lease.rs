// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use crate::clock::Clock;

pub struct LeaderLease<C: Clock> {
    expiration: Option<C::Instant>,
    lease_duration_millis: u64,
    clock: C,
}

impl<C: Clock> LeaderLease<C> {
    pub fn new(lease_duration_millis: u64, clock: C) -> Self {
        Self {
            expiration: None,
            lease_duration_millis,
            clock,
        }
    }

    pub fn grant(&mut self) {
        let now = self.clock.now();
        self.expiration = Some(now + self.lease_duration_millis);
    }

    pub fn is_valid(&self) -> bool {
        match self.expiration {
            None => false,
            Some(expiration) => self.clock.now() < expiration,
        }
    }

    pub fn revoke(&mut self) {
        self.expiration = None;
    }

    pub fn time_remaining_millis(&self) -> Option<u64> {
        match self.expiration {
            None => None,
            Some(expiration) => {
                let now = self.clock.now();
                if now < expiration {
                    Some(expiration - now)
                } else {
                    None
                }
            }
        }
    }
}
