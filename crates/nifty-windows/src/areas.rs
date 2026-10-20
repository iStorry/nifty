//! What a window or scroll area keeps of its content without an `HWND` each: the surfaces
//! it paints behind its controls, and the areas its context menus cover. Each is a rect in
//! the host's content pixels.

use std::cell::{Cell, RefCell};
use std::mem;

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::HDC;

use nifty_runtime::Surface;

use crate::convert::{Scale, rect_size};
use crate::draw::paint_surface;
use crate::palette::Palette;

/// An item and where it is.
#[derive(Clone, Debug)]
struct Placed<T> {
    id: u32,
    item: T,
    rect: RECT,
}

/// Items in the order they were added, each with no area until it is placed.
#[derive(Debug)]
pub(crate) struct Areas<T> {
    placed: RefCell<Vec<Placed<T>>>,
    next_id: Cell<u32>,
}

impl<T> Default for Areas<T> {
    fn default() -> Self {
        Self {
            placed: RefCell::new(Vec::new()),
            next_id: Cell::new(0),
        }
    }
}

/// The surfaces a host paints, in the order they are painted in.
pub(crate) type Surfaces = Areas<Surface>;

impl<T: Clone> Areas<T> {
    /// Adds `item` above the ones already there, with no area until it is placed.
    pub(crate) fn add(&self, item: T) -> u32 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        self.placed.borrow_mut().push(Placed {
            id,
            item,
            rect: RECT::default(),
        });
        id
    }

    /// Moves item `id` to `rect`, returning where it was, or `None` when it hasn't moved.
    pub(crate) fn place(&self, id: u32, rect: RECT) -> Option<RECT> {
        let previous = self.with(id, |placed| mem::replace(&mut placed.rect, rect));
        (previous != rect).then_some(previous)
    }

    /// Removes item `id`, returning the area it covered.
    pub(crate) fn remove(&self, id: u32) -> RECT {
        let removed = self.with(id, |placed| placed.rect);
        self.placed.borrow_mut().retain(|placed| placed.id != id);
        removed
    }

    /// The topmost item at `point`, if any is there.
    pub(crate) fn topmost_at(&self, point: POINT) -> Option<T> {
        self.placed
            .borrow()
            .iter()
            .rev()
            .find(|placed| contains(&placed.rect, point))
            .map(|placed| placed.item.clone())
    }

    /// The item with the smallest area at `point`, which is the innermost stack's; of
    /// areas the same size, the one added last.
    pub(crate) fn innermost_at(&self, point: POINT) -> Option<T> {
        self.placed
            .borrow()
            .iter()
            .rev()
            .filter(|placed| contains(&placed.rect, point))
            .min_by_key(|placed| {
                let (width, height) = rect_size(&placed.rect);
                i64::from(width) * i64::from(height)
            })
            .map(|placed| placed.item.clone())
    }

    fn with<R>(&self, id: u32, change: impl FnOnce(&mut Placed<T>) -> R) -> R {
        let mut placed = self.placed.borrow_mut();
        let item = placed
            .iter_mut()
            .find(|placed| placed.id == id)
            .expect("areas are only used until they are removed");
        change(item)
    }
}

impl Surfaces {
    /// Paints every surface, moved up by `offset` pixels, the distance its host's content
    /// is scrolled.
    pub(crate) fn paint(&self, dc: HDC, offset: i32, palette: &Palette, scale: Scale) {
        for placed in self.placed.borrow().iter() {
            let rect = RECT {
                top: placed.rect.top - offset,
                bottom: placed.rect.bottom - offset,
                ..placed.rect
            };
            paint_surface(dc, rect, placed.item, palette, scale);
        }
    }
}

fn contains(rect: &RECT, point: POINT) -> bool {
    (rect.left..rect.right).contains(&point.x) && (rect.top..rect.bottom).contains(&point.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(left: i32, top: i32, right: i32, bottom: i32) -> RECT {
        RECT {
            left,
            top,
            right,
            bottom,
        }
    }

    #[test]
    fn the_innermost_area_wins_and_ties_go_to_the_last() {
        let areas = Areas::default();
        let outer = areas.add("outer");
        let inner = areas.add("inner");
        let twin = areas.add("twin");
        areas.place(outer, rect(0, 0, 100, 100));
        areas.place(inner, rect(10, 10, 50, 50));
        areas.place(twin, rect(10, 10, 50, 50));
        assert_eq!(areas.innermost_at(POINT { x: 20, y: 20 }), Some("twin"));
        assert_eq!(areas.innermost_at(POINT { x: 80, y: 80 }), Some("outer"));
        assert_eq!(areas.innermost_at(POINT { x: 200, y: 0 }), None);
        areas.remove(twin);
        assert_eq!(areas.innermost_at(POINT { x: 20, y: 20 }), Some("inner"));
        assert_eq!(areas.topmost_at(POINT { x: 20, y: 20 }), Some("inner"));
    }
}
