//! Pop-up menus: Win32's own, which run their own loop until the user picks or closes them.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GA_ROOT, GetAncestor, GetWindowRect, HMENU,
    MF_CHECKED, MF_STRING, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_TOPALIGN,
    TrackPopupMenuEx,
};
use windows::core::PCWSTR;

use crate::convert::{escape_mnemonics, wide};

/// A pop-up menu, destroyed on drop.
struct PopupMenu(HMENU);

impl PopupMenu {
    fn new() -> Self {
        // SAFETY: `CreatePopupMenu` has no preconditions.
        Self(unsafe { CreatePopupMenu() }.expect("Windows can create a menu"))
    }

    /// Adds an item that answers `id` when picked, ticked when `checked`.
    fn add(&self, title: &str, id: usize, checked: bool) {
        let flags = if checked {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        let title = wide(&escape_mnemonics(title));
        // SAFETY: the menu is live, and Windows copies the NUL-terminated title.
        unsafe { AppendMenuW(self.0, flags, id, PCWSTR(title.as_ptr())) }
            .expect("Windows can add an item to a menu");
    }
}

impl Drop for PopupMenu {
    fn drop(&mut self) {
        // SAFETY: the menu was created by `new` and isn't shown any more.
        unsafe { DestroyMenu(self.0) }.expect("a menu Nifty made can be destroyed");
    }
}

/// Shows `options` in a menu just under `anchor`, with `chosen` ticked, and returns the
/// index of the option the user picks, or `None` when they close it.
pub(crate) fn choose(anchor: HWND, options: &[String], chosen: &str) -> Option<usize> {
    let menu = PopupMenu::new();
    for (index, option) in options.iter().enumerate() {
        // Ids start at 1, since 0 is what the menu answers when nothing is picked.
        menu.add(option, index + 1, option == chosen);
    }
    let mut below = RECT::default();
    // SAFETY: `anchor` is a live control on this thread, and `below` a valid RECT.
    unsafe { GetWindowRect(anchor, &raw mut below) }.expect("a live control has a rect");
    // SAFETY: `GetAncestor` accepts any window handle.
    let owner = unsafe { GetAncestor(anchor, GA_ROOT) };
    let flags = TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY;
    // SAFETY: the menu is live, and its owner is a window of this thread; with
    // TPM_RETURNCMD the call answers the picked item's id rather than posting it.
    let picked =
        unsafe { TrackPopupMenuEx(menu.0, flags.0, below.left, below.bottom, owner, None) };
    usize::try_from(picked.0).ok()?.checked_sub(1)
}
