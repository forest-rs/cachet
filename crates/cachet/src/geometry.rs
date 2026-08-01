// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// Two-dimensional extent in atlas texels.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Extent {
    width: u16,
    height: u16,
}

impl Extent {
    /// Creates an extent from its width and height.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// Returns the width in texels.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Returns the height in texels.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }

    /// Returns whether either dimension is zero.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Returns the area in texels.
    #[must_use]
    pub const fn area(self) -> u32 {
        (self.width as u32) * (self.height as u32)
    }
}

/// Axis-aligned rectangle in atlas texels.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Rect {
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

impl Rect {
    /// Creates a rectangle from its top-left origin and dimensions.
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

    /// Returns the width.
    #[must_use]
    pub const fn width(self) -> u16 {
        self.width
    }

    /// Returns the height.
    #[must_use]
    pub const fn height(self) -> u16 {
        self.height
    }

    /// Returns the rectangle extent.
    #[must_use]
    pub const fn extent(self) -> Extent {
        Extent::new(self.width, self.height)
    }

    /// Returns the area in texels.
    #[must_use]
    pub const fn area(self) -> u32 {
        self.extent().area()
    }

    pub(crate) const fn right(self) -> u16 {
        self.x + self.width
    }

    pub(crate) const fn bottom(self) -> u16 {
        self.y + self.height
    }

    pub(crate) const fn contains(self, other: Self) -> bool {
        other.x >= self.x
            && other.y >= self.y
            && other.right() <= self.right()
            && other.bottom() <= self.bottom()
    }
}
