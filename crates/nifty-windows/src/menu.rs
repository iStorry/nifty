//! Menus: the app's menus in every window's menu bar, their keyboard shortcuts as an
//! accelerator table, and the pop-up menus pickers show. All are Win32's own, which run
//! their own loop while open.

use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    VIRTUAL_KEY, VK_DELETE, VK_DOWN, VK_LEFT, VK_RETURN, VK_RIGHT, VK_UP, VkKeyScanW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ACCEL, AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreatePopupMenu,
    DestroyAcceleratorTable, DestroyMenu, FALT, FCONTROL, FSHIFT, FVIRTKEY, GA_ROOT, GetAncestor,
    GetWindowRect, HACCEL, HMENU, MENU_ITEM_FLAGS, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    MSG, TPM_LEFTALIGN, TPM_NONOTIFY, TPM_RETURNCMD, TPM_TOPALIGN, TrackPopupMenuEx,
    TranslateAcceleratorW,
};
use windows::core::PCWSTR;

use nifty_runtime::{Key, Menu, MenuEntry, MenuHandler, Shortcut};

use crate::convert::{escape_mnemonics, wide};

/// Menu items' command ids start here, above the ids the dialog manager sends itself, such
/// as `IDCANCEL` for Escape; they come with no control, which tells them from controls'.
const FIRST_COMMAND: u16 = 0x1000;

/// A Win32 menu, destroyed on drop unless it is handed to a window or another menu.
pub(crate) struct OwnedMenu(HMENU);

impl OwnedMenu {
    fn popup() -> Self {
        // SAFETY: `CreatePopupMenu` has no preconditions.
        Self(unsafe { CreatePopupMenu() }.expect("Windows can create a menu"))
    }

    fn bar() -> Self {
        // SAFETY: `CreateMenu` has no preconditions.
        Self(unsafe { CreateMenu() }.expect("Windows can create a menu bar"))
    }

    /// Hands the menu to whatever destroys it from now on, such as the window it is set on.
    pub(crate) fn into_handle(self) -> HMENU {
        ManuallyDrop::new(self).0
    }

    fn append(&self, flags: MENU_ITEM_FLAGS, id: usize, text: &str) {
        let text = wide(text);
        // SAFETY: the menu is live, and Windows copies the NUL-terminated text.
        unsafe { AppendMenuW(self.0, flags, id, PCWSTR(text.as_ptr())) }
            .expect("Windows can add an item to a menu");
    }

    /// An item titled `title`, which answers `id` when picked.
    fn add_item(&self, title: &str, id: usize, checked: bool) {
        let flags = if checked {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        self.append(flags, id, &escape_mnemonics(title));
    }

    fn add_separator(&self) {
        self.append(MF_SEPARATOR, 0, "");
    }

    /// Adds `submenu`, which this menu destroys with itself from now on.
    fn add_submenu(&self, title: &str, submenu: Self) {
        let handle = submenu.into_handle();
        self.append(MF_POPUP, handle.0.addr(), &escape_mnemonics(title));
    }

    /// The app's `entries`, each item numbered by its command id.
    fn fill(&self, entries: &[MenuEntry<usize>]) {
        for entry in entries {
            match entry {
                MenuEntry::Item {
                    title,
                    shortcut,
                    action,
                } => {
                    let text = match shortcut {
                        Some(shortcut) => {
                            format!("{}\t{}", escape_mnemonics(title), hint(*shortcut))
                        }
                        None => escape_mnemonics(title).into_owned(),
                    };
                    self.append(MF_STRING, usize::from(command_id(*action)), &text);
                }
                MenuEntry::Separator => self.add_separator(),
                MenuEntry::Submenu(submenu) => {
                    let menu = Self::popup();
                    menu.fill(&submenu.entries);
                    self.add_submenu(&submenu.title, menu);
                }
            }
        }
    }
}

impl Drop for OwnedMenu {
    fn drop(&mut self) {
        // SAFETY: the menu is still owned here, and isn't shown any more.
        unsafe { DestroyMenu(self.0) }.expect("a menu Nifty made can be destroyed");
    }
}

/// The app's menus and what runs their items, shared by every window, which each show
/// their own copy of the menu bar.
#[derive(Default)]
pub(crate) struct AppMenus {
    menus: RefCell<Vec<Menu<usize>>>,
    on_select: RefCell<Option<Rc<MenuHandler>>>,
    accelerators: RefCell<Option<Accelerators>>,
}

impl std::fmt::Debug for AppMenus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppMenus")
            .field("menus", &self.menus)
            .finish_non_exhaustive()
    }
}

impl AppMenus {
    pub(crate) fn set(&self, menus: &[Menu<usize>], on_select: MenuHandler) {
        *self.accelerators.borrow_mut() = Accelerators::of(menus);
        menus.clone_into(&mut self.menus.borrow_mut());
        *self.on_select.borrow_mut() = Some(Rc::new(on_select));
    }

    /// A new menu bar holding the app's menus, or `None` while it has none.
    pub(crate) fn bar(&self) -> Option<OwnedMenu> {
        let menus = self.menus.borrow();
        if menus.is_empty() {
            return None;
        }
        let bar = OwnedMenu::bar();
        for menu in menus.iter() {
            let submenu = OwnedMenu::popup();
            submenu.fill(&menu.entries);
            bar.add_submenu(&menu.title, submenu);
        }
        Some(bar)
    }

    /// Runs the item a menu or shortcut chose by command `id`, answering whether `id` is
    /// a menu item's.
    pub(crate) fn choose(&self, id: u16) -> bool {
        let Some(item) = id.checked_sub(FIRST_COMMAND) else {
            return false;
        };
        // The handler is cloned out first: it may change the menus.
        let on_select = self.on_select.borrow().clone();
        if let Some(on_select) = on_select {
            on_select(usize::from(item));
        }
        true
    }

