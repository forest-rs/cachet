// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{vec, vec::Vec};
use core::hash::Hash;

use crate::{
    AbortError, ArtifactRef, AtlasCache, AtlasConfig, CacheMetrics, CacheStats, CpuConfigError,
    EntryId, Evicted, Extent, Invalidated, Lease, LeaseError, PageData, PageId, PageStats,
    PageUpload, Placement, PopulateError, Publication, PublishError, Rect, Reservation,
    ReserveError, UploadAcknowledgement, UploadCheckpoint, UploadRegion,
};

/// Homogeneous configuration for a CPU-backed atlas.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuAtlasConfig {
    atlas: AtlasConfig,
    texel_bytes: u8,
}

impl CpuAtlasConfig {
    /// Combines placement configuration with one CPU texel layout.
    #[must_use]
    pub const fn new(atlas: AtlasConfig, texel_bytes: u8) -> Self {
        Self { atlas, texel_bytes }
    }

    /// Returns the backing-independent placement configuration.
    #[must_use]
    pub const fn atlas(self) -> AtlasConfig {
        self.atlas
    }

    /// Returns the number of bytes in one homogeneous texel.
    #[must_use]
    pub const fn texel_bytes(self) -> u8 {
        self.texel_bytes
    }

    fn page_byte_len(self) -> Result<usize, CpuConfigError> {
        self.atlas.validate()?;
        if self.texel_bytes == 0 {
            return Err(CpuConfigError::ZeroTexelBytes);
        }
        usize::from(self.atlas.page_extent().width())
            .checked_mul(usize::from(self.atlas.page_extent().height()))
            .and_then(|texels| texels.checked_mul(usize::from(self.texel_bytes)))
            .ok_or(CpuConfigError::PageBytesOverflow)
    }
}

/// Borrowed raster bytes used to populate a CPU-backed reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Raster<'a> {
    extent: Extent,
    bytes_per_row: usize,
    bytes: &'a [u8],
}

impl<'a> Raster<'a> {
    /// Describes tightly or loosely row-packed raster bytes.
    #[must_use]
    pub const fn new(extent: Extent, bytes_per_row: usize, bytes: &'a [u8]) -> Self {
        Self {
            extent,
            bytes_per_row,
            bytes,
        }
    }

    /// Returns the raster extent.
    #[must_use]
    pub const fn extent(self) -> Extent {
        self.extent
    }

    /// Returns the byte distance between source rows.
    #[must_use]
    pub const fn bytes_per_row(self) -> usize {
        self.bytes_per_row
    }

    /// Returns source bytes.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Cumulative CPU population and upload counters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CpuMetrics {
    populations: u64,
    raster_bytes: u64,
    upload_regions_acknowledged: u64,
    upload_texels_acknowledged: u64,
}

impl CpuMetrics {
    /// Returns successful CPU raster populations.
    #[must_use]
    pub const fn populations(self) -> u64 {
        self.populations
    }

    /// Returns source raster bytes copied, excluding generated padding.
    #[must_use]
    pub const fn raster_bytes(self) -> u64 {
        self.raster_bytes
    }

    /// Returns dirty records cleared by upload acknowledgement.
    #[must_use]
    pub const fn upload_regions_acknowledged(self) -> u64 {
        self.upload_regions_acknowledged
    }

    /// Returns dirty texel work cleared by upload acknowledgement.
    #[must_use]
    pub const fn upload_texels_acknowledged(self) -> u64 {
        self.upload_texels_acknowledged
    }
}

/// Current placement and CPU-backing gauges for one page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuPageStats {
    atlas: PageStats,
    bytes: u64,
    dirty_regions: u32,
    dirty_texels: u64,
}

impl CpuPageStats {
    /// Returns backing-independent packing and lifecycle gauges.
    #[must_use]
    pub const fn atlas(self) -> PageStats {
        self.atlas
    }

    /// Returns CPU bytes retained by this page.
    #[must_use]
    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    /// Returns dirty records awaiting acknowledgement.
    #[must_use]
    pub const fn dirty_regions(self) -> u32 {
        self.dirty_regions
    }

    /// Returns dirty texel work awaiting acknowledgement.
    #[must_use]
    pub const fn dirty_texels(self) -> u64 {
        self.dirty_texels
    }
}

/// Current aggregate placement and CPU-backing gauges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CpuStats {
    atlas: CacheStats,
    page_bytes: u64,
    dirty_regions: u32,
    dirty_texels: u64,
    metrics: CpuMetrics,
}

impl CpuStats {
    /// Returns backing-independent lifecycle and packing gauges.
    #[must_use]
    pub const fn atlas(self) -> CacheStats {
        self.atlas
    }

