// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{AllocationError, AllocationId};

/// Identifier for one page inside a [`RectAtlasSet`].
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtlasPageId(u32);

impl AtlasPageId {
    /// Creates an atlas page identifier from a raw integer.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw page identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Packed atlas rectangle returned from [`AtlasSlot::rect`] or
/// [`RectAtlas::free`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasRect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl AtlasRect {
    /// Creates a packed atlas rectangle.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Returns the left coordinate.
    #[must_use]
    pub const fn x(self) -> u16 {
        self.x
    }

    /// Returns the top coordinate.
    #[must_use]
    pub const fn y(self) -> u16 {
        self.y
    }

    /// Returns the rectangle width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Returns the rectangle height.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}

/// Allocation record returned by [`RectAtlas::allocate`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasSlot {
    allocation: AllocationId,
    rect: AtlasRect,
}

impl AtlasSlot {
    /// Creates an atlas allocation from an allocation id and rect.
    #[must_use]
    pub const fn new(allocation: AllocationId, rect: AtlasRect) -> Self {
        Self { allocation, rect }
    }

    /// Returns the opaque allocation id.
    #[must_use]
    pub const fn allocation(self) -> AllocationId {
        self.allocation
    }

    /// Returns the packed atlas rect.
    #[must_use]
    pub const fn rect(self) -> AtlasRect {
        self.rect
    }
}

/// Allocation record returned by [`RectAtlasSet::allocate_in`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PagedAtlasSlot {
    page: AtlasPageId,
    slot: AtlasSlot,
}

impl PagedAtlasSlot {
    /// Creates a paged atlas allocation from a page id and page-local slot.
    #[must_use]
    pub const fn new(page: AtlasPageId, slot: AtlasSlot) -> Self {
        Self { page, slot }
    }

    /// Returns the page that owns this allocation.
    #[must_use]
    pub const fn page(self) -> AtlasPageId {
        self.page
    }

    /// Returns the page-local atlas allocation.
    #[must_use]
    pub const fn slot(self) -> AtlasSlot {
        self.slot
    }

    /// Returns the page-local allocation id.
    #[must_use]
    pub const fn allocation(self) -> AllocationId {
        self.slot.allocation()
    }

    /// Returns the packed atlas rectangle.
    #[must_use]
    pub const fn rect(self) -> AtlasRect {
        self.slot.rect()
    }
}

/// Summary returned by [`RectAtlas::stats`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectAtlasStats {
    width: u16,
    height: u16,
    allocated: u32,
    free_regions: u32,
    next_x: u16,
    next_y: u16,
    row_height: u16,
}

impl RectAtlasStats {
    /// Creates atlas statistics from raw values.
    #[must_use]
    pub const fn new(
        width: u16,
        height: u16,
        allocated: u32,
        free_regions: u32,
        next_x: u16,
        next_y: u16,
        row_height: u16,
    ) -> Self {
        Self {
            width,
            height,
            allocated,
            free_regions,
            next_x,
            next_y,
            row_height,
        }
    }

    /// Returns atlas page width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Returns atlas page height.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }

    /// Returns the number of live allocations.
    #[must_use]
    pub const fn allocated(self) -> u32 {
        self.allocated
    }

    /// Returns the number of reclaimed free regions tracked for reuse.
    ///
    /// A rising count can indicate fragmentation pressure because the first
    /// slice does not coalesce adjacent free regions.
    #[must_use]
    pub const fn free_regions(self) -> u32 {
        self.free_regions
    }

    /// Returns the next placement x coordinate for the shelf allocator.
    #[must_use]
    pub const fn next_x(self) -> u16 {
        self.next_x
    }

    /// Returns the next placement y coordinate for the shelf allocator.
    #[must_use]
    pub const fn next_y(self) -> u16 {
        self.next_y
    }

    /// Returns the active shelf height.
    #[must_use]
    pub const fn row_height(self) -> u16 {
        self.row_height
    }
}

/// Aggregate summary returned by [`RectAtlasSet::stats`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RectAtlasSetStats {
    pages: u32,
    allocated: u32,
    free_regions: u32,
}

impl RectAtlasSetStats {
    /// Creates aggregate atlas-set statistics from raw values.
    #[must_use]
    pub const fn new(pages: u32, allocated: u32, free_regions: u32) -> Self {
        Self {
            pages,
            allocated,
            free_regions,
        }
    }

