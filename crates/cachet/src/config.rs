// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use core::fmt;

use crate::Extent;

/// Homogeneous physical configuration for an [`AtlasCache`](crate::AtlasCache).
///
/// Pixel meaning is deliberately caller-owned. Two caches may both use four
/// bytes per texel while remaining incompatible because their color space,
/// alpha convention, sampling, padding, or retention behavior differs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AtlasConfig {
    page_extent: Extent,
    max_pages: u32,
    padding: u16,
}

impl AtlasConfig {
    /// Creates an atlas configuration without artifact padding.
    #[must_use]
    pub const fn new(page_extent: Extent, max_pages: u32) -> Self {
        Self {
            page_extent,
            max_pages,
            padding: 0,
        }
    }

    /// Sets reserved padding on every side of an artifact.
    #[must_use]
    pub const fn with_padding(mut self, padding: u16) -> Self {
        self.padding = padding;
        self
    }

    /// Returns each atlas page's extent.
    #[must_use]
    pub const fn page_extent(self) -> Extent {
        self.page_extent
    }

    /// Returns the maximum number of lazily allocated pages.
    #[must_use]
    pub const fn max_pages(self) -> u32 {
        self.max_pages
    }

    /// Returns reserved padding on every side of an artifact.
    #[must_use]
    pub const fn padding(self) -> u16 {
        self.padding
    }

    pub(crate) const fn validate(self) -> Result<(), ConfigError> {
        if self.page_extent.is_empty() {
            return Err(ConfigError::EmptyPage);
        }
        if self.max_pages == 0 {
            return Err(ConfigError::NoPages);
        }
        Ok(())
    }
}

/// Invalid homogeneous atlas configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// A page dimension is zero.
    EmptyPage,
    /// No physical pages are permitted.
    NoPages,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPage => formatter.write_str("atlas pages must be nonempty"),
            Self::NoPages => formatter.write_str("an atlas cache must permit at least one page"),
        }
    }
}

impl core::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_configs_fail_before_allocating_pages() {
        assert_eq!(
            AtlasConfig::new(Extent::new(0, 8), 1).validate(),
            Err(ConfigError::EmptyPage)
        );
        assert_eq!(
            AtlasConfig::new(Extent::new(8, 8), 0).validate(),
            Err(ConfigError::NoPages)
        );
    }
}
