//! Context menus: areas of a window's or scroll area's content, each with a menu that
//! shows when the user right-clicks there, or asks for it from the keyboard. Controls
//! that have no menu of their own pass `WM_CONTEXTMENU` up to their parent, the host,
//! which finds the innermost area at the point.

use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;

use nifty_runtime::{MenuEntry, MenuHandler};

use crate::areas::Areas;
use crate::convert::{hwnd_wparam, point_param, screen_to_content};
use crate::menu::show_context_menu;

/// The point `WM_CONTEXTMENU` gives when the keyboard asked for the menu.
const FROM_KEYBOARD: POINT = POINT { x: -1, y: -1 };

/// The context menus a host shows, by the areas they cover.
pub(crate) type MenuAreas = Areas<Rc<ContextMenu>>;

/// A context menu's entries, and what runs the item the user chooses.
pub(crate) struct ContextMenu {
    pub(crate) entries: Vec<MenuEntry<usize>>,
    pub(crate) on_select: MenuHandler,
}

impl std::fmt::Debug for ContextMenu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContextMenu")
            .field("entries", &self.entries)
            .finish_non_exhaustive()
    }
}

/// Answers `WM_CONTEXTMENU` in `host`, whose content is scrolled up by `offset` pixels:
/// shows the menu of the innermost area at the click, or, from the keyboard, under the
/// control that has the focus. `None` when no area is there, so the message goes on to
/// the host's own parent.
pub(crate) fn show(
    host: HWND,
    areas: &MenuAreas,
    offset: i32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    let clicked = point_param(lparam);
    let (at, shown_at) = if clicked == FROM_KEYBOARD {
        keyboard_point(hwnd_wparam(wparam))
    } else {
        (clicked, clicked)
    };
    let menu = areas.innermost_at(screen_to_content(host, offset, at))?;
    // The menu is cloned out first: its item may remove the area.
    if let Some(item) = show_context_menu(host, &menu.entries, shown_at) {
        (menu.on_select)(item);
    }
    Some(LRESULT(0))
}

/// For a menu asked for from the keyboard, the screen point inside `focused` that picks
/// its area, the control's middle, and where the menu shows, under its start.
fn keyboard_point(focused: HWND) -> (POINT, POINT) {
    let mut rect = RECT::default();
    // SAFETY: `focused` is the live window the message came from, and `rect` a valid RECT.
    unsafe { GetWindowRect(focused, &raw mut rect) }.expect("a live window has a rect");
    let middle = POINT {
        x: i32::midpoint(rect.left, rect.right),
        y: i32::midpoint(rect.top, rect.bottom),
    };
    let below = POINT {
        x: rect.left,
        y: rect.bottom,
    };
    (middle, below)
}
