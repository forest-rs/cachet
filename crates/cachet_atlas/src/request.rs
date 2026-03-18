// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use cachet_residency::{Priority, Request};

use crate::{ArtifactSize, AtlasClass};

/// Atlas-facing request wrapper passed into the residency layer.
///
/// # Examples
///
/// This example wraps one logical emoji key with the extra atlas metadata that
/// `cachet_residency::Request` does not know about on its own.
///
/// ```
/// use cachet_atlas::{ArtifactRequest, ArtifactSize, AtlasClass};
/// use cachet_residency::Priority;
///
/// let request = ArtifactRequest::new(
///     "emoji:wave",
///     AtlasClass::new(7),
///     ArtifactSize::new(24, 24),
///     Priority::new(0, 90),
///     3,
/// );
///
/// assert_eq!(*request.request().key(), "emoji:wave");
/// assert_eq!(request.class().get(), 7);
/// assert_eq!(request.size().width(), 24);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactRequest<K> {
    request: Request<K>,
    class: AtlasClass,
    size: ArtifactSize,
}

impl<K> ArtifactRequest<K> {
    /// Creates an atlas request that embeds a generic residency request.
    #[must_use]
    pub const fn new(
        key: K,
        class: AtlasClass,
        size: ArtifactSize,
        priority: Priority,
        generation: u64,
    ) -> Self {
        Self {
            request: Request::new(key, priority, generation),
            class,
            size,
        }
    }

    /// Returns the inner residency request to pass to `cachet_residency`.
    #[must_use]
    pub const fn request(&self) -> &Request<K> {
        &self.request
    }

    /// Returns the atlas class attached to this request.
    #[must_use]
    pub const fn class(&self) -> AtlasClass {
        self.class
    }

    /// Returns the requested atlas-space size for this artifact.
    #[must_use]
    pub const fn size(&self) -> ArtifactSize {
        self.size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_request_preserves_metadata() {
        let request = ArtifactRequest::new(
            7_u32,
            AtlasClass::new(2),
            ArtifactSize::new(16, 24),
            Priority::new(1, 9),
            3,
        );

        assert_eq!(*request.request().key(), 7_u32);
        assert_eq!(request.class().get(), 2);
        assert_eq!(request.size().width(), 16);
        assert_eq!(request.size().height(), 24);
    }
}
