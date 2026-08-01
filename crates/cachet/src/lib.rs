// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

#![no_std]
//! A bounded raster-artifact atlas for glyphs, sprites, icons, and image
//! patches.
//!
//! Cachet owns packed CPU atlas pages and their cache lifecycle. Rasterization,
//! pixel interpretation, GPU objects, uploads, and renderer lifetime remain
//! caller-owned.

extern crate alloc;
