// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Compatibility class passed to [`ArtifactRequest::new`](crate::ArtifactRequest::new)
/// and returned by [`ResolvedArtifact::class`](crate::ResolvedArtifact::class).
///
/// A class groups artifacts that can share the same atlas family or packing
/// rules, for example a page format, sampling regime, padding policy, or
/// backend compatibility bucket. It is not itself a concrete atlas page id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AtlasClass(u16);

impl AtlasClass {
    /// Creates an atlas compatibility class.
    #[must_use]
    pub const fn new(raw: u16) -> Self {
        Self(raw)
    }

    /// Returns the raw class identifier.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

/// Pixel size passed to [`ArtifactRequest::new`](crate::ArtifactRequest::new).
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactSize {
    width: u16,
    height: u16,
}

impl ArtifactSize {
    /// Creates an artifact size.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Returns the artifact width in pixels.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Returns the artifact height in pixels.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }
}
