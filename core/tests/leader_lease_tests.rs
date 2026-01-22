// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::cell::Cell;
use raft_core::clock::{Clock, Instant};
use raft_core::components::leader_lease::LeaderLease;
use std::rc::Rc;

/// Simple test clock that can be controlled
#[derive(Clone)]
struct TestClock {
    current: Rc<Cell<Instant>>,
}

impl TestClock {
    fn new() -> Self {
        Self {
            current: Rc::new(Cell::new(Instant::from_millis(0))),
        }
    }

    fn advance(&self, millis: u64) {
        let now = self.current.get();
        self.current.set(now + millis);
    }
}

impl Clock for TestClock {
    type Instant = Instant;

    fn now(&self) -> Self::Instant {
        self.current.get()
    }
}

#[test]
fn test_lease_initially_invalid() {
    let clock = TestClock::new();
    let lease = LeaderLease::new(5000, clock);
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_valid_after_grant() {
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock);

    lease.grant();
    assert!(lease.is_valid());
}

#[test]
fn test_lease_expires_after_duration() {
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock.clone());

    lease.grant();
    assert!(lease.is_valid());

    // Advance time to just before expiration
    clock.advance(4999);
    assert!(lease.is_valid());

    // Advance to exactly expiration time
    clock.advance(1);
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_can_be_renewed() {
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock.clone());

    lease.grant();
    clock.advance(3000);
    assert!(lease.is_valid());

    // Renew the lease
    lease.grant();
    clock.advance(3000);
    // Should still be valid (renewed at t=3000, expires at t=8000, now at t=6000)
    assert!(lease.is_valid());

    clock.advance(2001);
    // Now expired (t=8001)
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_revocation() {
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock);

    lease.grant();
    assert!(lease.is_valid());

    lease.revoke();
    assert!(!lease.is_valid());
}

#[test]
fn test_time_remaining() {
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock.clone());

    assert_eq!(lease.time_remaining_millis(), None);

    lease.grant();
    assert_eq!(lease.time_remaining_millis(), Some(5000));

    clock.advance(2000);
    assert_eq!(lease.time_remaining_millis(), Some(3000));

    clock.advance(3000);
    assert_eq!(lease.time_remaining_millis(), None);
}
