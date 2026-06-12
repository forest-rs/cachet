// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;
use core::{cmp::Reverse, fmt, hash::Hash, mem};

use hashbrown::HashMap;

use crate::{Budget, Epoch, EpochSummary, Priority, Request, ResidencyHandle};

/// Error returned by [`ResidencyTracker::admit`](crate::ResidencyTracker::admit).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    /// The requested admission would exceed the configured hard capacity.
    OverCapacity {
        /// Cost already admitted into the tracker.
        used_cost: u32,
        /// Cost requested for the new resident.
        requested_cost: u32,
        /// Configured hard capacity for the tracker.
        capacity: u32,
    },
    /// The tracker cannot mint another unique residency handle.
    HandleExhausted,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OverCapacity {
                used_cost,
                requested_cost,
                capacity,
            } => write!(
                f,
                "admitting cost {requested_cost} would exceed capacity {capacity} with {used_cost} already used"
            ),
            Self::HandleExhausted => {
                f.write_str("residency tracker cannot mint another unique handle")
            }
        }
    }
}

impl core::error::Error for AdmissionError {}

/// Resident metadata returned from [`ResidencyTracker`] queries and eviction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResidentEntry<K> {
    key: K,
    handle: ResidencyHandle,
    generation: u64,
    cost: u32,
    priority: Priority,
    last_touched: Epoch,
}

impl<K> ResidentEntry<K> {
    /// Returns the logical key that was admitted.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the residency handle minted for this resident.
    #[must_use]
    pub const fn handle(&self) -> ResidencyHandle {
        self.handle
    }

    /// Returns the generation copied from the admitted request.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the admitted cost recorded for this resident.
    #[must_use]
    pub const fn cost(&self) -> u32 {
        self.cost
    }

    /// Returns the priority copied from the admitted request.
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.priority
    }

    /// Returns the last epoch in which this resident was marked used.
    #[must_use]
    pub const fn last_touched(&self) -> Epoch {
        self.last_touched
    }
}

/// One outcome reported by [`ResidencyTracker::process_requests`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProcessedRequest<K> {
    /// The queued request was admitted as a resident.
    Admitted {
        /// The queued request that was processed.
        request: Request<K>,
        /// The residency handle assigned to the admitted resident.
        handle: ResidencyHandle,
        /// The total used cost after this admission completed.
        used_cost_after: u32,
        /// Whether this admission crossed the configured soft limit.
        crossed_soft_limit: bool,
        /// Any stale resident displaced because the same logical key was
        /// re-requested with a newer generation before this admission
        /// completed.
        displaced: Option<ResidentEntry<K>>,
    },
    /// The queued request matched an already-resident key and reused it.
    AlreadyResident {
        /// The queued request that was processed.
        request: Request<K>,
        /// The existing residency handle for the resident key.
        handle: ResidencyHandle,
    },
    /// The queued request could not be admitted.
    Rejected {
        /// The queued request that was processed.
        request: Request<K>,
        /// The cost returned by the caller-supplied cost model.
        cost: u32,
        /// Any stale resident displaced because the same logical key was
        /// re-requested with a newer generation before the rejection was
        /// known.
        displaced: Option<ResidentEntry<K>>,
    },
}

impl<K> ProcessedRequest<K> {
    /// Returns the processed request.
    #[must_use]
    pub const fn request(&self) -> &Request<K> {
        match self {
            Self::Admitted { request, .. }
            | Self::AlreadyResident { request, .. }
            | Self::Rejected { request, .. } => request,
        }
    }

    /// Returns the resulting residency handle, if one exists.
    #[must_use]
    pub const fn handle(&self) -> Option<ResidencyHandle> {
        match self {
            Self::Admitted { handle, .. } | Self::AlreadyResident { handle, .. } => Some(*handle),
            Self::Rejected { .. } => None,
        }
    }

    /// Returns the total used cost after an admission completed, if present.
    #[must_use]
    pub const fn used_cost_after(&self) -> Option<u32> {
        match self {
            Self::Admitted {
                used_cost_after, ..
            } => Some(*used_cost_after),
            Self::AlreadyResident { .. } | Self::Rejected { .. } => None,
        }
    }

    /// Returns whether an admission crossed the configured soft limit.
    #[must_use]
    pub const fn crossed_soft_limit(&self) -> bool {
        match self {
            Self::Admitted {
                crossed_soft_limit, ..
            } => *crossed_soft_limit,
            Self::AlreadyResident { .. } | Self::Rejected { .. } => false,
        }
    }

    /// Returns any displaced resident carried by this outcome.
    #[must_use]
    pub const fn displaced(&self) -> Option<&ResidentEntry<K>> {
        match self {
            Self::Admitted { displaced, .. } | Self::Rejected { displaced, .. } => {
                displaced.as_ref()
            }
            Self::AlreadyResident { .. } => None,
        }
    }
}

/// Caller-owned sink for streamed batch-processing outcomes.
///
/// Callers can implement this trait to receive processed outcomes and evictions
/// directly while reusing their own output storage across batches.
pub trait RequestProcessingSink<K> {
    /// Receives one outcome for a queued request.
    fn processed(&mut self, processed: ProcessedRequest<K>);