    /// Returns the number of pages in the set.
    #[must_use]
    pub const fn pages(self) -> u32 {
        self.pages
    }

    /// Returns the total number of live allocations across all pages.
    #[must_use]
    pub const fn allocated(self) -> u32 {
        self.allocated
    }

    /// Returns the total number of free regions tracked across all pages.
    #[must_use]
    pub const fn free_regions(self) -> u32 {
        self.free_regions
    }
}

/// Bounded atlas allocator used by atlas-style workloads.
///
/// This is intentionally simple. It exists to make the atlas integration story
/// concrete without claiming that shelf packing is the final allocation policy.
///
/// Unlike the earlier sketch, this allocator supports freeing by allocation id
/// so residency-layer eviction can actually reclaim space.
///
/// This allocator is still single-page. Multi-page routing remains a higher
/// layer concern for now, even though atlas-oriented consumers may use
/// `cachet_atlas::AtlasClass` to group compatible content.
///
/// Freed regions are reused, but the first slice does not coalesce adjacent
/// free rectangles. Callers can watch [`RectAtlasStats::free_regions`] as a
/// simple fragmentation-pressure signal.
///
/// # Examples
///
/// This example allocates two rectangles, frees one by its `AllocationId`, and
/// then shows that a later allocation can reuse the reclaimed space.
///
/// ```
/// use cachet_storage::RectAtlas;
///
/// let mut atlas = RectAtlas::new(16, 16);
/// let left = atlas.allocate(6, 4).expect("left fits");
/// let right = atlas.allocate(6, 4).expect("right fits");
/// let reclaimed = atlas
///     .free(left.allocation())
///     .expect("a live allocation can be freed");
/// let reused = atlas.allocate(6, 4).expect("freed region can be reused");
///
/// assert_eq!(right.rect().x(), 6);
/// assert_eq!(reclaimed.width(), 6);
/// assert_eq!(reused.rect().x(), 0);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RectAtlas {
    width: u16,
    height: u16,
    next_x: u16,
    next_y: u16,
    row_height: u16,
    allocated: u32,
    next_allocation: u64,
    live_allocations: Vec<Option<AtlasRect>>,
    // TODO(cachet): Add free-region coalescing or stronger fragmentation
    // mitigation once a real workload proves the need.
    free_regions: Vec<AtlasRect>,
}

/// Storage-side multi-page wrapper around several [`RectAtlas`] pages.
///
/// This type keeps page ownership and page-local statistics in
/// `cachet_storage` without teaching the storage layer anything about
/// atlas-specific routing policy.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RectAtlasSet {
    pages: Vec<RectAtlas>,
}

