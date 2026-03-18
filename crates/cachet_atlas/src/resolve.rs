// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use cachet_storage::{AtlasPageId, AtlasRect, PagedAtlasSlot};

use crate::AtlasClass;

/// Atlas resolve record returned to a discrete artifact workload.
///
/// This is atlas-facing resolve metadata: the caller-facing answer produced
/// after logical residency has been admitted and storage has assigned a
/// physical atlas slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedArtifact<K> {
    key: K,
    class: AtlasClass,
    slot: PagedAtlasSlot,
}

impl<K> ResolvedArtifact<K> {
    /// Creates a resolved atlas record for a resident artifact.
    #[must_use]
    pub const fn new(key: K, class: AtlasClass, slot: PagedAtlasSlot) -> Self {
        Self { key, class, slot }
    }

    /// Returns the logical artifact key that this record resolves.
    #[must_use]
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Returns the atlas class carried through from the request.
    #[must_use]
    pub const fn class(&self) -> AtlasClass {
        self.class
    }

    /// Returns the atlas page chosen for this artifact.
    #[must_use]
    pub const fn page(&self) -> AtlasPageId {
        self.slot.page()
    }

    /// Returns the atlas slot assigned by `cachet_storage`.
    #[must_use]
    pub const fn slot(&self) -> PagedAtlasSlot {
        self.slot
    }

    /// Returns the atlas rectangle assigned to the artifact.
    #[must_use]
    pub const fn rect(&self) -> AtlasRect {
        self.slot.rect()
    }
}
