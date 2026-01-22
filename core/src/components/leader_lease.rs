// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

//! Leader Lease for linearizable reads.
//!
//! A leader can serve read-only queries without log replication if it holds
//! a valid lease, which is granted when receiving heartbeat acknowledgments
//! from a quorum of nodes.
//!
//! Safety invariant: `lease_duration < election_timeout`
//! This ensures that if a new leader is elected, the old leader's lease
//! has expired before the new leader can begin serving reads.

use crate::clock::Clock;

/// Tracks the leader's lease for linearizable reads.
///
/// A lease is valid if:
/// 1. It has been granted (via quorum heartbeat acknowledgment)
/// 2. The current time is before the expiration instant
pub struct LeaderLease<C: Clock> {
    /// The instant when the lease expires.
    /// None if no lease is currently held.
    expiration: Option<C::Instant>,

    /// The duration for which a lease is valid.
    /// Must be less than the election timeout.
    lease_duration_millis: u64,

    /// Reference to the clock for time checks.
    clock: C,
}

impl<C: Clock> LeaderLease<C> {
    /// Create a new LeaderLease tracker.
    ///
    /// # Arguments
    /// * `lease_duration_millis` - How long a lease remains valid
    /// * `clock` - The clock implementation for time tracking
    ///
    /// # Safety Invariant
    /// Caller must ensure `lease_duration_millis < election_timeout_min`
    pub fn new(lease_duration_millis: u64, clock: C) -> Self {
        Self {
            expiration: None,
            lease_duration_millis,
            clock,
        }
    }

    /// Grant a new lease based on quorum acknowledgment.
    ///
    /// Called when the leader receives heartbeat responses from a quorum.
    /// Sets the expiration to `now + lease_duration`.
    pub fn grant(&mut self) {
        let now = self.clock.now();
        self.expiration = Some(now + self.lease_duration_millis);
    }

    /// Check if the lease is currently valid.
    ///
    /// Returns true if:
    /// - A lease has been granted (expiration is Some)
    /// - The current time is before the expiration instant
    pub fn is_valid(&self) -> bool {
        match self.expiration {
            None => false,
            Some(expiration) => self.clock.now() < expiration,
        }
    }

    /// Revoke the current lease.
    ///
    /// Called when:
    /// - Node transitions from Leader to Follower/Candidate
    /// - Node detects a network partition
    /// - Election timeout fires (defensive revocation)
    pub fn revoke(&mut self) {
        self.expiration = None;
    }

    /// Get the time remaining on the lease in milliseconds.
    ///
    /// Returns None if no lease is held or the lease has expired.
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