    /// Returns total CPU page bytes.
    #[must_use]
    pub const fn page_bytes(self) -> u64 {
        self.page_bytes
    }

    /// Returns dirty records awaiting acknowledgement.
    #[must_use]
    pub const fn dirty_regions(self) -> u32 {
        self.dirty_regions
    }

    /// Returns dirty texel work awaiting acknowledgement.
    #[must_use]
    pub const fn dirty_texels(self) -> u64 {
        self.dirty_texels
    }

    /// Returns cumulative CPU-specific work counters.
    #[must_use]
    pub const fn metrics(self) -> CpuMetrics {
        self.metrics
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirtyRecord {
    sequence: u64,
    rect: Rect,
}

#[derive(Debug)]
struct CpuPage {
    bytes: Vec<u8>,
    dirty: Vec<DirtyRecord>,
}

/// CPU-backed raster-artifact cache built on [`AtlasCache`].
///
/// Population validates and copies complete rasters into zero-padded CPU page
/// mirrors before publishing them. Consumers can collect dirty rectangles,
/// borrow upload views, and acknowledge only the work represented by a
/// checkpoint. GPU objects and upload execution remain caller-owned.
#[derive(Debug)]
pub struct CpuAtlasCache<K, M = ()> {
    config: CpuAtlasConfig,
    page_byte_len: usize,
    atlas: AtlasCache<K, M>,
    pages: Vec<CpuPage>,
    upload_sequence: u64,
    metrics: CpuMetrics,
}

impl<K, M> CpuAtlasCache<K, M>
where
    K: Clone + Eq + Hash,
{
    /// Creates an empty CPU-backed cache without allocating any pages.
    pub fn new(config: CpuAtlasConfig) -> Result<Self, CpuConfigError> {
        let page_byte_len = config.page_byte_len()?;
        let atlas = AtlasCache::new(config.atlas())?;
        Ok(Self {
            config,
            page_byte_len,
            atlas,
            pages: Vec::new(),
            upload_sequence: 0,
            metrics: CpuMetrics::default(),
        })
    }

    /// Returns the homogeneous CPU and placement configuration.
    #[must_use]
    pub const fn config(&self) -> CpuAtlasConfig {
        self.config
    }

    /// Returns the number of lazily allocated stable pages.
    #[must_use]
    pub fn page_count(&self) -> u32 {
        self.atlas.page_count()
    }

    /// Returns whether an identifier currently names a populated entry.
    #[must_use]
    pub fn is_ready(&self, entry: EntryId) -> bool {
        self.atlas.is_ready(entry)
    }

    /// Looks up and touches a populated artifact.
    pub fn lookup(&mut self, key: &K) -> Option<ArtifactRef<'_, M>> {
        self.atlas.lookup(key)
    }

    /// Finds or allocates CPU-backed placement for a caller key.
    pub fn reserve(
        &mut self,
        key: K,
        extent: Extent,
        evicted: &mut Vec<Evicted<K, M>>,
    ) -> Result<Reservation, ReserveError> {
        let reservation = self.atlas.reserve(key, extent, evicted)?;
        self.sync_pages();
        Ok(reservation)
    }

    /// Copies a complete raster into a vacant reservation and publishes it.
    ///
    /// The copy is validated before page bytes or dirty state change. Padding
    /// in the complete allocation is cleared to zero. Retain the returned
    /// publication lease until uploads and prepared readers permit reuse.
    pub fn populate(
        &mut self,
        id: EntryId,
        raster: Raster<'_>,
        metadata: M,
    ) -> Result<Publication, PopulateError> {
        let placement = self
            .atlas
            .reserved_placement(id)
            .map_err(map_publish_error)?;
        let reserved = placement.content().extent();
        if raster.extent() != reserved {
            return Err(PopulateError::ExtentMismatch {
                reserved,
                raster: raster.extent(),
            });
        }

        let texel_bytes = usize::from(self.config.texel_bytes());
        let source_row_bytes = usize::from(reserved.width())
            .checked_mul(texel_bytes)
            .ok_or(PopulateError::RasterLayoutOverflow)?;
        if raster.bytes_per_row() < source_row_bytes {
            return Err(PopulateError::StrideTooSmall);
        }
        let required = usize::from(reserved.height() - 1)
            .checked_mul(raster.bytes_per_row())
            .and_then(|rows| rows.checked_add(source_row_bytes))
            .ok_or(PopulateError::RasterLayoutOverflow)?;
        if raster.bytes().len() < required {
            return Err(PopulateError::DataTooShort);
        }
        let sequence = self
            .upload_sequence
            .checked_add(1)
            .ok_or(PopulateError::UploadSequenceExhausted)?;

        self.write_raster(placement, raster, source_row_bytes);
        let publication = self
            .atlas
            .publish(id, metadata)
            .map_err(map_publish_error)?;
        self.upload_sequence = sequence;
        self.pages[placement.page().index() as usize]
            .dirty
            .push(DirtyRecord {
                sequence,
                rect: placement.allocation(),
            });
        self.metrics.populations = self.metrics.populations.saturating_add(1);
        let copied = u64::from(reserved.height())
            .saturating_mul(u64::try_from(source_row_bytes).unwrap_or(u64::MAX));
        self.metrics.raster_bytes = self.metrics.raster_bytes.saturating_add(copied);
        Ok(publication)
    }

    /// Aborts a vacant reservation and returns its key.
    pub fn abort(&mut self, id: EntryId) -> Result<K, AbortError> {
        self.atlas.abort(id)
    }

    /// Removes a key immediately and defers physical reuse when it is leased.
    pub fn invalidate(&mut self, key: &K) -> Option<Invalidated<K, M>> {
        self.atlas.invalidate(key)
    }

    /// Pins every distinct ready entry until the returned lease is dropped.
    pub fn lease(
        &mut self,
        entries: impl IntoIterator<Item = EntryId>,
    ) -> Result<Lease, LeaseError> {
        self.atlas.lease(entries)
    }

    /// Reclaims retired placements whose final external lease has dropped.
    pub fn reclaim(&mut self) -> u32 {
        self.atlas.reclaim()
    }

    /// Returns backing-independent cumulative lifecycle counters.
    #[must_use]
    pub const fn atlas_metrics(&self) -> CacheMetrics {
        self.atlas.metrics()
    }

    /// Returns cumulative CPU population and upload counters.
    #[must_use]
    pub const fn metrics(&self) -> CpuMetrics {
        self.metrics
    }

    /// Returns aggregate placement and CPU-backing gauges.
    #[must_use]
    pub fn stats(&self) -> CpuStats {
        let mut dirty_regions = 0_u32;
        let mut dirty_texels = 0_u64;
        for page in &self.pages {
            dirty_regions = dirty_regions.saturating_add(page.dirty.len() as u32);
            dirty_texels = dirty_texels.saturating_add(
                page.dirty
                    .iter()
                    .map(|dirty| u64::from(dirty.rect.area()))
                    .sum::<u64>(),
            );
        }
        CpuStats {
            atlas: self.atlas.stats(),
            page_bytes: (self.pages.len() as u64).saturating_mul(self.page_byte_len as u64),
            dirty_regions,
            dirty_texels,
            metrics: self.metrics,
        }
    }

    /// Returns placement and CPU-backing gauges for one page.
    #[must_use]
    pub fn page_stats(&self, page: PageId) -> Option<CpuPageStats> {
        let atlas = self.atlas.page_stats(page)?;
        let page = self.pages.get(page.index() as usize)?;
        Some(CpuPageStats {
            atlas,
            bytes: self.page_byte_len as u64,
            dirty_regions: page.dirty.len() as u32,
            dirty_texels: page
                .dirty
                .iter()
                .map(|dirty| u64::from(dirty.rect.area()))
                .sum(),
        })
    }

    /// Borrows the complete CPU mirror for one stable page.
    #[must_use]
    pub fn page_data(&self, page: PageId) -> Option<PageData<'_>> {
        let page_data = self.pages.get(page.index() as usize)?;
        Some(PageData::new(
            page,
            self.config.atlas().page_extent(),
            self.config.texel_bytes(),
            self.page_bytes_per_row(),
            &page_data.bytes,
        ))
    }

