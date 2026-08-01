// Copyright 2026 the Cachet Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::vec::Vec;

use crate::{Extent, Rect};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct FitScore {
    wasted_area: u32,
    short_side: u16,
    long_side: u16,
    y: u16,
    x: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct RectAllocator {
    extent: Extent,
    free: Vec<Rect>,
    allocated_texels: u64,
}

impl RectAllocator {
    pub(crate) fn new(extent: Extent) -> Self {
        Self {
            extent,
            free: alloc::vec![Rect::new(0, 0, extent.width(), extent.height())],
            allocated_texels: 0,
        }
    }

    pub(crate) fn best_fit(&self, extent: Extent) -> Option<FitScore> {
        self.free
            .iter()
            .copied()
            .filter(|region| fits(*region, extent))
            .map(|region| score(region, extent))
            .min()
    }

    pub(crate) fn allocate(&mut self, extent: Extent) -> Option<Rect> {
        let (index, region) = self
            .free
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, region)| fits(*region, extent))
            .min_by_key(|(_, region)| score(*region, extent))?;
        self.free.swap_remove(index);

        let allocated = Rect::new(region.x(), region.y(), extent.width(), extent.height());
        self.split(region, extent);
        self.allocated_texels = self
            .allocated_texels
            .saturating_add(u64::from(extent.area()));
        Some(allocated)
    }

    pub(crate) fn free(&mut self, rect: Rect) {
        debug_assert!(Rect::new(0, 0, self.extent.width(), self.extent.height()).contains(rect));
        self.allocated_texels = self.allocated_texels.saturating_sub(u64::from(rect.area()));
        self.free.push(rect);
        self.coalesce();
    }

    pub(crate) const fn allocated_texels(&self) -> u64 {
        self.allocated_texels
    }

    pub(crate) fn free_texels(&self) -> u64 {
        self.free.iter().map(|rect| u64::from(rect.area())).sum()
    }

    pub(crate) fn free_regions(&self) -> u32 {
        self.free.len() as u32
    }

    pub(crate) fn largest_free_region(&self) -> Option<Rect> {
        self.free.iter().copied().max_by_key(|rect| rect.area())
    }

    fn split(&mut self, region: Rect, extent: Extent) {
        let remaining_width = region.width() - extent.width();
        let remaining_height = region.height() - extent.height();

        if remaining_width > remaining_height {
            self.push_nonempty(Rect::new(
                region.x() + extent.width(),
                region.y(),
                remaining_width,
                region.height(),
            ));
            self.push_nonempty(Rect::new(
                region.x(),
                region.y() + extent.height(),
                extent.width(),
                remaining_height,
            ));
        } else {
            self.push_nonempty(Rect::new(
                region.x() + extent.width(),
                region.y(),
                remaining_width,
                extent.height(),
            ));
            self.push_nonempty(Rect::new(
                region.x(),
                region.y() + extent.height(),
                region.width(),
                remaining_height,
            ));
        }
    }

    fn push_nonempty(&mut self, rect: Rect) {
        if rect.width() != 0 && rect.height() != 0 {
            self.free.push(rect);
        }
    }

    fn coalesce(&mut self) {
        loop {
            let mut merged = None;
            'search: for left in 0..self.free.len() {
                for right in (left + 1)..self.free.len() {
                    if let Some(rect) = merge(self.free[left], self.free[right]) {
                        merged = Some((left, right, rect));
                        break 'search;
                    }
                }
            }
            let Some((left, right, rect)) = merged else {
                return;
            };
            self.free.swap_remove(right);
            self.free.swap_remove(left);
            self.free.push(rect);
        }
    }
}

fn fits(region: Rect, extent: Extent) -> bool {
    extent.width() <= region.width() && extent.height() <= region.height()
}

fn score(region: Rect, extent: Extent) -> FitScore {
    let remaining_width = region.width() - extent.width();
    let remaining_height = region.height() - extent.height();
    FitScore {
        wasted_area: region.area() - extent.area(),
        short_side: remaining_width.min(remaining_height),
        long_side: remaining_width.max(remaining_height),
        y: region.y(),
        x: region.x(),
    }
}

fn merge(left: Rect, right: Rect) -> Option<Rect> {
    if left.y() == right.y() && left.height() == right.height() {
        if left.right() == right.x() {
            return Some(Rect::new(
                left.x(),
                left.y(),
                left.width() + right.width(),
                left.height(),
            ));
        }
        if right.right() == left.x() {
            return Some(Rect::new(
                right.x(),
                right.y(),
                right.width() + left.width(),
                right.height(),
            ));
        }
    }
    if left.x() == right.x() && left.width() == right.width() {
        if left.bottom() == right.y() {
            return Some(Rect::new(
                left.x(),
                left.y(),
                left.width(),
                left.height() + right.height(),
            ));
        }
        if right.bottom() == left.y() {
            return Some(Rect::new(
                right.x(),
                right.y(),
                right.width(),
                right.height() + left.height(),
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_allocations_preserve_area_without_overlap() {
        let mut allocator = RectAllocator::new(Extent::new(10, 8));
        let first = allocator.allocate(Extent::new(4, 3)).expect("first fits");
        let second = allocator.allocate(Extent::new(6, 2)).expect("second fits");
        let third = allocator.allocate(Extent::new(3, 4)).expect("third fits");

        assert_eq!(allocator.allocated_texels(), 36);
        assert_eq!(allocator.free_texels(), 44);
        assert!(!overlaps(first, second));
        assert!(!overlaps(first, third));
        assert!(!overlaps(second, third));
    }

    #[test]
    fn freeing_every_allocation_recovers_one_full_page() {
        let mut allocator = RectAllocator::new(Extent::new(8, 8));
        let first = allocator.allocate(Extent::new(4, 4)).expect("first fits");
        let second = allocator.allocate(Extent::new(4, 4)).expect("second fits");
        let third = allocator.allocate(Extent::new(8, 4)).expect("third fits");

        allocator.free(second);
        allocator.free(first);
        allocator.free(third);

        assert_eq!(allocator.allocated_texels(), 0);
        assert_eq!(allocator.free_regions(), 1);
        assert_eq!(allocator.largest_free_region(), Some(Rect::new(0, 0, 8, 8)));
        assert_eq!(
            allocator.allocate(Extent::new(8, 8)),
            Some(Rect::new(0, 0, 8, 8))
        );
    }

    fn overlaps(left: Rect, right: Rect) -> bool {
        left.x() < right.right()
            && right.x() < left.right()
            && left.y() < right.bottom()
            && right.y() < left.bottom()
    }
}
