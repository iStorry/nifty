//! Nifty's icons as glyphs of Segoe Fluent Icons, the icon font Windows 11 ships, which
//! keeps the code points of Windows 10's Segoe MDL2 Assets. Apps bundle no images.

use nifty_runtime::Icon;

/// The glyph for `icon` in the icon font; `None` for `Icon::None`.
pub(crate) fn glyph(icon: Icon) -> Option<char> {
    let code = match icon {
        Icon::None => return None,
        Icon::Plus => 0xE710,
        Icon::Trash => 0xE74D,
        Icon::Star => 0xE734,
        Icon::StarFill => 0xE735,
        Icon::Check => 0xE73E,
        Icon::CheckCircle => 0xE930,
        Icon::Circle => 0xEA3A,
        Icon::Home => 0xE80F,
        // The fonts have no inbox tray, so an inbox shows the mail envelope.
        Icon::Inbox | Icon::Mail => 0xE715,
        Icon::Archive => 0xE7B8,
        Icon::List => 0xEA37,
        Icon::Calendar => 0xE787,
        Icon::Sun => 0xE706,
        Icon::Flag => 0xE7C1,
        Icon::Close => 0xE711,
        Icon::Gear => 0xE713,
        Icon::Search => 0xE721,
        Icon::Send => 0xE724,
        Icon::Folder => 0xE8B7,
        Icon::Clock => 0xE823,
        Icon::Tag => 0xE8EC,
        Icon::Person => 0xE77B,
        Icon::Document => 0xE8A5,
        Icon::Download => 0xE896,
        Icon::Heart => 0xEB51,
        Icon::Bell => 0xEA8F,
        Icon::Pencil => 0xE70F,
        Icon::Share => 0xE72D,
        Icon::Info => 0xE946,
    };
    Some(char::from_u32(code).expect("icon code points are in the private use area"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_but_none_has_a_glyph() {
        for &icon in Icon::ALL {
            assert_eq!(glyph(icon).is_some(), icon != Icon::None, "{icon:?}");
        }
    }
}