    /// Collects all currently dirty rectangles and returns their checkpoint.
    ///
    /// `out` is cleared before collection. Writes performed after this call
    /// receive later sequence numbers and survive acknowledgement of the
    /// returned checkpoint.
    pub fn collect_uploads(&self, out: &mut Vec<UploadRegion>) -> UploadCheckpoint {
        out.clear();
        for (index, page) in self.pages.iter().enumerate() {
            let page_id = PageId::new(index as u32);
            out.extend(
                page.dirty
                    .iter()
                    .map(|dirty| UploadRegion::new(page_id, dirty.rect)),
            );
        }
        UploadCheckpoint::new(self.upload_sequence)
    }

    /// Borrows bytes for one collected dirty rectangle.
    ///
    /// Rows retain the complete page stride. The byte slice begins at the
    /// rectangle's top-left texel and ends after its final texel.
    #[must_use]
    pub fn page_upload(&self, region: UploadRegion) -> Option<PageUpload<'_>> {
        let page = self.pages.get(region.page().index() as usize)?;
        let page_rect = Rect::new(
            0,
            0,
            self.config.atlas().page_extent().width(),
            self.config.atlas().page_extent().height(),
        );
        let rect = region.rect();
        if rect.width() == 0 || rect.height() == 0 || !page_rect.contains(rect) {
            return None;
        }
        let texel_bytes = usize::from(self.config.texel_bytes());
        let stride = self.page_bytes_per_row();
        let start = usize::from(rect.y())
            .checked_mul(stride)?
            .checked_add(usize::from(rect.x()).checked_mul(texel_bytes)?)?;
        let end = start
            .checked_add(usize::from(rect.height() - 1).checked_mul(stride)?)?
            .checked_add(usize::from(rect.width()).checked_mul(texel_bytes)?)?;
        Some(PageUpload::new(
            region,
            self.config.texel_bytes(),
            stride,
            page.bytes.get(start..end)?,
        ))
    }

    /// Acknowledges dirty records at or before a collected checkpoint.
    pub fn acknowledge_uploads(&mut self, checkpoint: UploadCheckpoint) -> UploadAcknowledgement {
        let mut regions = 0_u32;
        let mut texels = 0_u64;
        for page in &mut self.pages {
            page.dirty.retain(|dirty| {
                if dirty.sequence <= checkpoint.sequence() {
                    regions = regions.saturating_add(1);
                    texels = texels.saturating_add(u64::from(dirty.rect.area()));
                    false
                } else {
                    true
                }
            });
        }
        self.metrics.upload_regions_acknowledged = self
            .metrics
            .upload_regions_acknowledged
            .saturating_add(u64::from(regions));
        self.metrics.upload_texels_acknowledged = self
            .metrics
            .upload_texels_acknowledged
            .saturating_add(texels);
        UploadAcknowledgement::new(regions, texels)
    }

    fn sync_pages(&mut self) {
        while self.pages.len() < self.atlas.page_count() as usize {
            self.pages.push(CpuPage {
                bytes: vec![0; self.page_byte_len],
                dirty: Vec::new(),
            });
        }
    }

    fn page_bytes_per_row(&self) -> usize {
        usize::from(self.config.atlas().page_extent().width())
            * usize::from(self.config.texel_bytes())
    }

    fn write_raster(&mut self, placement: Placement, raster: Raster<'_>, source_row_bytes: usize) {
        let stride = self.page_bytes_per_row();
        let texel_bytes = usize::from(self.config.texel_bytes());
        let page = &mut self.pages[placement.page().index() as usize].bytes;
        let allocation = placement.allocation();
        let allocation_row_bytes = usize::from(allocation.width()) * texel_bytes;
        for row in 0..usize::from(allocation.height()) {
            let start = (usize::from(allocation.y()) + row) * stride
                + usize::from(allocation.x()) * texel_bytes;
            page[start..start + allocation_row_bytes].fill(0);
        }
        let content = placement.content();
        for row in 0..usize::from(content.height()) {
            let source_start = row * raster.bytes_per_row();
            let destination_start =
                (usize::from(content.y()) + row) * stride + usize::from(content.x()) * texel_bytes;
            page[destination_start..destination_start + source_row_bytes]
                .copy_from_slice(&raster.bytes()[source_start..source_start + source_row_bytes]);
        }
    }
}

