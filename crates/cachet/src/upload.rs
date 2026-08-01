// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::{Extent, PageId, Rect};

/// Cache-local checkpoint captured while collecting dirty regions.
///
/// Acknowledgement clears only records at or before this checkpoint. Writes
/// performed afterward remain pending.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UploadCheckpoint(pub(crate) u64);

impl UploadCheckpoint {
    pub(crate) const fn new(sequence: u64) -> Self {
        Self(sequence)
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.0
    }
}

/// One dirty page rectangle that should be uploaded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UploadRegion {
    page: PageId,
    rect: Rect,
}

impl UploadRegion {
    pub(crate) const fn new(page: PageId, rect: Rect) -> Self {
        Self { page, rect }
    }

    /// Returns the page to update.
    #[must_use]
    pub const fn page(self) -> PageId {
        self.page
    }

    /// Returns the dirty rectangle.
    #[must_use]
    pub const fn rect(self) -> Rect {
        self.rect
    }
}

/// Borrowed CPU mirror for one complete atlas page.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageData<'a> {
    page: PageId,
    extent: Extent,
    texel_bytes: u8,
    bytes_per_row: usize,
    bytes: &'a [u8],
}

impl<'a> PageData<'a> {
    pub(crate) const fn new(
        page: PageId,
        extent: Extent,
        texel_bytes: u8,
        bytes_per_row: usize,
        bytes: &'a [u8],
    ) -> Self {
        Self {
            page,
            extent,
            texel_bytes,
            bytes_per_row,
            bytes,
        }
    }

    /// Returns the page identifier.
    #[must_use]
    pub const fn page(self) -> PageId {
        self.page
    }

    /// Returns the page extent.
    #[must_use]
    pub const fn extent(self) -> Extent {
        self.extent
    }

    /// Returns bytes in one texel.
    #[must_use]
    pub const fn texel_bytes(self) -> u8 {
        self.texel_bytes
    }

    /// Returns the complete page row stride.
    #[must_use]
    pub const fn bytes_per_row(self) -> usize {
        self.bytes_per_row
    }

    /// Returns every byte in the CPU page mirror.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Borrowed bytes and layout for one dirty page rectangle.
///
/// The first byte is the dirty rectangle's top-left texel. Rows retain the
/// complete page stride; the final row ends after the rectangle's last texel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PageUpload<'a> {
    region: UploadRegion,
    texel_bytes: u8,
    bytes_per_row: usize,
    bytes: &'a [u8],
}

impl<'a> PageUpload<'a> {
    pub(crate) const fn new(
        region: UploadRegion,
        texel_bytes: u8,
        bytes_per_row: usize,
        bytes: &'a [u8],
    ) -> Self {
        Self {
            region,
            texel_bytes,
            bytes_per_row,
            bytes,
        }
    }

    /// Returns the described page region.
    #[must_use]
    pub const fn region(self) -> UploadRegion {
        self.region
    }

    /// Returns bytes in one texel.
    #[must_use]
    pub const fn texel_bytes(self) -> u8 {
        self.texel_bytes
    }

    /// Returns the byte distance between rows.
    #[must_use]
    pub const fn bytes_per_row(self) -> usize {
        self.bytes_per_row
    }

    /// Returns bytes beginning at the dirty rectangle's top-left texel.
    #[must_use]
    pub const fn bytes(self) -> &'a [u8] {
        self.bytes
    }
}

/// Work cleared by upload acknowledgement.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UploadAcknowledgement {
    regions: u32,
    texels: u64,
}

impl UploadAcknowledgement {
    pub(crate) const fn new(regions: u32, texels: u64) -> Self {
        Self { regions, texels }
    }

    /// Returns dirty records cleared.
    #[must_use]
    pub const fn regions(self) -> u32 {
        self.regions
    }

    /// Returns dirty texel work cleared.
    #[must_use]
    pub const fn texels(self) -> u64 {
        self.texels
    }
}
