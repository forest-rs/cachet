// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{sync::Arc, vec::Vec};
use core::fmt;

use crate::Rect;

/// Cache-local identifier for one stable atlas page.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PageId(u32);

impl PageId {
    pub(crate) const fn new(index: u32) -> Self {
        Self(index)
    }

    /// Returns the zero-based page index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.0
    }
}

/// Cache-local generational identifier for one logical entry.
///
/// A table slot may be reused, but its generation changes so stale identifiers
/// do not validate as later artifacts in the same cache.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct EntryId {
    index: u32,
    generation: u32,
}

impl EntryId {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Returns the entry-table index.
    #[must_use]
    pub const fn index(self) -> u32 {
        self.index
    }

    /// Returns the captured slot generation.
    #[must_use]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Physical placement of one artifact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
    entry: EntryId,
    page: PageId,
    content: Rect,
    allocation: Rect,
}

impl Placement {
    pub(crate) const fn new(entry: EntryId, page: PageId, content: Rect, allocation: Rect) -> Self {
        Self {
            entry,
            page,
            content,
            allocation,
        }
    }

    /// Returns the logical entry identifier.
    #[must_use]
    pub const fn entry(self) -> EntryId {
        self.entry
    }

    /// Returns the physical page.
    #[must_use]
    pub const fn page(self) -> PageId {
        self.page
    }

    /// Returns the rectangle containing artifact pixels.
    #[must_use]
    pub const fn content(self) -> Rect {
        self.content
    }

    /// Returns the complete allocation including zero padding.
    #[must_use]
    pub const fn allocation(self) -> Rect {
        self.allocation
    }
}

/// Borrowed ready artifact returned by lookup.
#[derive(Clone, Copy, Debug)]
pub struct ArtifactRef<'a, M> {
    placement: Placement,
    metadata: &'a M,
}

impl<'a, M> ArtifactRef<'a, M> {
    pub(crate) const fn new(placement: Placement, metadata: &'a M) -> Self {
        Self {
            placement,
            metadata,
        }
    }

    /// Returns the entry identifier.
    #[must_use]
    pub const fn entry(self) -> EntryId {
        self.placement.entry()
    }

    /// Returns the physical atlas placement.
    #[must_use]
    pub const fn placement(self) -> Placement {
        self.placement
    }

    /// Returns caller-defined artifact metadata.
    #[must_use]
    pub const fn metadata(self) -> &'a M {
        self.metadata
    }
}

/// State observed by a reservation request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservationStatus {
    /// The key already has a published artifact.
    Ready,
    /// The key has an existing reservation awaiting publication.
    Pending,
    /// This call created a reservation that the caller owns.
    Vacant,
}

/// Placement reserved for one caller key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Reservation {
    placement: Placement,
    status: ReservationStatus,
}

impl Reservation {
    pub(crate) const fn new(placement: Placement, status: ReservationStatus) -> Self {
        Self { placement, status }
    }

    /// Returns the entry identifier.
    #[must_use]
    pub const fn entry(self) -> EntryId {
        self.placement.entry()
    }

    /// Returns the reserved placement.
    #[must_use]
    pub const fn placement(self) -> Placement {
        self.placement
    }

    /// Returns the observed reservation state.
    #[must_use]
    pub const fn status(self) -> ReservationStatus {
        self.status
    }

    /// Returns whether this call created a reservation requiring production.
    #[must_use]
    pub const fn needs_population(self) -> bool {
        matches!(self.status, ReservationStatus::Vacant)
    }
}

/// Ready artifact evicted under physical capacity pressure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Evicted<K, M> {
    key: K,
    metadata: M,
    placement: Placement,
}

impl<K, M> Evicted<K, M> {
    pub(crate) const fn new(key: K, metadata: M, placement: Placement) -> Self {
        Self {
            key,
            metadata,
            placement,
        }
    }

    /// Returns the evicted key.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns caller metadata retained with the artifact.
    #[must_use]
    pub const fn metadata(&self) -> &M {
        &self.metadata
    }

    /// Returns the now-invalid historical placement.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// Returns all owned parts.
    #[must_use]
    pub fn into_parts(self) -> (K, M, Placement) {
        (self.key, self.metadata, self.placement)
    }
}

/// Artifact removed by explicit key invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Invalidated<K, M> {
    key: K,
    metadata: Option<M>,
    placement: Placement,
    deferred: bool,
}

impl<K, M> Invalidated<K, M> {
    pub(crate) const fn new(
        key: K,
        metadata: Option<M>,
        placement: Placement,
        deferred: bool,
    ) -> Self {
        Self {
            key,
            metadata,
            placement,
            deferred,
        }
    }

