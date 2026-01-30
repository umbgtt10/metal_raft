// Copyright 2025 Umberto Gotti <umberto.gotti@umbertogotti.dev>
// Licensed under the Apache License, Version 2.0
// http://www.apache.org/licenses/LICENSE-2.0

pub use crate::collections::error::CollectionError;

pub trait ChunkCollection {
    type Item: Copy;

    fn new(data: &[Self::Item]) -> Self;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn as_slice(&self) -> &[Self::Item];
    fn clear(&mut self);
    fn extend_from_slice(&mut self, data: &[Self::Item]) -> Result<(), CollectionError>;
}
