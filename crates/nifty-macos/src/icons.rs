//! Nifty's icons as SF Symbols, which come with macOS, so apps bundle no images.

use objc2::rc::Retained;
use objc2_app_kit::{NSColor, NSImage, NSImageSymbolConfiguration};
use objc2_foundation::NSString;

use nifty_runtime::Icon;

/// The symbol for `icon`, drawn in the text color around it; `None` for `Icon::None`.
pub(crate) fn symbol(icon: Icon) -> Option<Retained<NSImage>> {
    let name = symbol_name(icon)?;
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        None,
    );
    Some(image.unwrap_or_else(|| panic!("macOS has the SF Symbol `{name}`")))
}

/// The symbol for `icon` in `color` rather than the text color, as sidebars draw theirs.
pub(crate) fn tinted_symbol(icon: Icon, color: &NSColor) -> Option<Retained<NSImage>> {
    let image = symbol(icon)?;
    let tint = NSImageSymbolConfiguration::configurationWithHierarchicalColor(color);
    Some(image.imageWithSymbolConfiguration(&tint).unwrap_or(image))
}

fn symbol_name(icon: Icon) -> Option<&'static str> {
    Some(match icon {
        Icon::None => return None,
        Icon::Plus => "plus",
        Icon::Trash => "trash",
        Icon::Star => "star",
        Icon::StarFill => "star.fill",
        Icon::Check => "checkmark",
        Icon::CheckCircle => "checkmark.circle",
        Icon::Circle => "circle",
        Icon::Home => "house",
        Icon::Inbox => "tray",
        Icon::Archive => "archivebox",
        Icon::List => "list.bullet",
        Icon::Calendar => "calendar",
        Icon::Sun => "sun.max",
        Icon::Flag => "flag",
        Icon::Close => "xmark",
        Icon::Gear => "gearshape",
        Icon::Search => "magnifyingglass",
        Icon::Mail => "envelope",
        Icon::Send => "paperplane",
        Icon::Folder => "folder",
        Icon::Clock => "clock",
        Icon::Tag => "tag",
        Icon::Person => "person",
        Icon::Document => "doc",
        Icon::Download => "arrow.down.circle",
        Icon::Heart => "heart",
        Icon::Bell => "bell",
        Icon::Pencil => "pencil",
        Icon::Share => "square.and.arrow.up",
        Icon::Info => "info.circle",
    })
}