    /// Returns the invalidated key.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns metadata when the invalidated entry had been populated.
    #[must_use]
    pub const fn metadata(&self) -> Option<&M> {
        self.metadata.as_ref()
    }

    /// Returns the now-invalid historical placement.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// Returns whether physical reuse waits for a live lease.
    #[must_use]
    pub const fn reuse_deferred(&self) -> bool {
        self.deferred
    }

    /// Returns all owned parts.
    #[must_use]
    pub fn into_parts(self) -> (K, Option<M>, Placement) {
        (self.key, self.metadata, self.placement)
    }
}

/// Ownership token preventing referenced artifact pixels from being reused.
///
/// Retain a lease for as long as prepared renderer work may sample its
/// placements. Dropping it releases the pins; a later reservation or explicit
/// [`AtlasCache::reclaim`](crate::AtlasCache::reclaim) makes retired space
/// reusable.
pub struct Lease {
    first: Option<LeasePin>,
    remaining: Vec<LeasePin>,
}

struct LeasePin {
    entry: EntryId,
    _pin: Arc<()>,
}

/// Newly published placement and its initial reuse-protection lease.
///
/// Publication makes an entry visible to lookup but does not imply that a GPU
/// write has completed. Retain this value, or its contained [`Lease`], until
/// caller-owned producer and consumer ordering permits physical reuse.
#[must_use = "retain the publication lease until the placement may be safely reused"]
#[derive(Debug)]
pub struct Publication {
    placement: Placement,
    lease: Lease,
}

impl Publication {
    pub(crate) const fn new(placement: Placement, lease: Lease) -> Self {
        Self { placement, lease }
    }

    /// Returns the published entry identifier.
    #[must_use]
    pub const fn entry(&self) -> EntryId {
        self.placement.entry()
    }

    /// Returns the published placement.
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// Borrows the initial reuse-protection lease.
    #[must_use]
    pub const fn lease(&self) -> &Lease {
        &self.lease
    }

    /// Keeps only the initial reuse-protection lease.
    #[must_use]
    pub fn into_lease(self) -> Lease {
        self.lease
    }

    /// Returns the placement and its initial lease separately.
    #[must_use]
    pub fn into_parts(self) -> (Placement, Lease) {
        (self.placement, self.lease)
    }
}

impl Lease {
    /// Creates an empty reusable lease token.
    ///
    /// Use [`AtlasCache::lease_into`](crate::AtlasCache::lease_into) to fill
    /// this token. After the protected work completes, [`clear`](Self::clear)
    /// releases its pins while retaining batch capacity for the next frame.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            first: None,
            remaining: Vec::new(),
        }
    }

    pub(crate) fn with_capacity(entries: usize) -> Self {
        Self {
            first: None,
            remaining: Vec::with_capacity(entries.saturating_sub(1)),
        }
    }

    pub(crate) fn one(entry: EntryId, pin: Arc<()>) -> Self {
        Self {
            first: Some(LeasePin { entry, _pin: pin }),
            remaining: Vec::new(),
        }
    }

    pub(crate) fn reserve(&mut self, entries: usize) {
        let needed = entries.saturating_sub(1);
        if needed > self.remaining.capacity() {
            self.remaining.reserve_exact(needed);
        }
    }

    pub(crate) fn contains(&self, entry: EntryId) -> bool {
        self.first.as_ref().is_some_and(|pin| pin.entry == entry)
            || self.remaining.iter().any(|pin| pin.entry == entry)
    }

    pub(crate) fn push(&mut self, entry: EntryId, pin: Arc<()>) {
        let pin = LeasePin { entry, _pin: pin };
        if self.first.is_none() {
            self.first = Some(pin);
        } else {
            self.remaining.push(pin);
        }
    }

    pub(crate) fn entry_ids(&self) -> impl Iterator<Item = EntryId> + '_ {
        self.first
            .iter()
            .chain(&self.remaining)
            .map(|pin| pin.entry)
    }

    /// Returns the number of distinct protected entries.
    #[must_use]
    pub fn entries(&self) -> usize {
        usize::from(self.first.is_some()) + self.remaining.len()
    }

    /// Returns whether no entries are protected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.first.is_none()
    }

    /// Releases every pin while retaining storage for a later batch.
    ///
    /// This is equivalent to dropping the lease for reuse purposes. Call it
    /// only after all producer and consumer work protected by this token has
    /// completed.
    pub fn clear(&mut self) {
        self.first = None;
        self.remaining.clear();
    }

    /// Returns the number of distinct entries that fit without allocation.
    #[must_use]
    pub fn capacity(&self) -> usize {
        1 + self.remaining.capacity()
    }
}

impl Default for Lease {
    fn default() -> Self {
        Self::empty()
    }
}

impl fmt::Debug for Lease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Lease")
            .field("entries", &self.entries())
            .finish()
    }
}