fn map_publish_error(error: PublishError) -> PopulateError {
    match error {
        PublishError::InvalidEntry(id) => PopulateError::InvalidEntry(id),
        PublishError::NotReserved(id) => PopulateError::NotReserved(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReservationStatus;

    fn cache(extent: Extent, texel_bytes: u8) -> CpuAtlasCache<&'static str, u32> {
        CpuAtlasCache::new(CpuAtlasConfig::new(
            AtlasConfig::new(extent, 1),
            texel_bytes,
        ))
        .expect("valid CPU cache")
    }

    fn reserve(
        cache: &mut CpuAtlasCache<&'static str, u32>,
        key: &'static str,
        extent: Extent,
    ) -> Reservation {
        let reservation = cache
            .reserve(key, extent, &mut Vec::new())
            .expect("reservation");
        assert_eq!(reservation.status(), ReservationStatus::Vacant);
        reservation
    }

    #[test]
    fn r8_population_copies_rows_and_zeroes_padding() {
        let mut cache = CpuAtlasCache::<&str, u32>::new(CpuAtlasConfig::new(
            AtlasConfig::new(Extent::new(4, 4), 1).with_padding(1),
            1,
        ))
        .expect("valid cache");
        let reserved = reserve(&mut cache, "coverage", Extent::new(2, 2));
        let raster = Raster::new(Extent::new(2, 2), 3, &[1, 2, 99, 3, 4]);
        let publication = cache
            .populate(reserved.entry(), raster, 7)
            .expect("population");

        let page = cache
            .page_data(publication.placement().page())
            .expect("page data");
        assert_eq!(page.bytes_per_row(), 4);
        assert_eq!(
            page.bytes(),
            &[0, 0, 0, 0, 0, 1, 2, 0, 0, 3, 4, 0, 0, 0, 0, 0]
        );
        assert_eq!(cache.metrics().raster_bytes(), 4);
        assert_eq!(cache.stats().dirty_texels(), 16);
    }

    #[test]
    fn rgba_population_honors_source_stride() {
        let mut cache = cache(Extent::new(2, 2), 4);
        let reserved = reserve(&mut cache, "color", Extent::new(2, 2));
        let bytes = [
            1, 2, 3, 4, 5, 6, 7, 8, 90, 91, 92, 93, 9, 10, 11, 12, 13, 14, 15, 16,
        ];
        drop(
            cache
                .populate(
                    reserved.entry(),
                    Raster::new(Extent::new(2, 2), 12, &bytes),
                    8,
                )
                .expect("population"),
        );
        assert_eq!(
            cache.page_data(PageId::new(0)).expect("page").bytes(),
            &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16]
        );
    }

    #[test]
    fn invalid_rasters_leave_bytes_and_dirty_state_untouched() {
        let mut cache = cache(Extent::new(4, 4), 1);
        let reserved = reserve(&mut cache, "bad", Extent::new(2, 2));
        assert!(matches!(
            cache.populate(
                reserved.entry(),
                Raster::new(Extent::new(1, 2), 1, &[1, 2]),
                0,
            ),
            Err(PopulateError::ExtentMismatch { .. })
        ));
        assert!(matches!(
            cache.populate(
                reserved.entry(),
                Raster::new(Extent::new(2, 2), 1, &[1, 2]),
                0,
            ),
            Err(PopulateError::StrideTooSmall)
        ));
        assert!(matches!(
            cache.populate(
                reserved.entry(),
                Raster::new(Extent::new(2, 2), 2, &[1, 2, 3]),
                0,
            ),
            Err(PopulateError::DataTooShort)
        ));
        assert!(
            cache
                .page_data(PageId::new(0))
                .expect("page")
                .bytes()
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(cache.stats().dirty_regions(), 0);
        assert!(!cache.is_ready(reserved.entry()));
    }

    #[test]
    fn upload_checkpoints_preserve_later_writes() {
        let mut cache = cache(Extent::new(4, 2), 1);
        let first = reserve(&mut cache, "a", Extent::new(2, 2));
        drop(
            cache
                .populate(
                    first.entry(),
                    Raster::new(Extent::new(2, 2), 2, &[1, 2, 3, 4]),
                    1,
                )
                .expect("first population"),
        );
        let mut uploads = Vec::new();
        let checkpoint = cache.collect_uploads(&mut uploads);
        assert_eq!(uploads.len(), 1);
        let upload = cache.page_upload(uploads[0]).expect("upload view");
        assert_eq!(upload.bytes_per_row(), 4);
        assert_eq!(upload.bytes(), &[1, 2, 0, 0, 3, 4]);

        let second = reserve(&mut cache, "b", Extent::new(2, 2));
        drop(
            cache
                .populate(
                    second.entry(),
                    Raster::new(Extent::new(2, 2), 2, &[5, 6, 7, 8]),
                    2,
                )
                .expect("second population"),
        );
        let acknowledged = cache.acknowledge_uploads(checkpoint);
        assert_eq!(acknowledged.regions(), 1);
        assert_eq!(acknowledged.texels(), 4);
        assert_eq!(cache.stats().dirty_regions(), 1);
        assert_eq!(cache.metrics().upload_regions_acknowledged(), 1);
    }

    #[test]
    fn publication_lease_is_preserved_by_cpu_facade() {
        let mut cache = cache(Extent::new(2, 2), 1);
        let reserved = reserve(&mut cache, "a", Extent::new(2, 2));
        let publication = cache
            .populate(
                reserved.entry(),
                Raster::new(Extent::new(2, 2), 2, &[1, 2, 3, 4]),
                1,
            )
            .expect("population");
        assert!(cache.invalidate(&"a").expect("entry").reuse_deferred());
        assert_eq!(cache.reclaim(), 0);
        drop(publication);
        assert_eq!(cache.reclaim(), 1);
    }
}
