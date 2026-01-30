// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

#[cfg(all(feature = "in-memory-storage", feature = "semihosting-storage"))]
compile_error!("Features 'in-memory-storage' and 'semihosting-storage' are mutually exclusive.");

#[cfg(feature = "in-memory-storage")]
pub mod in_memory;

#[cfg(feature = "in-memory-storage")]
pub use in_memory::in_memory_storage::InMemoryStorage;

#[cfg(feature = "semihosting-storage")]
pub mod semihosting;

#[cfg(feature = "semihosting-storage")]
pub use semihosting::semihosting_storage::SemihostingStorage;