impl RectAtlas {
    /// Creates an atlas allocator for one bounded page.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            next_x: 0,
            next_y: 0,
            row_height: 0,
            allocated: 0,
            next_allocation: 0,
            live_allocations: Vec::new(),
            free_regions: Vec::new(),
        }
    }

    /// Allocates one atlas slot.
    pub fn allocate(&mut self, width: u16, height: u16) -> Result<AtlasSlot, AllocationError> {
        if width == 0 || height == 0 || width > self.width || height > self.height {
            return Err(AllocationError::TooLarge);
        }

        if let Some(index) = self
            .free_regions
            .iter()
            .position(|rect| rect.width() >= width && rect.height() >= height)
        {
            let region = self.free_regions.swap_remove(index);
            let rect = AtlasRect::new(region.x(), region.y(), width, height);
            self.push_remainder_regions(region, width, height);
            return self.finish_allocation(rect);
        }

        if self.next_x.saturating_add(width) > self.width {
            self.next_x = 0;
            self.next_y = self.next_y.saturating_add(self.row_height);
            self.row_height = 0;
        }

        if self.next_y.saturating_add(height) > self.height {
            return Err(AllocationError::Exhausted);
        }

        let rect = AtlasRect::new(self.next_x, self.next_y, width, height);
        self.next_x = self.next_x.saturating_add(width);
        self.row_height = self.row_height.max(height);
        self.finish_allocation(rect)
    }

    /// Frees a live allocation by id and returns its reclaimed rectangle.
    pub fn free(&mut self, allocation: AllocationId) -> Result<AtlasRect, AllocationError> {
        let Some(index) = usize::try_from(allocation.get()).ok() else {
            return Err(AllocationError::UnknownAllocation);
        };
        let Some(slot) = self.live_allocations.get_mut(index) else {
            return Err(AllocationError::UnknownAllocation);
        };
        let Some(rect) = slot.take() else {
            return Err(AllocationError::UnknownAllocation);
        };
        self.allocated = self.allocated.saturating_sub(1);
        self.free_regions.push(rect);
        Ok(rect)
    }

    /// Returns atlas statistics for diagnostics and tests.
    #[must_use]
    pub fn stats(&self) -> RectAtlasStats {
        RectAtlasStats::new(
            self.width,
            self.height,
            self.allocated,
            self.free_regions.len() as u32,
            self.next_x,
            self.next_y,
            self.row_height,
        )
    }

    fn finish_allocation(&mut self, rect: AtlasRect) -> Result<AtlasSlot, AllocationError> {
        let allocation = AllocationId::new(self.next_allocation);
        let Some(index) = usize::try_from(allocation.get()).ok() else {
            return Err(AllocationError::IdExhausted);
        };
        self.next_allocation = self
            .next_allocation
            .checked_add(1)
            .ok_or(AllocationError::IdExhausted)?;
        if self.live_allocations.len() <= index {
            self.live_allocations.resize(index + 1, None);
        }
        self.live_allocations[index] = Some(rect);
        self.allocated = self.allocated.saturating_add(1);
        Ok(AtlasSlot::new(allocation, rect))
    }

    fn push_remainder_regions(&mut self, region: AtlasRect, width: u16, height: u16) {
        let remaining_width = region.width().saturating_sub(width);
        let remaining_height = region.height().saturating_sub(height);

        if remaining_width > 0 {
            self.free_regions.push(AtlasRect::new(
                region.x().saturating_add(width),
                region.y(),
                remaining_width,
                height,
            ));
        }
        if remaining_height > 0 {
            self.free_regions.push(AtlasRect::new(
                region.x(),
                region.y().saturating_add(height),
                region.width(),
                remaining_height,
            ));
        }
    }
}

impl RectAtlasSet {
    /// Creates an empty set of atlas pages.
    #[must_use]
    pub const fn new() -> Self {
        Self { pages: Vec::new() }
    }

    /// Adds one page to the set and returns its storage-side page id.
    pub fn add_page(&mut self, width: u16, height: u16) -> Result<AtlasPageId, AllocationError> {
        let page_id = AtlasPageId::new(
            u32::try_from(self.pages.len()).map_err(|_| AllocationError::IdExhausted)?,
        );
        self.pages.push(RectAtlas::new(width, height));
        Ok(page_id)
    }

    /// Returns the atlas page for a page id, if present.
    #[must_use]
    pub fn page(&self, page: AtlasPageId) -> Option<&RectAtlas> {
        let index = usize::try_from(page.get()).ok()?;
        self.pages.get(index)
    }

    /// Returns page-local statistics for one page id, if present.
    #[must_use]
    pub fn page_stats(&self, page: AtlasPageId) -> Option<RectAtlasStats> {
        Some(self.page(page)?.stats())
    }

    /// Allocates within one specific page.
    pub fn allocate_in(
        &mut self,
        page: AtlasPageId,
        width: u16,
        height: u16,
    ) -> Result<PagedAtlasSlot, AllocationError> {
        let index = usize::try_from(page.get()).map_err(|_| AllocationError::UnknownAllocation)?;
        let slot = self
            .pages
            .get_mut(index)
            .ok_or(AllocationError::UnknownAllocation)?
            .allocate(width, height)?;
        Ok(PagedAtlasSlot::new(page, slot))
    }

    /// Frees one paged allocation and returns its reclaimed rectangle.
    pub fn free(&mut self, slot: PagedAtlasSlot) -> Result<AtlasRect, AllocationError> {
        let index =
            usize::try_from(slot.page().get()).map_err(|_| AllocationError::UnknownAllocation)?;
        self.pages
            .get_mut(index)
            .ok_or(AllocationError::UnknownAllocation)?
            .free(slot.allocation())
    }

