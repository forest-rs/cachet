// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{sync::Arc, vec, vec::Vec};
use core::hash::Hash;

use hashbrown::HashMap;

use crate::{
    AbortError, ArtifactRef, AtlasConfig, CacheMetrics, CacheStats, ConfigError, EntryId, Evicted,
    Extent, Invalidated, Lease, LeaseError, PageId, PageStats, Placement, Publication,
    PublishError, Rect, Reservation, ReservationStatus, ReserveError, allocator::RectAllocator,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EntryPhase {
    Reserved,
    Ready,
    Retired,
}

#[derive(Debug)]
struct Entry<K, M> {
    key: Option<K>,
    metadata: Option<M>,
    placement: Placement,
    phase: EntryPhase,
    last_used: u64,
    pin: Arc<()>,
}

#[derive(Debug)]
struct EntrySlot<K, M> {
    generation: u32,
    entry: Option<Entry<K, M>>,
}

#[derive(Debug)]
struct Page {
    allocator: RectAllocator,
}

/// Bounded keyed cache of backing-independent atlas placements.
///
/// `AtlasCache` owns page geometry, packing, publication, eviction, and reuse
/// safety. It does not allocate pixel storage or track GPU work. A caller that
/// renders directly into GPU pages maps stable [`PageId`] values to its own
/// textures and retains leases according to its submission lifetime.
///
/// One instance must represent one complete compatibility domain, including
/// padding, backing authority, production ordering, sampling, and retention
/// behavior.
#[derive(Debug)]
pub struct AtlasCache<K, M = ()> {
    config: AtlasConfig,
    pages: Vec<Page>,
    index: HashMap<K, EntryId>,
    entries: Vec<EntrySlot<K, M>>,
    access_sequence: u64,
    metrics: CacheMetrics,
}

impl<K, M> AtlasCache<K, M>
where
    K: Clone + Eq + Hash,
{
    /// Creates an empty cache without allocating any pages.
    pub fn new(config: AtlasConfig) -> Result<Self, ConfigError> {
        config.validate()?;
        Ok(Self {
            config,
            pages: Vec::new(),
            index: HashMap::new(),
            entries: Vec::new(),
            access_sequence: 0,
            metrics: CacheMetrics::default(),
        })
    }

    /// Returns the homogeneous placement configuration.
    #[must_use]
    pub const fn config(&self) -> AtlasConfig {
        self.config
    }

    /// Returns the number of lazily allocated stable pages.
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.pages.len() as u32
    }

    /// Returns whether an identifier currently names a published entry.
    #[must_use]
    pub fn is_ready(&self, entry: EntryId) -> bool {
        self.entry(entry)
            .is_some_and(|entry| entry.phase == EntryPhase::Ready)
    }

    /// Looks up and touches a published artifact.
    ///
    /// The returned borrow does not prevent later physical reuse. Copy its
    /// placement into prepared work and acquire a [`Lease`] before allowing
    /// the cache to mutate again.
    pub fn lookup(&mut self, key: &K) -> Option<ArtifactRef<'_, M>> {
        self.metrics.probes = self.metrics.probes.saturating_add(1);
        let Some(id) = self.index.get(key).copied() else {
            self.metrics.misses = self.metrics.misses.saturating_add(1);
            return None;
        };
        let Some(entry) = self.entry(id) else {
            self.metrics.misses = self.metrics.misses.saturating_add(1);
            return None;
        };
        if entry.phase != EntryPhase::Ready {
            self.metrics.misses = self.metrics.misses.saturating_add(1);
            return None;
        }

        self.metrics.hits = self.metrics.hits.saturating_add(1);
        self.touch(id);
        let entry = self.entry(id).expect("a touched entry remains present");
        Some(ArtifactRef::new(
            entry.placement,
            entry.metadata.as_ref().expect("a ready entry has metadata"),
        ))
    }

    /// Finds or allocates placement for a caller key.
    ///
    /// Ready and pending keys retain their existing placement. A vacant result
    /// owns non-evictable placement until [`publish`](Self::publish),
    /// [`abort`](Self::abort), or [`invalidate`](Self::invalidate). Ready,
    /// unleased entries may be evicted to satisfy a new reservation and are
    /// appended to `evicted` in eviction order.
    pub fn reserve(
        &mut self,
        key: K,
        extent: Extent,
        evicted: &mut Vec<Evicted<K, M>>,
    ) -> Result<Reservation, ReserveError> {
        self.metrics.probes = self.metrics.probes.saturating_add(1);
        if let Some(id) = self.index.get(&key).copied() {
            let entry = self.entry(id).expect("indexed entries remain present");
            let existing = entry.placement.content().extent();
            if existing != extent {
                return Err(ReserveError::ConflictingExtent {
                    existing,
                    requested: extent,
                });
            }
            let placement = entry.placement;
            let status = match entry.phase {
                EntryPhase::Reserved => {
                    self.metrics.misses = self.metrics.misses.saturating_add(1);
                    ReservationStatus::Pending
                }
                EntryPhase::Ready => {
                    self.metrics.hits = self.metrics.hits.saturating_add(1);
                    self.touch(id);
                    ReservationStatus::Ready
                }
                EntryPhase::Retired => unreachable!("retired entries are not indexed"),
            };
            return Ok(Reservation::new(placement, status));
        }
        self.metrics.misses = self.metrics.misses.saturating_add(1);

        let allocation_extent = self.allocation_extent(extent)?;
        self.reclaim();
        let id = self.next_entry_id()?;
        let (page, allocation) = loop {
            if let Some(placement) = self.try_allocate(allocation_extent) {
                break placement;
            }
            if let Some(placement) = self.try_create_page(allocation_extent)? {
                break placement;
            }
            let Some(removed) = self.evict_lru() else {
                self.metrics.pressure_failures = self.metrics.pressure_failures.saturating_add(1);
                return Err(ReserveError::NoSpace);
            };
            evicted.push(removed);
        };

        let padding = self.config.padding();
        let content = Rect::new(
            allocation.x() + padding,
            allocation.y() + padding,
            extent.width(),
            extent.height(),
        );
        let placement = Placement::new(id, page, content, allocation);
        self.insert_entry(
            id,
            Entry {
                key: Some(key.clone()),
                metadata: None,
                placement,
                phase: EntryPhase::Reserved,
                last_used: 0,
                pin: Arc::new(()),
            },
        );
        let previous = self.index.insert(key, id);
        debug_assert!(previous.is_none());
        self.metrics.reservations = self.metrics.reservations.saturating_add(1);
        Ok(Reservation::new(placement, ReservationStatus::Vacant))
    }

    /// Publishes caller metadata and returns the placement's initial lease.
    ///
    /// Publication is an ordering promise made by the caller: content has been
    /// produced, or writes have been encoded so all readers will observe them
    /// under the caller's execution model. It does not wait for GPU completion.
    /// Retain the returned [`Publication`] until producer and reader work no
    /// longer require protection from physical reuse.
    pub fn publish(&mut self, id: EntryId, metadata: M) -> Result<Publication, PublishError> {
        let phase = self.entry(id).ok_or(PublishError::InvalidEntry(id))?.phase;
        if phase != EntryPhase::Reserved {
            return Err(PublishError::NotReserved(id));
        }

        let last_used = self.next_access_sequence();
        let entry = self.entry_mut(id).expect("validated entry remains present");
        entry.metadata = Some(metadata);
        entry.phase = EntryPhase::Ready;
        entry.last_used = last_used;
        let placement = entry.placement;
        let pin = Arc::clone(&entry.pin);
        self.metrics.publications = self.metrics.publications.saturating_add(1);
        Ok(Publication::new(placement, Lease::new(vec![pin])))
    }

    /// Aborts a vacant reservation and returns its key.
    pub fn abort(&mut self, id: EntryId) -> Result<K, AbortError> {
        let phase = self.entry(id).ok_or(AbortError::InvalidEntry(id))?.phase;
        if phase != EntryPhase::Reserved {
            return Err(AbortError::NotReserved(id));
        }
        let entry = self
            .take_entry(id)
            .expect("validated entry remains present");
        let key = entry.key.expect("a reservation retains its key");
        let removed = self.index.remove(&key);
        debug_assert_eq!(removed, Some(id));
        self.free_placement(entry.placement);
        Ok(key)
    }

    /// Removes a key immediately and defers physical reuse when it is leased.
    pub fn invalidate(&mut self, key: &K) -> Option<Invalidated<K, M>> {
        let id = self.index.remove(key)?;
        let leased = {
            let entry = self.entry(id).expect("indexed entries remain present");
            Arc::strong_count(&entry.pin) > 1
        };
        self.metrics.invalidations = self.metrics.invalidations.saturating_add(1);

        if leased {
            let entry = self.entry_mut(id).expect("indexed entries remain present");
            let key = entry.key.take().expect("an indexed entry retains its key");
            let metadata = entry.metadata.take();
            let placement = entry.placement;
            entry.phase = EntryPhase::Retired;
            self.metrics.deferred_reuses = self.metrics.deferred_reuses.saturating_add(1);
            return Some(Invalidated::new(key, metadata, placement, true));
        }

        let entry = self.take_entry(id).expect("indexed entries remain present");
        let key = entry.key.expect("an indexed entry retains its key");
        let metadata = entry.metadata;
        let placement = entry.placement;
        self.free_placement(placement);
        Some(Invalidated::new(key, metadata, placement, false))
    }

    /// Pins every distinct ready entry until the returned lease is dropped.
    ///
    /// Validation is atomic with respect to this `&mut self` call: an error
    /// returns without pinning a subset of the requested entries.
    pub fn lease(
        &mut self,
        entries: impl IntoIterator<Item = EntryId>,
    ) -> Result<Lease, LeaseError> {
        let mut ids = Vec::new();
        for id in entries {
            if ids.contains(&id) {
                continue;
            }
            let entry = self.entry(id).ok_or(LeaseError::InvalidEntry(id))?;
            if entry.phase != EntryPhase::Ready {
                return Err(LeaseError::NotReady(id));
            }
            ids.push(id);
        }

        let mut pins = Vec::with_capacity(ids.len());
        for id in ids {
            let last_used = self.next_access_sequence();
            let entry = self.entry_mut(id).expect("validated entry remains present");
            entry.last_used = last_used;
            pins.push(Arc::clone(&entry.pin));
        }
        Ok(Lease::new(pins))
    }

    /// Reclaims retired placements whose final external lease has dropped.
    ///
    /// Reservations call this automatically before applying capacity pressure.
    pub fn reclaim(&mut self) -> u32 {
        let ids: Vec<_> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let entry = slot.entry.as_ref()?;
                (entry.phase == EntryPhase::Retired && Arc::strong_count(&entry.pin) == 1)
                    .then(|| EntryId::new(index as u32, slot.generation))
            })
            .collect();

        for id in &ids {
            let entry = self
                .take_entry(*id)
                .expect("selected entries remain present");
            self.free_placement(entry.placement);
        }
        let reclaimed = ids.len() as u32;
        self.metrics.reclaims = self.metrics.reclaims.saturating_add(u64::from(reclaimed));
        reclaimed
    }

    /// Returns cumulative lifecycle counters.
    #[must_use]
    pub const fn metrics(&self) -> CacheMetrics {
        self.metrics
    }

    /// Returns current aggregate gauges and cumulative metrics.
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats {
            pages: self.page_count(),
            ready_entries: 0,
            reserved_entries: 0,
            retired_entries: 0,
            leased_entries: 0,
            allocated_texels: 0,
            free_texels: 0,
            free_regions: 0,
            metrics: self.metrics,
        };
        for page in 0..self.page_count() {
            let page = self
                .page_stats(PageId::new(page))
                .expect("all counted pages exist");
            stats.ready_entries = stats.ready_entries.saturating_add(page.ready_entries());
            stats.reserved_entries = stats
                .reserved_entries
                .saturating_add(page.reserved_entries());
            stats.retired_entries = stats.retired_entries.saturating_add(page.retired_entries());
            stats.leased_entries = stats.leased_entries.saturating_add(page.leased_entries());
            stats.allocated_texels = stats
                .allocated_texels
                .saturating_add(page.allocated_texels());
            stats.free_texels = stats.free_texels.saturating_add(page.free_texels());
            stats.free_regions = stats.free_regions.saturating_add(page.free_regions());
        }
        stats
    }

    /// Returns current packing and lifecycle gauges for one page.
    #[must_use]
    pub fn page_stats(&self, page: PageId) -> Option<PageStats> {
        let stored = self.pages.get(page.index() as usize)?;
        let mut stats = PageStats {
            page,
            extent: self.config.page_extent(),
            ready_entries: 0,
            reserved_entries: 0,
            retired_entries: 0,
            leased_entries: 0,
            allocated_texels: stored.allocator.allocated_texels(),
            free_texels: stored.allocator.free_texels(),
            free_regions: stored.allocator.free_regions(),
            largest_free_region: stored.allocator.largest_free_region().map(Rect::extent),
        };
        for entry in self.entries.iter().filter_map(|slot| slot.entry.as_ref()) {
            if entry.placement.page() != page {
                continue;
            }
            match entry.phase {
                EntryPhase::Reserved => {
                    stats.reserved_entries = stats.reserved_entries.saturating_add(1);
                }
                EntryPhase::Ready => {
                    stats.ready_entries = stats.ready_entries.saturating_add(1);
                }
                EntryPhase::Retired => {
                    stats.retired_entries = stats.retired_entries.saturating_add(1);
                }
            }
            if Arc::strong_count(&entry.pin) > 1 {
                stats.leased_entries = stats.leased_entries.saturating_add(1);
            }
        }
        Some(stats)
    }

    fn allocation_extent(&mut self, extent: Extent) -> Result<Extent, ReserveError> {
        if extent.is_empty() {
            return Err(ReserveError::EmptyArtifact);
        }
        let padding = u32::from(self.config.padding()) * 2;
        let width = u32::from(extent.width()) + padding;
        let height = u32::from(extent.height()) + padding;
        let page = self.config.page_extent();
        if width > u32::from(page.width()) || height > u32::from(page.height()) {
            self.metrics.too_large_rejections = self.metrics.too_large_rejections.saturating_add(1);
            return Err(ReserveError::ArtifactTooLarge {
                requested: extent,
                page,
            });
        }
        Ok(Extent::new(width as u16, height as u16))
    }

    fn try_allocate(&mut self, extent: Extent) -> Option<(PageId, Rect)> {
        let index = self
            .pages
            .iter()
            .enumerate()
            .filter_map(|(index, page)| page.allocator.best_fit(extent).map(|fit| (fit, index)))
            .min_by_key(|(fit, index)| (*fit, *index))?
            .1;
        let allocation = self.pages[index]
            .allocator
            .allocate(extent)
            .expect("a selected fit remains available");
        Some((PageId::new(index as u32), allocation))
    }

    fn try_create_page(&mut self, extent: Extent) -> Result<Option<(PageId, Rect)>, ReserveError> {
        if self.page_count() >= self.config.max_pages() {
            return Ok(None);
        }
        let index = u32::try_from(self.pages.len()).map_err(|_| ReserveError::PageIdExhausted)?;
        let mut page = Page {
            allocator: RectAllocator::new(self.config.page_extent()),
        };
        let allocation = page
            .allocator
            .allocate(extent)
            .expect("validated artifacts fit on an empty page");
        self.pages.push(page);
        Ok(Some((PageId::new(index), allocation)))
    }

    fn evict_lru(&mut self) -> Option<Evicted<K, M>> {
        let (index, generation) = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                let entry = slot.entry.as_ref()?;
                (entry.phase == EntryPhase::Ready && Arc::strong_count(&entry.pin) == 1)
                    .then_some((entry.last_used, index, slot.generation))
            })
            .min_by_key(|(last_used, index, _)| (*last_used, *index))
            .map(|(_, index, generation)| (index, generation))?;
        let id = EntryId::new(index as u32, generation);
        let entry = self
            .take_entry(id)
            .expect("selected entries remain present");
        let key = entry.key.expect("a ready entry retains its key");
        let metadata = entry.metadata.expect("a ready entry retains metadata");
        let removed = self.index.remove(&key);
        debug_assert_eq!(removed, Some(id));
        self.free_placement(entry.placement);
        self.metrics.evictions = self.metrics.evictions.saturating_add(1);
        Some(Evicted::new(key, metadata, entry.placement))
    }

    fn free_placement(&mut self, placement: Placement) {
        self.pages[placement.page().index() as usize]
            .allocator
            .free(placement.allocation());
    }

    fn next_entry_id(&self) -> Result<EntryId, ReserveError> {
        for (index, slot) in self.entries.iter().enumerate() {
            if slot.entry.is_none()
                && let Some(generation) = slot.generation.checked_add(1)
            {
                return Ok(EntryId::new(index as u32, generation));
            }
        }
        let index =
            u32::try_from(self.entries.len()).map_err(|_| ReserveError::EntryIdExhausted)?;
        Ok(EntryId::new(index, 0))
    }

    fn insert_entry(&mut self, id: EntryId, entry: Entry<K, M>) {
        let index = id.index() as usize;
        if index == self.entries.len() {
            self.entries.push(EntrySlot {
                generation: id.generation(),
                entry: Some(entry),
            });
            return;
        }
        let slot = &mut self.entries[index];
        debug_assert!(slot.entry.is_none());
        slot.generation = id.generation();
        slot.entry = Some(entry);
    }

    fn entry(&self, id: EntryId) -> Option<&Entry<K, M>> {
        let slot = self.entries.get(id.index() as usize)?;
        (slot.generation == id.generation())
            .then_some(slot.entry.as_ref())
            .flatten()
    }

    fn entry_mut(&mut self, id: EntryId) -> Option<&mut Entry<K, M>> {
        let slot = self.entries.get_mut(id.index() as usize)?;
        (slot.generation == id.generation())
            .then_some(slot.entry.as_mut())
            .flatten()
    }

    fn take_entry(&mut self, id: EntryId) -> Option<Entry<K, M>> {
        let slot = self.entries.get_mut(id.index() as usize)?;
        if slot.generation != id.generation() {
            return None;
        }
        slot.entry.take()
    }

    fn touch(&mut self, id: EntryId) {
        let last_used = self.next_access_sequence();
        if let Some(entry) = self.entry_mut(id) {
            entry.last_used = last_used;
        }
    }

    fn next_access_sequence(&mut self) -> u64 {
        if self.access_sequence == u64::MAX {
            for entry in self
                .entries
                .iter_mut()
                .filter_map(|slot| slot.entry.as_mut())
            {
                entry.last_used = 0;
            }
            self.access_sequence = 0;
        }
        self.access_sequence += 1;
        self.access_sequence
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(extent: Extent, max_pages: u32) -> AtlasCache<&'static str, u32> {
        AtlasCache::new(AtlasConfig::new(extent, max_pages)).expect("valid cache")
    }

    fn reserve_vacant(
        cache: &mut AtlasCache<&'static str, u32>,
        key: &'static str,
        extent: Extent,
    ) -> Reservation {
        let reservation = cache
            .reserve(key, extent, &mut Vec::new())
            .expect("reservation succeeds");
        assert_eq!(reservation.status(), ReservationStatus::Vacant);
        reservation
    }

    #[test]
    fn reservations_are_pending_until_publication() {
        let mut cache = cache(Extent::new(8, 8), 1);
        let first = reserve_vacant(&mut cache, "a", Extent::new(2, 3));

        assert!(cache.lookup(&"a").is_none());
        let pending = cache
            .reserve("a", Extent::new(2, 3), &mut Vec::new())
            .expect("existing reservation");
        assert_eq!(pending.status(), ReservationStatus::Pending);
        assert_eq!(pending.placement(), first.placement());
        assert_eq!(
            cache.reserve("a", Extent::new(3, 2), &mut Vec::new()),
            Err(ReserveError::ConflictingExtent {
                existing: Extent::new(2, 3),
                requested: Extent::new(3, 2),
            })
        );

        let publication = cache.publish(first.entry(), 17).expect("publication");
        assert_eq!(publication.placement(), first.placement());
        drop(publication);
        let artifact = cache.lookup(&"a").expect("published lookup");
        assert_eq!(*artifact.metadata(), 17);
        assert_eq!(artifact.placement(), first.placement());
    }

    #[test]
    fn publication_lease_protects_direct_gpu_production() {
        let mut cache = cache(Extent::new(2, 2), 1);
        let reserved = reserve_vacant(&mut cache, "gpu", Extent::new(2, 2));
        let publication = cache.publish(reserved.entry(), 1).expect("publication");

        let invalidated = cache.invalidate(&"gpu").expect("present key");
        assert!(invalidated.reuse_deferred());
        assert_eq!(
            cache.reserve("next", Extent::new(2, 2), &mut Vec::new()),
            Err(ReserveError::NoSpace)
        );

        drop(publication);
        assert_eq!(cache.reclaim(), 1);
        assert!(
            cache
                .reserve("next", Extent::new(2, 2), &mut Vec::new())
                .is_ok()
        );
    }

    #[test]
    fn lru_evicts_only_unleased_ready_entries() {
        let mut cache = cache(Extent::new(4, 2), 1);
        let a = reserve_vacant(&mut cache, "a", Extent::new(2, 2));
        drop(cache.publish(a.entry(), 1).expect("publish a"));
        let b = reserve_vacant(&mut cache, "b", Extent::new(2, 2));
        drop(cache.publish(b.entry(), 2).expect("publish b"));
        let _ = cache.lookup(&"a").expect("touch a");

        let mut evicted = Vec::new();
        let c = cache
            .reserve("c", Extent::new(2, 2), &mut evicted)
            .expect("eviction creates room");
        assert_eq!(c.status(), ReservationStatus::Vacant);
        assert_eq!(evicted.len(), 1);
        assert_eq!(evicted[0].key(), &"b");
        assert_eq!(evicted[0].metadata(), &2);
        assert!(cache.lookup(&"a").is_some());
        assert!(cache.lookup(&"b").is_none());
    }

    #[test]
    fn invalidated_revision_can_coexist_with_leased_pixels() {
        let mut cache = cache(Extent::new(4, 2), 1);
        let old = reserve_vacant(&mut cache, "glyph", Extent::new(2, 2));
        let old_publication = cache.publish(old.entry(), 4).expect("old publication");
        assert!(
            cache
                .invalidate(&"glyph")
                .expect("old revision")
                .reuse_deferred()
        );

        let new = reserve_vacant(&mut cache, "glyph", Extent::new(2, 2));
        assert_ne!(old.placement().allocation(), new.placement().allocation());
        drop(cache.publish(new.entry(), 5).expect("new publication"));
        assert_eq!(*cache.lookup(&"glyph").expect("new revision").metadata(), 5);

        drop(old_publication);
        assert_eq!(cache.reclaim(), 1);
        assert_eq!(cache.stats().retired_entries(), 0);
    }

    #[test]
    fn aborted_slot_reuse_changes_generation() {
        let mut cache = cache(Extent::new(2, 2), 1);
        let old = reserve_vacant(&mut cache, "old", Extent::new(2, 2));
        assert_eq!(cache.abort(old.entry()), Ok("old"));
        let new = reserve_vacant(&mut cache, "new", Extent::new(2, 2));
        assert_eq!(old.entry().index(), new.entry().index());
        assert_ne!(old.entry().generation(), new.entry().generation());
        assert!(!cache.is_ready(old.entry()));
        assert!(matches!(
            cache.publish(old.entry(), 9),
            Err(PublishError::InvalidEntry(id)) if id == old.entry()
        ));
    }

    #[test]
    fn batch_leases_deduplicate_and_validate_before_pinning() {
        let mut cache = cache(Extent::new(4, 2), 1);
        let a = reserve_vacant(&mut cache, "a", Extent::new(2, 2));
        drop(cache.publish(a.entry(), 1).expect("publish a"));
        let pending = reserve_vacant(&mut cache, "pending", Extent::new(2, 2));

        assert!(matches!(
            cache.lease([a.entry(), pending.entry()]),
            Err(LeaseError::NotReady(id)) if id == pending.entry()
        ));
        assert_eq!(cache.stats().leased_entries(), 0);
        let lease = cache
            .lease([a.entry(), a.entry()])
            .expect("ready entry leases");
        assert_eq!(lease.entries(), 1);
        assert_eq!(cache.stats().leased_entries(), 1);
    }

    #[test]
    fn padding_and_fragmentation_are_observable() {
        let mut cache =
            AtlasCache::<&str, ()>::new(AtlasConfig::new(Extent::new(8, 8), 1).with_padding(1))
                .expect("valid cache");
        let reserved = cache
            .reserve("a", Extent::new(2, 3), &mut Vec::new())
            .expect("reservation");
        assert_eq!(
            reserved.placement().allocation().extent(),
            Extent::new(4, 5)
        );
        assert_eq!(reserved.placement().content(), Rect::new(1, 1, 2, 3));

        let page = cache
            .page_stats(reserved.placement().page())
            .expect("page stats");
        assert_eq!(page.reserved_entries(), 1);
        assert_eq!(page.allocated_texels(), 20);
        assert_eq!(page.free_texels(), 44);
        assert!(page.free_regions() >= 1);
        assert!(page.largest_free_region().is_some());
        assert_eq!(cache.metrics().reservations(), 1);
    }
}
