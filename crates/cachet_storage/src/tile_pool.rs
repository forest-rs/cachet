// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{AllocationError, AllocationId};

/// Tile allocation record returned by [`TilePool::allocate`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TileSlot {
    allocation: AllocationId,
    index: u32,
}

impl TileSlot {
    /// Creates a tile slot from an allocation id and physical index.
    #[must_use]
    pub const fn new(allocation: AllocationId, index: u32) -> Self {
        Self { allocation, index }
    }

    /// Returns the opaque allocation id.
    #[must_use]
    pub const fn allocation(self) -> AllocationId {
        self.allocation
    }

    /// Returns the physical tile index in the pool.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }
}

/// Summary returned by [`TilePool::stats`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TilePoolStats {
    capacity: u32,
    allocated: u32,
    free: u32,
}

impl TilePoolStats {
    /// Creates pool statistics from raw counters.
    #[must_use]
    pub const fn new(capacity: u32, allocated: u32, free: u32) -> Self {
        Self {
            capacity,
            allocated,
            free,
        }
    }

    /// Returns the total slot capacity.
    #[must_use]
    pub const fn capacity(self) -> u32 {
        self.capacity
    }

    /// Returns the number of allocated slots.
    #[must_use]
    pub const fn allocated(self) -> u32 {
        self.allocated
    }

    /// Returns the number of free slots.
    #[must_use]
    pub const fn free(self) -> u32 {
        self.free
    }
}

/// Fixed-size allocator for equal-sized tile slots.
///
/// # Examples
///
/// This example allocates two slots, frees one validated slot record, and
/// leaves the pool ready to reuse it on a later allocation.
///
/// ```
/// use cachet_storage::TilePool;
///
/// let mut pool = TilePool::new(2);
/// let first = pool.allocate().expect("first slot");
/// let second = pool.allocate().expect("second slot");
///
/// assert_eq!(first.index(), 0);
/// assert_eq!(second.index(), 1);
/// pool.free(first).expect("slot was allocated from this pool");
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TilePool {
    free_list: Vec<u32>,
    allocated: Vec<Option<AllocationId>>,
    next_allocation: u64,
}

impl TilePool {
    /// Creates a fixed-size tile allocator with `capacity` slots.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        let mut free_list = Vec::with_capacity(capacity as usize);
        let mut allocated = Vec::with_capacity(capacity as usize);
        for index in (0..capacity).rev() {
            free_list.push(index);
        }
        allocated.resize(capacity as usize, None);
        Self {
            free_list,
            allocated,
            next_allocation: 0,
        }
    }

    /// Allocates the next free tile slot from the pool.
    pub fn allocate(&mut self) -> Result<TileSlot, AllocationError> {
        let index = self.free_list.pop().ok_or(AllocationError::Exhausted)?;
        let allocation = AllocationId::new(self.next_allocation);
        self.next_allocation = self
            .next_allocation
            .checked_add(1)
            .ok_or(AllocationError::IdExhausted)?;
        self.allocated[index as usize] = Some(allocation);
        Ok(TileSlot::new(allocation, index))
    }

    /// Returns one previously allocated slot to the pool.
    pub fn free(&mut self, slot: TileSlot) -> Result<(), AllocationError> {
        let Some(state) = self.allocated.get_mut(slot.index() as usize) else {
            return Err(AllocationError::InvalidFree);
        };
        if *state != Some(slot.allocation()) {
            return Err(AllocationError::InvalidFree);
        }
        *state = None;
        self.free_list.push(slot.index());
        Ok(())
    }

    /// Returns pool statistics for diagnostics and tests.
    #[must_use]
    pub fn stats(&self) -> TilePoolStats {
        let free = self.free_list.len() as u32;
        let capacity = self.allocated.len() as u32;
        TilePoolStats::new(capacity, capacity.saturating_sub(free), free)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn freed_slots_can_be_reused() {
        let mut pool = TilePool::new(1);
        let slot = pool.allocate().expect("first slot is available");
        assert_eq!(slot.index(), 0);
        pool.free(slot).expect("allocated slot can be freed");
        let recycled = pool.allocate().expect("freed slot is reusable");
        assert_eq!(recycled.index(), 0);
    }

    #[test]
    fn double_free_is_rejected() {
        let mut pool = TilePool::new(1);
        let slot = pool.allocate().expect("first slot is available");
        pool.free(slot).expect("allocated slot can be freed");
        let error = pool.free(slot).expect_err("double free must fail");
        assert_eq!(error, AllocationError::InvalidFree);
    }

    #[test]
    fn freeing_a_slot_from_another_pool_is_rejected() {
        let mut left = TilePool::new(1);
        let mut right = TilePool::new(1);
        let foreign_slot = left.allocate().expect("left pool should allocate");

        let error = right
            .free(foreign_slot)
            .expect_err("foreign slots must not be accepted");

        assert_eq!(error, AllocationError::InvalidFree);
    }
}