    /// Returns aggregate diagnostics across all pages.
    #[must_use]
    pub fn stats(&self) -> RectAtlasSetStats {
        let mut allocated = 0_u32;
        let mut free_regions = 0_u32;
        for page in &self.pages {
            let stats = page.stats();
            allocated = allocated.saturating_add(stats.allocated());
            free_regions = free_regions.saturating_add(stats.free_regions());
        }
        RectAtlasSetStats::new(self.pages.len() as u32, allocated, free_regions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shelf_allocator_starts_new_row_when_needed() {
        let mut atlas = RectAtlas::new(8, 8);
        let left = atlas.allocate(4, 3).expect("left fits");
        let right = atlas.allocate(4, 2).expect("right fits");
        let next_row = atlas.allocate(3, 2).expect("new row fits");

        assert_eq!(left.rect().x(), 0);
        assert_eq!(right.rect().x(), 4);
        assert_eq!(next_row.rect().x(), 0);
        assert_eq!(next_row.rect().y(), 3);
    }

    #[test]
    fn freed_rectangles_can_be_reused() {
        let mut atlas = RectAtlas::new(8, 8);
        let first = atlas.allocate(4, 3).expect("first allocation fits");
        let _second = atlas.allocate(4, 3).expect("second allocation fits");
        let reclaimed = atlas
            .free(first.allocation())
            .expect("live allocation can be freed");
        let reused = atlas.allocate(4, 3).expect("freed space can be reused");

        assert_eq!(reclaimed, AtlasRect::new(0, 0, 4, 3));
        assert_eq!(reused.rect(), AtlasRect::new(0, 0, 4, 3));
    }

    #[test]
    fn freeing_unknown_allocations_is_rejected() {
        let mut atlas = RectAtlas::new(8, 8);
        let allocation = atlas.allocate(4, 3).expect("first allocation fits");
        atlas
            .free(allocation.allocation())
            .expect("live allocation can be freed");

        let error = atlas
            .free(allocation.allocation())
            .expect_err("stale allocation ids must be rejected");

        assert_eq!(error, AllocationError::UnknownAllocation);
    }

    #[test]
    fn rect_atlas_set_tracks_page_identity() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(8, 8).expect("page id fits");
        let page1 = pages.add_page(8, 8).expect("page id fits");

        let first = pages.allocate_in(page0, 4, 3).expect("page 0 has room");
        let second = pages.allocate_in(page1, 4, 3).expect("page 1 has room");

        assert_eq!(first.page(), page0);
        assert_eq!(second.page(), page1);
        assert_eq!(first.rect(), AtlasRect::new(0, 0, 4, 3));
        assert_eq!(second.rect(), AtlasRect::new(0, 0, 4, 3));
    }

    #[test]
    fn rect_atlas_set_reuses_freed_space_within_the_same_page() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(8, 8).expect("page id fits");
        let _page1 = pages.add_page(8, 8).expect("page id fits");

        let first = pages.allocate_in(page0, 4, 3).expect("page 0 has room");
        let freed_rect = pages.free(first).expect("live allocation can be freed");
        let reused = pages
            .allocate_in(page0, 4, 3)
            .expect("freed space can be reused");

        assert_eq!(freed_rect, AtlasRect::new(0, 0, 4, 3));
        assert_eq!(reused.page(), page0);
        assert_eq!(reused.rect(), freed_rect);
    }

    #[test]
    fn rect_atlas_set_reports_page_local_and_aggregate_stats() {
        let mut pages = RectAtlasSet::new();
        let page0 = pages.add_page(8, 8).expect("page id fits");
        let page1 = pages.add_page(8, 8).expect("page id fits");

        let first = pages.allocate_in(page0, 4, 3).expect("page 0 has room");
        let _second = pages.allocate_in(page1, 4, 3).expect("page 1 has room");
        let _ = pages.free(first).expect("live allocation can be freed");

        let page0_stats = pages.page_stats(page0).expect("page 0 stats exist");
        let page1_stats = pages.page_stats(page1).expect("page 1 stats exist");
        let total = pages.stats();

        assert_eq!(page0_stats.allocated(), 0);
        assert_eq!(page0_stats.free_regions(), 1);
        assert_eq!(page1_stats.allocated(), 1);
        assert_eq!(page1_stats.free_regions(), 0);
        assert_eq!(total.pages(), 2);
        assert_eq!(total.allocated(), 1);
        assert_eq!(total.free_regions(), 1);
    }
}