    /// Turns a key press that is one of the items' shortcuts into a command for the window
    /// `root`, answering whether it did.
    pub(crate) fn translate(&self, root: HWND, message: &MSG) -> bool {
        self.accelerators
            .borrow()
            .as_ref()
            .is_some_and(|accelerators| {
                // SAFETY: `root` is a live Nifty window and `message` came from its queue.
                unsafe { TranslateAcceleratorW(root, accelerators.0, message) != 0 }
            })
    }
}

/// The items' shortcuts, as an accelerator table, destroyed on drop.
struct Accelerators(HACCEL);

impl Accelerators {
    /// The table of every item's shortcut in `menus`, or `None` when no item has one.
    fn of(menus: &[Menu<usize>]) -> Option<Self> {
        let mut entries = Vec::new();
        for menu in menus {
            collect(&menu.entries, &mut entries);
        }
        if entries.is_empty() {
            return None;
        }
        // SAFETY: `entries` is a valid list of accelerators, which Windows copies.
        let table = unsafe { CreateAcceleratorTableW(&entries) }
            .expect("Windows can make an accelerator table");
        Some(Self(table))
    }
}

impl Drop for Accelerators {
    fn drop(&mut self) {
        // SAFETY: the table was made by `of` and no window uses it any more.
        let destroyed = unsafe { DestroyAcceleratorTable(self.0) };
        debug_assert!(
            destroyed.as_bool(),
            "an accelerator table Nifty made can be destroyed"
        );
    }
}

/// Adds an accelerator for each item in `entries` with a shortcut whose key the keyboard
/// has.
fn collect(entries: &[MenuEntry<usize>], accelerators: &mut Vec<ACCEL>) {
    for entry in entries {
        match entry {
            MenuEntry::Item {
                shortcut: Some(shortcut),
                action,
                ..
            } => accelerators.extend(virtual_key(shortcut.key).map(|key| {
                let mut flags = FVIRTKEY | FCONTROL;
                if shortcut.shift {
                    flags |= FSHIFT;
                }
                if shortcut.alt {
                    flags |= FALT;
                }
                ACCEL {
                    fVirt: flags,
                    key: key.0,
                    cmd: command_id(*action),
                }
            })),
            MenuEntry::Submenu(submenu) => collect(&submenu.entries, accelerators),
            MenuEntry::Item { shortcut: None, .. } | MenuEntry::Separator => {}
        }
    }
}

/// An item's command id.
fn command_id(item: usize) -> u16 {
    u16::try_from(item)
        .ok()
        .and_then(|item| item.checked_add(FIRST_COMMAND))
        .expect("an app has fewer menu items than command ids")
}

/// The key's virtual-key code on the user's keyboard layout, or `None` for a character
/// the layout has no key for.
fn virtual_key(key: Key) -> Option<VIRTUAL_KEY> {
    let code = match key {
        Key::Return => return Some(VK_RETURN),
        Key::Delete => return Some(VK_DELETE),
        Key::Up => return Some(VK_UP),
        Key::Down => return Some(VK_DOWN),
        Key::Left => return Some(VK_LEFT),
        Key::Right => return Some(VK_RIGHT),
        typed => typed
            .character()
            .expect("every other key types a character"),
    };
    let mut utf16 = [0; 2];
    let [unit] = code.encode_utf16(&mut utf16) else {
        return None;
    };
    // SAFETY: `VkKeyScanW` accepts any character.
    let scan = unsafe { VkKeyScanW(*unit) };
    // The low byte is the key; -1 means the layout has none for the character.
    u8::try_from(scan & 0xFF)
        .ok()
        .filter(|_| scan != -1)
        .map(|key| VIRTUAL_KEY(u16::from(key)))
}

/// A shortcut as Windows menus show it after an item's title, such as `Ctrl+Shift+N`.
fn hint(shortcut: Shortcut) -> String {
    let mut hint = String::from("Ctrl+");
    if shortcut.alt {
        hint.push_str("Alt+");
    }
    if shortcut.shift {
        hint.push_str("Shift+");
    }
    match shortcut.key {
        Key::Return => hint.push_str("Enter"),
        Key::Delete => hint.push_str("Del"),
        Key::Up => hint.push_str("Up"),
        Key::Down => hint.push_str("Down"),
        Key::Left => hint.push_str("Left"),
        Key::Right => hint.push_str("Right"),
        typed => hint.extend(
            typed
                .character()
                .expect("every other key types a character")
                .to_uppercase(),
        ),
    }
    hint
}

/// Shows `options` in a menu just under `anchor`, with `chosen` ticked, and returns the
/// index of the option the user picks, or `None` when they close it.
pub(crate) fn choose(anchor: HWND, options: &[String], chosen: &str) -> Option<usize> {
    let menu = OwnedMenu::popup();
    for (index, option) in options.iter().enumerate() {
        // Ids start at 1, since 0 is what the menu answers when nothing is picked.
        menu.add_item(option, index + 1, option == chosen);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints_read_as_windows_writes_them() {
        let shortcut = |key, shift, alt| Shortcut { key, shift, alt };
        assert_eq!(hint(shortcut(Key::N, false, false)), "Ctrl+N");
        assert_eq!(hint(shortcut(Key::N, true, true)), "Ctrl+Alt+Shift+N");
        assert_eq!(hint(shortcut(Key::Return, false, false)), "Ctrl+Enter");
        assert_eq!(hint(shortcut(Key::Comma, false, false)), "Ctrl+,");
    }
}
