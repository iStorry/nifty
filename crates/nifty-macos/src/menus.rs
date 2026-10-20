//! The app's menu bar: the app menu and the Edit menu `AppKit` apps have, and the app's
//! own menus, the first before Edit and the rest after it, as File comes before Edit.

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSMenu, NSMenuItem};
use objc2_foundation::NSString;

use nifty_runtime::{Key, Menu, MenuEntry, Shortcut, SystemCommand};

use crate::classes::MenuTarget;

/// The key equivalents `AppKit` gives the keys that type no character.
const RETURN: &str = "\r";
const BACKSPACE: &str = "\u{8}";
const UP_ARROW: &str = "\u{F700}";
const DOWN_ARROW: &str = "\u{F701}";
const LEFT_ARROW: &str = "\u{F702}";
const RIGHT_ARROW: &str = "\u{F703}";

/// The Edit menu's commands, and the responder-chain actions `AppKit` implements for them;
/// they make editing shortcuts work in text fields, which take them through the menu.
fn edit_commands() -> [(SystemCommand, Sel); 6] {
    [
        (SystemCommand::Undo, sel!(undo:)),
        (SystemCommand::Redo, sel!(redo:)),
        (SystemCommand::Cut, sel!(cut:)),
        (SystemCommand::Copy, sel!(copy:)),
        (SystemCommand::Paste, sel!(paste:)),
        (SystemCommand::SelectAll, sel!(selectAll:)),
    ]
}

/// Replaces the menu bar with the app menu, the Edit menu and `menus`, whose items tell
/// `target` when they are chosen.
pub(crate) fn install(
    app: &NSApplication,
    mtm: MainThreadMarker,
    menus: &[Menu<usize>],
    target: Option<&MenuTarget>,
) {
    let menu_bar = NSMenu::new(mtm);
    let quit = system_item(mtm, SystemCommand::Quit, sel!(terminate:));
    add_submenu(&menu_bar, &submenu(mtm, "", [quit]), mtm);
    let (first, rest) = match menus.split_first() {
        Some((first, rest)) => (Some(first), rest),
        None => (None, menus),
    };
    if let Some(first) = first {
        add_submenu(&menu_bar, &app_menu(mtm, first, target), mtm);
    }
    let edit = edit_commands()
        .into_iter()
        .map(|(command, action)| system_item(mtm, command, action));
    add_submenu(&menu_bar, &submenu(mtm, "Edit", edit), mtm);
    for menu in rest {
        add_submenu(&menu_bar, &app_menu(mtm, menu, target), mtm);
    }
    app.setMainMenu(Some(&menu_bar));
}

fn submenu(
    mtm: MainThreadMarker,
    title: &str,
    items: impl IntoIterator<Item = Retained<NSMenuItem>>,
) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for item in items {
        menu.addItem(&item);
    }
    menu
}

fn add_submenu(menu_bar: &NSMenu, menu: &NSMenu, mtm: MainThreadMarker) {
    let item = NSMenuItem::new(mtm);
    item.setSubmenu(Some(menu));
    menu_bar.addItem(&item);
}

/// A context menu of `entries`, which tell `target` when they are chosen.
pub(crate) fn context_menu(
    mtm: MainThreadMarker,
    entries: &[MenuEntry<usize>],
    target: &MenuTarget,
) -> Retained<NSMenu> {
    entries_menu(mtm, "", entries, Some(target))
}

/// One of the app's own menus, its items numbered by their tags.
fn app_menu(
    mtm: MainThreadMarker,
    menu: &Menu<usize>,
    target: Option<&MenuTarget>,
) -> Retained<NSMenu> {
    entries_menu(mtm, &menu.title, &menu.entries, target)
}

fn entries_menu(
    mtm: MainThreadMarker,
    title: &str,
    entries: &[MenuEntry<usize>],
    target: Option<&MenuTarget>,
) -> Retained<NSMenu> {
    let items = entries.iter().map(|entry| match entry {
        MenuEntry::Item {
            title,
            shortcut,
            action,
        } => {
            let item = item(mtm, title, MenuTarget::action(), *shortcut);
            item.setTag(isize::try_from(*action).expect("menus have far fewer items than isize"));
            // SAFETY: `MenuTarget` implements the action, and the backend keeps it alive
            // for as long as these menus show.
            unsafe { item.setTarget(target.map(AsRef::as_ref)) };
            item
        }
        MenuEntry::Separator => NSMenuItem::separatorItem(mtm),
        MenuEntry::Submenu(submenu) => {
            let item = NSMenuItem::new(mtm);
            item.setTitle(&NSString::from_str(&submenu.title));
            item.setSubmenu(Some(&app_menu(mtm, submenu, target)));
            item
        }
    });
    submenu(mtm, title, items)
}

fn system_item(mtm: MainThreadMarker, command: SystemCommand, action: Sel) -> Retained<NSMenuItem> {
    item(mtm, command.title(), action, Some(command.shortcut()))
}

fn item(
    mtm: MainThreadMarker,
    title: &str,
    action: Sel,
    shortcut: Option<Shortcut>,
) -> Retained<NSMenuItem> {
    let (key, modifiers) = shortcut.map_or(
        (String::new(), NSEventModifierFlags::empty()),
        key_equivalent,
    );
    // SAFETY: `action` is a selector the item's target, or the responder chain, implements.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::from_str(&key),
        )
    };
    item.setKeyEquivalentModifierMask(modifiers);
    item
}

/// The key equivalent and modifiers `AppKit` writes `shortcut` as.
fn key_equivalent(shortcut: Shortcut) -> (String, NSEventModifierFlags) {
    let key = match shortcut.key {
        Key::Return => RETURN.to_owned(),
        Key::Delete => BACKSPACE.to_owned(),
        Key::Up => UP_ARROW.to_owned(),
        Key::Down => DOWN_ARROW.to_owned(),
        Key::Left => LEFT_ARROW.to_owned(),
        Key::Right => RIGHT_ARROW.to_owned(),
        typed => typed
            .character()
            .map(String::from)
            .expect("every other key types a character"),
    };
    let mut modifiers = NSEventModifierFlags::Command;
    if shortcut.shift {
        modifiers |= NSEventModifierFlags::Shift;
    }
    if shortcut.alt {
        modifiers |= NSEventModifierFlags::Option;
    }
    (key, modifiers)
}