    /// Receives one resident evicted to make room during processing.
    fn evicted(&mut self, evicted: ResidentEntry<K>);
}

/// Caller-owned batch context passed to [`ResidencyTracker::process_requests`].
///
/// The batch owns reusable request scratch plus a caller-chosen sink for
/// streamed outcomes. Reusing one batch across epochs lets callers avoid
/// allocating owned report vectors on the hot path.
#[derive(Clone, Debug)]
pub struct RequestProcessingBatch<K, S> {
    queued: Vec<Request<K>>,
    sink: S,
}

impl<K, S> RequestProcessingBatch<K, S> {
    /// Creates a request-processing batch from a caller-owned sink.
    #[must_use]
    pub fn new(sink: S) -> Self {
        Self {
            queued: Vec::new(),
            sink,
        }
    }

    /// Creates a batch with preallocated request scratch capacity.
    #[must_use]
    pub fn with_capacity(request_capacity: usize, sink: S) -> Self {
        Self {
            queued: Vec::with_capacity(request_capacity),
            sink,
        }
    }

    /// Returns the sink carried by this batch.
    #[must_use]
    pub const fn sink(&self) -> &S {
        &self.sink
    }

    /// Returns the sink carried by this batch mutably.
    #[must_use]
    pub fn sink_mut(&mut self) -> &mut S {
        &mut self.sink
    }

    /// Returns the sink, consuming the batch.
    #[must_use]
    pub fn into_sink(self) -> S {
        self.sink
    }

    fn capture_requests(&mut self, requests: &mut Vec<Request<K>>) {
        // Swap the tracker's current request buffer into the batch so the
        // batch can sort and drain it in place. The tracker receives the
        // batch's previous scratch buffer back, which preserves capacity for
        // the next round of `request()` calls instead of dropping it.
        self.queued.clear();
        mem::swap(&mut self.queued, requests);
    }
}

/// Reusable output sink for residency batch processing.
///
/// The two result streams mean different things:
///
/// - `processed` reports what happened to each queued request
/// - `evicted` reports unrelated residents removed to make room under budget
///   pressure
///
/// This type owns `Vec` storage, but it is meant to be kept inside a
/// [`RequestProcessingBatch`] and reused across batches. Call [`Self::clear`]
/// before processing if the previous results are no longer needed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestProcessingOutput<K> {
    processed: Vec<ProcessedRequest<K>>,
    evicted: Vec<ResidentEntry<K>>,
}

impl<K> RequestProcessingOutput<K> {
    /// Creates empty request-processing output.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            processed: Vec::new(),
            evicted: Vec::new(),
        }
    }

    /// Creates output with preallocated processed-result capacity.
    #[must_use]
    pub fn with_capacity(processed_capacity: usize) -> Self {
        Self {
            processed: Vec::with_capacity(processed_capacity),
            evicted: Vec::new(),
        }
    }

    /// Creates output from existing vectors.
    #[must_use]
    pub const fn from_parts(
        processed: Vec<ProcessedRequest<K>>,
        evicted: Vec<ResidentEntry<K>>,
    ) -> Self {
        Self { processed, evicted }
    }

    /// Clears previous results while preserving allocated capacity.
    pub fn clear(&mut self) {
        self.processed.clear();
        self.evicted.clear();
    }

    /// Returns one outcome per processed queued request.
    #[must_use]
    pub fn processed(&self) -> &[ProcessedRequest<K>] {
        &self.processed
    }

    /// Returns the residents evicted to make room during processing.
    #[must_use]
    pub fn evicted(&self) -> &[ResidentEntry<K>] {
        &self.evicted
    }

    /// Returns the owned result vectors.
    #[must_use]
    pub fn into_parts(self) -> (Vec<ProcessedRequest<K>>, Vec<ResidentEntry<K>>) {
        (self.processed, self.evicted)
    }
}

impl<K> Default for RequestProcessingOutput<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K> RequestProcessingSink<K> for RequestProcessingOutput<K> {
    fn processed(&mut self, processed: ProcessedRequest<K>) {
        self.processed.push(processed);
    }

