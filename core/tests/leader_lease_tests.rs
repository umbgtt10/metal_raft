// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

use core::cell::Cell;
use raft_core::clock::{Clock, Instant};
use raft_core::components::leader_lease::LeaderLease;
use std::rc::Rc;

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
    // Arrange
    let clock = TestClock::new();
    let lease = LeaderLease::new(5000, clock);

    // Assert
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_valid_after_grant() {
    // Arrange
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock);

    // Act
    lease.grant();

    // Assert
    assert!(lease.is_valid());
}

#[test]
fn test_lease_expires_after_duration() {
    // Arrange
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock.clone());

    // Act
    lease.grant();

    // Assert
    assert!(lease.is_valid());

    // Act
    clock.advance(4999);

    // Assert
    assert!(lease.is_valid());

    // Act
    clock.advance(1);

    // Assert
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_can_be_renewed() {
    // Arrange
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock.clone());

    // Act
    lease.grant();
    clock.advance(3000);

    // Assert
    assert!(lease.is_valid());

    // Act
    lease.grant();
    clock.advance(3000);

    // Assert
    assert!(lease.is_valid());

    // Act
    clock.advance(2001);

    // Assert
    assert!(!lease.is_valid());
}

#[test]
fn test_lease_revocation() {
    // Arrange
    let clock = TestClock::new();
    let mut lease = LeaderLease::new(5000, clock);

    // Act
    lease.grant();

    // Assert
    assert!(lease.is_valid());

    // Act
    lease.revoke();

    // Assert
    assert!(!lease.is_valid());
}

#[test]
fn test_time_remaining() {
    // Arrange
    let clock = TestClock::new();

    // Act
    let mut lease = LeaderLease::new(5000, clock.clone());

    // Assert
    assert_eq!(lease.time_remaining_millis(), None);

    // Act
    lease.grant();

    // Assert
    assert_eq!(lease.time_remaining_millis(), Some(5000));

    // Act
    clock.advance(2000);

    // Assert
    assert_eq!(lease.time_remaining_millis(), Some(3000));

    // Act
    clock.advance(3000);

    // Assert
    assert_eq!(lease.time_remaining_millis(), None);
}
