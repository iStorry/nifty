//! The surfaces a window or scroll area paints behind its controls. They cost no `HWND`:
//! each is a rect its host fills when it erases its background.

use std::cell::{Cell, RefCell};
use std::mem;

use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::Graphics::Gdi::HDC;

use nifty_runtime::Surface;

use crate::convert::Scale;
use crate::draw::paint_surface;
use crate::palette::Palette;

/// A surface and where it is, in its host's content pixels.
#[derive(Clone, Copy, Debug)]
struct PaintedSurface {
    id: u32,
    surface: Surface,
    rect: RECT,
}

/// In the order they were created, which is the order they are painted in.
#[derive(Debug, Default)]
pub(crate) struct Surfaces {
    painted: RefCell<Vec<PaintedSurface>>,
    next_id: Cell<u32>,
}

impl Surfaces {
    /// Adds `surface` above the ones already painted, with no area until it is placed.
    pub(crate) fn add(&self, surface: Surface) -> u32 {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1));
        self.painted.borrow_mut().push(PaintedSurface {
            id,
            surface,
            rect: RECT::default(),
        });
        id
    }

    /// Moves surface `id` to `rect`, returning where it was, or `None` when it hasn't moved.
    pub(crate) fn place(&self, id: u32, rect: RECT) -> Option<RECT> {
        let previous = self.with(id, |painted| mem::replace(&mut painted.rect, rect));
        (previous != rect).then_some(previous)
    }

    /// Removes surface `id`, returning the area it covered.
    pub(crate) fn remove(&self, id: u32) -> RECT {
        let removed = self.with(id, |painted| painted.rect);
        self.painted.borrow_mut().retain(|painted| painted.id != id);
        removed
    }

    /// The topmost surface at `point`, if any is there.
    pub(crate) fn at(&self, point: POINT) -> Option<Surface> {
        self.painted
            .borrow()
            .iter()
            .rev()
            .find(|painted| contains(&painted.rect, point))
            .map(|painted| painted.surface)
    }

    /// Paints every surface, moved up by `offset` pixels, the distance its host's content
    /// is scrolled.
    pub(crate) fn paint(&self, dc: HDC, offset: i32, palette: &Palette, scale: Scale) {
        for painted in self.painted.borrow().iter() {
            let rect = RECT {
                top: painted.rect.top - offset,
                bottom: painted.rect.bottom - offset,
                ..painted.rect
            };
            paint_surface(dc, rect, painted.surface, palette, scale);
        }
    }

    fn with<R>(&self, id: u32, change: impl FnOnce(&mut PaintedSurface) -> R) -> R {
        let mut painted = self.painted.borrow_mut();
        let surface = painted
            .iter_mut()
            .find(|painted| painted.id == id)
            .expect("surfaces are only used until they are removed");
        change(surface)
    }
}

fn contains(rect: &RECT, point: POINT) -> bool {
    (rect.left..rect.right).contains(&point.x) && (rect.top..rect.bottom).contains(&point.y)
}