    fn evicted(&mut self, evicted: ResidentEntry<K>) {
        self.evicted.push(evicted);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResidentSlot<K> {
    entry: ResidentEntry<K>,
    prev: Option<ResidencyHandle>,
    next: Option<ResidencyHandle>,
}

enum PreparedRequest<K> {
    Fresh,
    AlreadyResident { handle: ResidencyHandle },
    Displaced { resident: ResidentEntry<K> },
}

/// Small first-slice residency tracker with explicit request and admission APIs.
///
/// The tracker is intentionally boring: callers queue [`Request`] values,
/// decide what to admit, and then use resident handles to track usage and
/// eviction.
///
/// Costs are caller-defined budget units. The tracker does not assume they are
/// bytes or pixels; it only enforces them against the configured [`Budget`].
///
/// Implementation note:
/// handle lookup currently uses a sparse `Vec<Option<usize>>` keyed by handle
/// id. That keeps resident-handle operations `O(1)`, but the side table grows
/// with total lifetime admissions rather than current resident count. Long-lived
/// trackers with heavy churn may eventually want a compaction or handle-reuse
/// strategy.
///
/// Complexity:
///
/// - [`Self::resident`], [`Self::resident_by_key`], [`Self::resident_handle_for_key`],
///   [`Self::mark_used`], [`Self::invalidate_key`], and [`Self::evict_lru`] are `O(1)`
/// - [`Self::process_requests`] is `O(n log n + e * r)` over queued requests,
///   evictions, and live residents because it sorts by priority and uses a
///   priority-aware eviction scan
///
/// # Examples
///
/// ```
/// use cachet_residency::{Budget, Epoch, Priority, Request, ResidencyTracker};
///
/// let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
/// tracker.begin_epoch(Epoch::new(1));
///
/// let request = Request::new("icon:save", Priority::new(0, 50), 7);
/// tracker.request(request.clone());
/// let handle = tracker.admit(&request, 1).expect("room for one icon");
///
/// assert!(tracker.mark_used(handle));
/// assert_eq!(tracker.resident(handle).expect("resident").generation(), 7);
/// assert_eq!(
///     tracker
///         .resident_by_key(&"icon:save")
///         .expect("resident by key")
///         .handle(),
///     handle
/// );
/// ```
///
/// Reusable batch processing with collected output looks like this:
///
/// ```
/// use cachet_residency::{
///     Budget, Epoch, Priority, Request, RequestProcessingBatch, RequestProcessingOutput,
///     ResidencyTracker,
/// };
///
/// let mut tracker = ResidencyTracker::new(Budget::new(3, 2));
/// tracker.begin_epoch(Epoch::new(1));
///
/// tracker.request(Request::new("hero", Priority::new(1, 10), 0));
/// tracker.request(Request::new("prefetch", Priority::new(0, 10), 0));
///
/// let mut batch = RequestProcessingBatch::new(RequestProcessingOutput::new());
/// tracker
///     .process_requests(&mut batch, |request| match *request.key() {
///         "hero" => 2,
///         "prefetch" => 2,
///         _ => 1,
///     })
///     .expect("valid admissions");
///
/// assert_eq!(batch.sink().processed().len(), 2);
/// assert!(batch.sink().processed()[0].handle().is_some());
/// assert!(!batch.sink().processed()[0].crossed_soft_limit());
/// assert!(batch.sink().evicted().is_empty());
/// assert_eq!(batch.sink().processed()[1].request().key(), &"prefetch");
/// assert!(batch.sink().processed()[1].handle().is_none());
/// ```
#[derive(Clone, Debug)]
pub struct ResidencyTracker<K> {
    budget: Budget,
    epoch: Epoch,
    next_handle: u64,
    requests: Vec<Request<K>>,
    residents: Vec<ResidentSlot<K>>,
    // TODO(cachet): Add handle compaction or reuse. Long-lived, high-churn
    // trackers currently grow this side table with lifetime admissions.
    handle_to_index: Vec<Option<usize>>,
    key_to_handle: HashMap<K, ResidencyHandle>,
    lru_head: Option<ResidencyHandle>,
    lru_tail: Option<ResidencyHandle>,
    admissions_this_epoch: usize,
    evictions_this_epoch: usize,
    invalidations_this_epoch: usize,
    used_cost: u32,
}

impl<K> ResidencyTracker<K> {
    /// Creates a tracker for the provided budget.
    #[must_use]
    pub fn new(budget: Budget) -> Self {
        Self {
            budget,
            epoch: Epoch::default(),
            next_handle: 0,
            requests: Vec::new(),
            residents: Vec::new(),
            handle_to_index: Vec::new(),
            key_to_handle: HashMap::new(),
            lru_head: None,
            lru_tail: None,
            admissions_this_epoch: 0,
            evictions_this_epoch: 0,
            invalidations_this_epoch: 0,
            used_cost: 0,
        }
    }

    /// Returns the current budget.
    #[must_use]
    pub const fn budget(&self) -> Budget {
        self.budget
    }

    /// Returns the active epoch.
    #[must_use]
    pub const fn epoch(&self) -> Epoch {
        self.epoch
    }

    /// Starts a new epoch and clears per-epoch counters.
    pub fn begin_epoch(&mut self, epoch: Epoch) {
        self.epoch = epoch;
        self.admissions_this_epoch = 0;
        self.evictions_this_epoch = 0;
        self.invalidations_this_epoch = 0;
        self.requests.clear();
    }

    /// Queues a logical residency request.
    pub fn request(&mut self, request: Request<K>) {
        self.requests.push(request);
    }

    /// Returns the currently queued requests.
    #[must_use]
    pub fn requests(&self) -> &[Request<K>] {
        &self.requests
    }

    /// Returns an iterator over the live resident set.
    pub fn residents(&self) -> impl Iterator<Item = &ResidentEntry<K>> {
        self.residents.iter().map(|resident| &resident.entry)
    }

    /// Returns the total cost currently admitted.
    #[must_use]
    pub const fn used_cost(&self) -> u32 {
        self.used_cost
    }

    /// Returns whether a new resident of `cost` fits inside the hard capacity.
    #[must_use]
    pub const fn can_admit(&self, cost: u32) -> bool {
        self.used_cost.saturating_add(cost) <= self.budget.capacity()
    }

    /// Returns whether admitting `cost` would stay within the preferred soft limit.
    #[must_use]
    pub const fn within_soft_limit(&self, cost: u32) -> bool {
        self.used_cost.saturating_add(cost) <= self.budget.soft_limit()
    }

    /// Returns the resident entry for a handle, if present.
    #[must_use]
    pub fn resident(&self, handle: ResidencyHandle) -> Option<&ResidentEntry<K>> {
        let index = self.index_for(handle)?;
        Some(&self.residents[index].entry)
    }

    /// Marks a resident as used in the current epoch.
    pub fn mark_used(&mut self, handle: ResidencyHandle) -> bool {
        let Some(index) = self.index_for(handle) else {
            return false;
        };
        self.residents[index].entry.last_touched = self.epoch;
        self.move_to_tail(handle);
        true
    }

    /// Finishes the current epoch and reports summary counters.
    #[must_use]
    pub fn end_epoch(&self) -> EpochSummary {
        EpochSummary::new(
            self.epoch,
            self.requests.len(),
            self.admissions_this_epoch,
            self.evictions_this_epoch,
            self.invalidations_this_epoch,
            self.used_cost,
        )
    }

    fn next_handle(&mut self) -> Result<ResidencyHandle, AdmissionError> {
        let handle = ResidencyHandle::new(self.next_handle);
        self.next_handle = self
            .next_handle
            .checked_add(1)
            .ok_or(AdmissionError::HandleExhausted)?;
        Ok(handle)
    }

    fn index_for(&self, handle: ResidencyHandle) -> Option<usize> {
        let handle_index = usize::try_from(handle.get()).ok()?;
        self.handle_to_index
            .get(handle_index)
            .and_then(|index| *index)
    }

    fn link_new_tail(&mut self, handle: ResidencyHandle) {
        if let Some(tail) = self.lru_tail {
            if let Some(tail_index) = self.index_for(tail) {
                self.residents[tail_index].next = Some(handle);
            }
        } else {
            self.lru_head = Some(handle);
        }
        self.lru_tail = Some(handle);
    }

    fn move_to_tail(&mut self, handle: ResidencyHandle) {
        if self.lru_tail == Some(handle) {
            return;
        }

        let Some(index) = self.index_for(handle) else {
            return;
        };
        let prev = self.residents[index].prev;
        let next = self.residents[index].next;

        match prev {
            Some(prev_handle) => {
                if let Some(prev_index) = self.index_for(prev_handle) {
                    self.residents[prev_index].next = next;
                }
            }
            None => {
                self.lru_head = next;
            }
        }

        if let Some(next_handle) = next
            && let Some(next_index) = self.index_for(next_handle)
        {
            self.residents[next_index].prev = prev;
        }

        self.residents[index].prev = self.lru_tail;
        self.residents[index].next = None;

        if let Some(tail) = self.lru_tail
            && let Some(tail_index) = self.index_for(tail)
        {
            self.residents[tail_index].next = Some(handle);
        }
        self.lru_tail = Some(handle);
    }
}

impl<K> ResidencyTracker<K>
where
    K: Eq + Hash,
{
    /// Evicts the least-recently-used resident.
    ///
    /// This method is intentionally pure recency. Higher-level admission paths
    /// such as [`process_requests`](Self::process_requests) apply additional
    /// priority-aware eviction policy before calling into removal internals.
    pub fn evict_lru(&mut self) -> Option<ResidentEntry<K>> {
        let handle = self.lru_head?;
        let evicted = self.remove_handle(handle)?;
        self.evictions_this_epoch += 1;
        Some(evicted)
    }

    /// Returns the residency handle currently assigned to a logical key.
    #[must_use]
    pub fn resident_handle_for_key(&self, key: &K) -> Option<ResidencyHandle> {
        self.key_to_handle.get(key).copied()
    }

    /// Returns the resident entry currently assigned to a logical key.
    #[must_use]
    pub fn resident_by_key(&self, key: &K) -> Option<&ResidentEntry<K>> {
        let handle = self.resident_handle_for_key(key)?;
        self.resident(handle)
    }

    /// Invalidates the resident that matches the provided logical key.
    ///
    /// Invalidation is caller-driven staleness: the caller already knows this
    /// resident is no longer valid and wants it removed immediately, rather
    /// than waiting for budget pressure or a newer generation to displace it.
    pub fn invalidate_key(&mut self, key: &K) -> usize {
        let Some(handle) = self.resident_handle_for_key(key) else {
            return 0;
        };
        let removed = self.remove_handle(handle);
        if removed.is_some() {
            self.invalidations_this_epoch += 1;
            1
        } else {
            0
        }
    }

    /// Removes the resident for a specific handle.
    ///
    /// This is a direct caller-driven removal, distinct from invalidation or
    /// budget-driven eviction. It is useful when a higher-level integration
    /// needs to roll back an admitted resident because some external resource
    /// step, such as physical placement, could not be completed.
    pub fn remove(&mut self, handle: ResidencyHandle) -> Option<ResidentEntry<K>> {
        self.remove_handle(handle)
    }

    fn prepare_request(&mut self, request: &Request<K>) -> PreparedRequest<K> {
        let Some(handle) = self.resident_handle_for_key(request.key()) else {
            return PreparedRequest::Fresh;
        };

        let Some(resident) = self.resident(handle) else {
            return PreparedRequest::Fresh;
        };

        if resident.generation() >= request.generation() {
            let _ = self.mark_used(handle);
            return PreparedRequest::AlreadyResident { handle };
        }

        let displaced = self
            .remove_handle(handle)
            .expect("key index only points at live residents");
        self.invalidations_this_epoch += 1;
        PreparedRequest::Displaced {
            resident: displaced,
        }
    }

    fn remove_handle(&mut self, handle: ResidencyHandle) -> Option<ResidentEntry<K>> {
        let index = self.index_for(handle)?;
        let prev = self.residents[index].prev;
        let next = self.residents[index].next;

        match prev {
            Some(prev_handle) => {
                if let Some(prev_index) = self.index_for(prev_handle) {
                    self.residents[prev_index].next = next;
                }
            }
            None => {
                self.lru_head = next;
            }
        }

        match next {
            Some(next_handle) => {
                if let Some(next_index) = self.index_for(next_handle) {
                    self.residents[next_index].prev = prev;
                }
            }
            None => {
                self.lru_tail = prev;
            }
        }

        let handle_index = usize::try_from(handle.get()).ok()?;
        self.handle_to_index[handle_index] = None;

        let removed = self.residents.swap_remove(index);
        self.key_to_handle.remove(&removed.entry.key);

        if let Some(moved) = self.residents.get(index) {
            let moved_index = usize::try_from(moved.entry.handle.get()).ok()?;
            self.handle_to_index[moved_index] = Some(index);
        }

        self.used_cost = self.used_cost.saturating_sub(removed.entry.cost);
        Some(removed.entry)
    }
}

impl<K> ResidencyTracker<K>
where
    K: Clone + Eq + Hash,
{
    /// Admits a resident entry using the data from a request.
    ///
    /// If the key is already resident with the same or newer generation, this
    /// returns the existing handle and marks that resident as used. If the key
    /// is resident with an older generation, the stale resident is displaced
    /// before the new admission is attempted.
    ///
    /// `cost` is a caller-defined budget unit that is checked against the
    /// tracker's [`Budget`].
    pub fn admit(
        &mut self,
        request: &Request<K>,
        cost: u32,
    ) -> Result<ResidencyHandle, AdmissionError> {
        match self.prepare_request(request) {
            PreparedRequest::AlreadyResident { handle } => Ok(handle),
            PreparedRequest::Fresh | PreparedRequest::Displaced { .. } => {
                self.admit_fresh(request, cost)
            }
        }
    }

    /// Processes queued requests using a caller-owned batch context.
    ///
    /// The caller supplies a cost model for each queued request. Requests with
    /// resident keys are deduplicated automatically, stale generations displace
    /// older residents, and lower-priority residents are evicted as needed to
    /// make room. Among evictable residents with the same priority, older
    /// residents are preferred for eviction.
    ///
    /// This method reorders queued requests internally by priority, but it
    /// reuses the batch's request scratch and streams outcomes into the batch's
    /// sink instead of allocating owned report vectors for every call.
    pub fn process_requests<F, S>(
        &mut self,
        batch: &mut RequestProcessingBatch<K, S>,
        mut cost_for: F,
    ) -> Result<(), AdmissionError>
    where
        F: FnMut(&Request<K>) -> u32,
        S: RequestProcessingSink<K>,
    {
        batch.capture_requests(&mut self.requests);
        batch
            .queued
            .sort_unstable_by_key(|request| Reverse(request.priority()));

        for request in batch.queued.drain(..) {
            let displaced = match self.prepare_request(&request) {
                PreparedRequest::Fresh => None,
                PreparedRequest::AlreadyResident { handle } => {
                    batch
                        .sink
                        .processed(ProcessedRequest::AlreadyResident { request, handle });
                    continue;
                }
                PreparedRequest::Displaced { resident } => Some(resident),
            };

            let cost = cost_for(&request);
            if cost > self.budget.capacity() {
                batch.sink.processed(ProcessedRequest::Rejected {
                    request,
                    cost,
                    displaced,
                });
                continue;
            }

            while !self.can_admit(cost) {
                let Some(resident) = self.evict_for_request(request.priority()) else {
                    break;
                };
                batch.sink.evicted(resident);
            }

            if self.can_admit(cost) {
                let handle = self.admit_fresh(&request, cost)?;
                let used_cost_after = self.used_cost;
                batch.sink.processed(ProcessedRequest::Admitted {
                    request,
                    handle,
                    used_cost_after,
                    crossed_soft_limit: used_cost_after > self.budget.soft_limit(),
                    displaced,
                });
            } else {
                batch.sink.processed(ProcessedRequest::Rejected {
                    request,
                    cost,
                    displaced,
                });
            }
        }

        Ok(())
    }

    fn admit_fresh(
        &mut self,
        request: &Request<K>,
        cost: u32,
    ) -> Result<ResidencyHandle, AdmissionError> {
        if !self.can_admit(cost) {
            return Err(AdmissionError::OverCapacity {
                used_cost: self.used_cost,
                requested_cost: cost,
                capacity: self.budget.capacity(),
            });
        }

        let handle = self.next_handle()?;
        let index = self.residents.len();
        self.handle_to_index.push(Some(index));
        self.key_to_handle.insert(request.key().clone(), handle);
        self.used_cost = self.used_cost.saturating_add(cost);
        self.admissions_this_epoch += 1;

        let slot = ResidentSlot {
            entry: ResidentEntry {
                key: request.key().clone(),
                handle,
                generation: request.generation(),
                cost,
                priority: request.priority(),
                last_touched: self.epoch,
            },
            prev: self.lru_tail,
            next: None,
        };
        self.residents.push(slot);
        self.link_new_tail(handle);
        Ok(handle)
    }

    fn evict_for_request(&mut self, request_priority: Priority) -> Option<ResidentEntry<K>> {
        // TODO(cachet): Add a cheaper priority-aware eviction index if real
        // workloads show this resident scan on every eviction is too costly.
        let handle = self
            .residents
            .iter()
            .filter(|resident| resident.entry.priority <= request_priority)
            .min_by_key(|resident| {
                (
                    resident.entry.priority,
                    resident.entry.last_touched,
                    resident.entry.handle,
                )
            })
            .map(|resident| resident.entry.handle)?;

        let evicted = self.remove_handle(handle)?;
        self.evictions_this_epoch += 1;
        Some(evicted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSink<K> {
        processed: Vec<ProcessedRequest<K>>,
        evicted: Vec<ResidentEntry<K>>,
    }

    impl<K> TestSink<K> {
        fn new() -> Self {
            Self {
                processed: Vec::new(),
                evicted: Vec::new(),
            }
        }
    }

    impl<K> RequestProcessingSink<K> for TestSink<K> {
        fn processed(&mut self, processed: ProcessedRequest<K>) {
            self.processed.push(processed);
        }

        fn evicted(&mut self, evicted: ResidentEntry<K>) {
            self.evicted.push(evicted);
        }
    }

    fn process_requests_output<K, F>(
        tracker: &mut ResidencyTracker<K>,
        cost_for: F,
    ) -> Result<RequestProcessingOutput<K>, AdmissionError>
    where
        K: Clone + Eq + core::hash::Hash,
        F: FnMut(&Request<K>) -> u32,
    {
        let mut batch = RequestProcessingBatch::new(RequestProcessingOutput::new());
        tracker.process_requests(&mut batch, cost_for)?;
        Ok(batch.into_sink())
    }

    fn process_requests_for_test<F>(
        mut tracker: ResidencyTracker<&'static str>,
        cost_for: F,
    ) -> RequestProcessingOutput<&'static str>
    where
        F: FnMut(&Request<&'static str>) -> u32,
    {
        process_requests_output(&mut tracker, cost_for).expect("batch processing should complete")
    }

    #[test]
    fn lru_eviction_prefers_oldest_touch() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
        tracker.begin_epoch(Epoch::new(1));

        let left = Request::new(1_u32, Priority::new(0, 10), 0);
        let right = Request::new(2_u32, Priority::new(0, 20), 0);
        let left_handle = tracker.admit(&left, 1).expect("left fits");
        let _right_handle = tracker.admit(&right, 1).expect("right fits");

        tracker.begin_epoch(Epoch::new(2));
        assert!(tracker.mark_used(left_handle));

        let evicted = tracker.evict_lru().expect("one resident is evicted");
        assert_eq!(evicted.key(), &2_u32);
    }

    #[test]
    fn tracker_uses_capacity_not_soft_limit_for_admission() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 2));
        tracker.begin_epoch(Epoch::new(1));

        let first = Request::new(1_u32, Priority::new(0, 10), 0);
        let second = Request::new(2_u32, Priority::new(0, 10), 0);
        let third = Request::new(3_u32, Priority::new(0, 10), 0);

        assert!(tracker.within_soft_limit(2));
        tracker.admit(&first, 2).expect("first fits");
        assert!(!tracker.within_soft_limit(1));
        tracker
            .admit(&second, 1)
            .expect("second still fits in capacity");

        let error = tracker
            .admit(&third, 2)
            .expect_err("third exceeds capacity");
        assert!(matches!(
            error,
            AdmissionError::OverCapacity {
                used_cost: 3,
                requested_cost: 2,
                capacity: 4
            }
        ));
    }

    #[test]
    fn resident_lookup_by_key_returns_handle_and_entry() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
        tracker.begin_epoch(Epoch::new(1));

        let request = Request::new("glyph", Priority::new(0, 10), 2);
        let handle = tracker.admit(&request, 1).expect("request fits");

        assert_eq!(tracker.resident_handle_for_key(&"glyph"), Some(handle));
        assert_eq!(
            tracker
                .resident_by_key(&"glyph")
                .expect("resident by key")
                .generation(),
            2
        );
    }

