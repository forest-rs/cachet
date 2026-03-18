// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use hashbrown::HashMap;

use crate::ResidencyHandle;

/// Caller-owned metadata keyed by [`ResidencyHandle`].
///
/// This is the calm glue between the residency kernel and workload-specific
/// resolved state such as atlas allocation ids, dense slot indices, or backend
/// resource records.
///
/// Use this when the kernel should stay generic but the caller still needs a
/// standard place to bridge from residency handles to workload-facing metadata.
/// It keeps atlas, tile, or backend-specific nouns out of the core API.
///
/// # Examples
///
/// This example binds one residency handle to a caller-owned dense slot index.
///
/// ```
/// use cachet_residency::{ResidencyBindings, ResidencyHandle};
///
/// let mut bindings = ResidencyBindings::new();
/// let handle = ResidencyHandle::new(7);
///
/// assert_eq!(bindings.bind(handle, 3_u32), None);
/// assert_eq!(bindings.get(handle), Some(&3));
/// assert_eq!(bindings.unbind(handle), Some(3));
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResidencyBindings<V> {
    bindings: HashMap<ResidencyHandle, V>,
}

impl<V> ResidencyBindings<V> {
    /// Creates an empty residency binding table.
    #[must_use]
    pub fn new() -> Self {
        Self {
            bindings: HashMap::new(),
        }
    }

    /// Inserts or replaces the metadata bound to a residency handle.
    pub fn bind(&mut self, handle: ResidencyHandle, value: V) -> Option<V> {
        self.bindings.insert(handle, value)
    }

    /// Returns the metadata currently bound to a residency handle.
    #[must_use]
    pub fn get(&self, handle: ResidencyHandle) -> Option<&V> {
        self.bindings.get(&handle)
    }

    /// Returns mutable metadata currently bound to a residency handle.
    pub fn get_mut(&mut self, handle: ResidencyHandle) -> Option<&mut V> {
        self.bindings.get_mut(&handle)
    }

    /// Removes and returns the metadata bound to a residency handle.
    pub fn unbind(&mut self, handle: ResidencyHandle) -> Option<V> {
        self.bindings.remove(&handle)
    }

    /// Returns whether a handle is currently bound.
    #[must_use]
    pub fn contains(&self, handle: ResidencyHandle) -> bool {
        self.bindings.contains_key(&handle)
    }

    /// Returns the number of currently bound handles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns whether the binding table is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}