    #[test]
    fn admit_reuses_existing_handle_for_same_generation() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
        tracker.begin_epoch(Epoch::new(1));

        let request = Request::new("glyph", Priority::new(0, 10), 2);
        let first = tracker.admit(&request, 1).expect("first fits");
        let second = tracker.admit(&request, 9).expect("duplicate reuses handle");

        assert_eq!(first, second);
        assert_eq!(tracker.residents().count(), 1);
        assert_eq!(tracker.used_cost(), 1);
    }

    #[test]
    fn newer_generation_replaces_stale_resident() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
        tracker.begin_epoch(Epoch::new(1));

        let stale = Request::new("glyph", Priority::new(0, 10), 2);
        let fresh = Request::new("glyph", Priority::new(0, 20), 3);
        let stale_handle = tracker.admit(&stale, 1).expect("stale fits");
        let fresh_handle = tracker.admit(&fresh, 2).expect("fresh fits");

        assert_ne!(stale_handle, fresh_handle);
        assert!(tracker.resident(stale_handle).is_none());
        assert_eq!(
            tracker
                .resident_by_key(&"glyph")
                .expect("fresh resident by key")
                .generation(),
            3
        );
        assert_eq!(tracker.used_cost(), 2);
        assert_eq!(tracker.end_epoch().invalidations(), 1);
    }

    #[test]
    fn invalidate_key_uses_key_index() {
        let mut tracker = ResidencyTracker::new(Budget::new(4, 4));
        tracker.begin_epoch(Epoch::new(1));

        let stale = Request::new("stale", Priority::new(0, 30), 0);
        let fresh = Request::new("fresh", Priority::new(0, 10), 0);

        tracker.admit(&stale, 1).expect("stale resident fits");
        tracker.admit(&fresh, 1).expect("fresh resident fits");

        assert_eq!(tracker.invalidate_key(&"stale"), 1);
        assert_eq!(tracker.used_cost(), 1);
        assert_eq!(tracker.resident_by_key(&"stale"), None);
        assert_eq!(
            tracker
                .resident_by_key(&"fresh")
                .expect("fresh resident remains")
                .key(),
            &"fresh"
        );
    }

    #[test]
    fn process_requests_evicts_and_reports() {
        let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
        tracker.begin_epoch(Epoch::new(1));

        let left = Request::new("left", Priority::new(0, 40), 0);
        let right = Request::new("right", Priority::new(0, 30), 0);
        let _left_handle = tracker.admit(&left, 1).expect("left fits");
        let right_handle = tracker.admit(&right, 1).expect("right fits");

        tracker.begin_epoch(Epoch::new(2));
        assert!(tracker.mark_used(right_handle));

        tracker.request(Request::new("new", Priority::new(1, 90), 0));
        let report =
            process_requests_output(&mut tracker, |_| 1).expect("handle space should remain");

        assert_eq!(report.evicted().len(), 1);
        assert_eq!(report.evicted()[0].key(), &"right");
        assert_eq!(report.processed().len(), 1);

        match &report.processed()[0] {
            ProcessedRequest::Admitted {
                request,
                used_cost_after,
                crossed_soft_limit,
                displaced,
                ..
            } => {
                assert_eq!(request.key(), &"new");
                assert_eq!(*used_cost_after, 2);
                assert!(!crossed_soft_limit);
                assert!(displaced.is_none());
            }
            other => panic!("expected admission, got {other:?}"),
        }

        assert!(tracker.resident_by_key(&"new").is_some());
        assert!(tracker.resident_by_key(&"left").is_some());
        assert!(tracker.resident_by_key(&"right").is_none());
    }

    #[test]
    fn process_requests_does_not_evict_higher_priority_residents_for_lower_priority_work() {
        let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
        tracker.begin_epoch(Epoch::new(1));

        let protected = Request::new("protected", Priority::new(2, 50), 0);
        let background = Request::new("background", Priority::new(0, 50), 0);
        tracker.admit(&protected, 1).expect("protected fits");
        tracker.admit(&background, 1).expect("background fits");

        tracker.begin_epoch(Epoch::new(2));
        tracker.request(Request::new("low", Priority::new(0, 40), 0));
        let report =
            process_requests_output(&mut tracker, |_| 1).expect("batch processing should complete");

        assert!(report.evicted().is_empty());
        assert!(matches!(
            &report.processed()[0],
            ProcessedRequest::Rejected {
                request,
                cost: 1,
                displaced: None
            } if request.key() == &"low"
        ));
        assert!(tracker.resident_by_key(&"protected").is_some());
        assert!(tracker.resident_by_key(&"background").is_some());
        assert!(tracker.resident_by_key(&"low").is_none());
    }

    #[test]
    fn process_requests_reports_soft_limit_crossings() {
        let mut tracker = ResidencyTracker::new(Budget::new(3, 2));
        tracker.begin_epoch(Epoch::new(1));

        tracker.request(Request::new("first", Priority::new(1, 20), 0));
        tracker.request(Request::new("second", Priority::new(1, 10), 0));
        tracker.request(Request::new("third", Priority::new(1, 5), 0));

        let report = process_requests_output(&mut tracker, |_| 1)
            .expect("all three should fit in hard capacity");

        assert_eq!(report.processed().len(), 3);
        assert_eq!(report.processed()[0].used_cost_after(), Some(1));
        assert!(!report.processed()[0].crossed_soft_limit());
        assert_eq!(report.processed()[1].used_cost_after(), Some(2));
        assert!(!report.processed()[1].crossed_soft_limit());
        assert_eq!(report.processed()[2].used_cost_after(), Some(3));
        assert!(report.processed()[2].crossed_soft_limit());
    }

    #[test]
    fn process_requests_handles_already_resident_work() {
        let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
        tracker.begin_epoch(Epoch::new(1));

        let resident = Request::new("resident", Priority::new(1, 90), 0);
        let handle = tracker.admit(&resident, 1).expect("resident fits");

        tracker.begin_epoch(Epoch::new(2));
        tracker.request(resident);

        let report = process_requests_for_test(tracker, |_| 1);
        assert!(report.evicted().is_empty());
        assert!(matches!(
            &report.processed()[0],
            ProcessedRequest::AlreadyResident {
                request,
                handle: reused
            } if request.key() == &"resident" && *reused == handle
        ));
    }

    #[test]
    fn process_requests_handles_displaced_work() {
        let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
        tracker.begin_epoch(Epoch::new(1));

        let stale = Request::new("glyph", Priority::new(1, 80), 0);
        let stale_handle = tracker.admit(&stale, 1).expect("stale resident fits");

        tracker.begin_epoch(Epoch::new(2));
        tracker.request(Request::new("glyph", Priority::new(1, 90), 1));

        let report = process_requests_for_test(tracker, |_| 1);
        assert!(report.evicted().is_empty());
        assert!(matches!(
            &report.processed()[0],
            ProcessedRequest::Admitted {
                request,
                displaced: Some(displaced),
                ..
            } if request.key() == &"glyph" && displaced.handle() == stale_handle
        ));
    }

    #[test]
    fn process_requests_handles_rejected_work() {
        let mut tracker = ResidencyTracker::new(Budget::new(1, 1));
        tracker.begin_epoch(Epoch::new(1));
        tracker.request(Request::new("too-large", Priority::new(1, 90), 0));

        let report = process_requests_for_test(tracker, |_| 2);
        assert!(report.evicted().is_empty());
        assert!(matches!(
            &report.processed()[0],
            ProcessedRequest::Rejected {
                request,
                cost: 2,
                displaced: None
            } if request.key() == &"too-large"
        ));
    }

    #[test]
    fn process_requests_handles_rejected_displacement() {
        let mut tracker = ResidencyTracker::new(Budget::new(1, 1));
        tracker.begin_epoch(Epoch::new(1));

        let stale = Request::new("glyph", Priority::new(1, 80), 0);
        let stale_handle = tracker.admit(&stale, 1).expect("stale resident fits");

        tracker.begin_epoch(Epoch::new(2));
        tracker.request(Request::new("glyph", Priority::new(1, 90), 1));

        let report = process_requests_for_test(tracker, |_| 2);
        assert!(report.evicted().is_empty());
        assert!(matches!(
            &report.processed()[0],
            ProcessedRequest::Rejected {
                request,
                cost: 2,
                displaced: Some(displaced)
            } if request.key() == &"glyph" && displaced.handle() == stale_handle
        ));
    }

    #[test]
    fn process_requests_streams_equivalent_outcomes() {
        let mut tracker = ResidencyTracker::new(Budget::new(2, 2));
        tracker.begin_epoch(Epoch::new(1));

        let left = Request::new("left", Priority::new(0, 40), 0);
        let right = Request::new("right", Priority::new(0, 30), 0);
        let _left_handle = tracker.admit(&left, 1).expect("left fits");
        let right_handle = tracker.admit(&right, 1).expect("right fits");

        tracker.begin_epoch(Epoch::new(2));
        assert!(tracker.mark_used(right_handle));
        tracker.request(Request::new("new", Priority::new(1, 90), 0));

        let mut batch = RequestProcessingBatch::new(TestSink::new());
        tracker
            .process_requests(&mut batch, |_| 1)
            .expect("batch processing should complete");
        let sink = batch.into_sink();

        assert_eq!(sink.evicted.len(), 1);
        assert_eq!(sink.evicted[0].key(), &"right");
        assert_eq!(sink.processed.len(), 1);

        match &sink.processed[0] {
            ProcessedRequest::Admitted {
                request,
                used_cost_after,
                crossed_soft_limit,
                displaced,
                ..
            } => {
                assert_eq!(request.key(), &"new");
                assert_eq!(*used_cost_after, 2);
                assert!(!crossed_soft_limit);
                assert!(displaced.is_none());
            }
            other => panic!("expected admission, got {other:?}"),
        }
    }
}
