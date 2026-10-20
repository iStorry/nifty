//! The user's appearance settings that Win32 controls don't follow by themselves: the
//! accent color, the text size chosen under Accessibility, and the reading direction.

use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Globalization::{
    GetLocaleInfoEx, GetUserDefaultUILanguage, LCIDToLocaleName, LOCALE_IREADINGLAYOUT,
    LOCALE_RETURN_NUMBER,
};
use windows::Win32::Graphics::Gdi::{COLOR_HOTLIGHT, GetSysColor};
use windows::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;
use windows::core::{PCWSTR, Result};

/// `LOCALE_IREADINGLAYOUT`'s value for languages read right to left, such as Arabic.
const RIGHT_TO_LEFT_LAYOUT: u32 = 1;

/// Text at the standard size, as Settings shows it.
pub(crate) const STANDARD_TEXT_SIZE: u16 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    /// The Accessibility text size as a percentage, from 100 up to 225.
    pub(crate) text_size_percent: u16,
    /// The accent shade Fluent uses for text on a light background.
    pub(crate) accent_text: COLORREF,
}

impl Settings {
    /// The current settings. Windows without `UISettings`, such as Server Core, gets the
    /// standard text size and the system hyperlink color.
    pub(crate) fn current() -> Self {
        Self::read().unwrap_or_else(|_| Self {
            text_size_percent: STANDARD_TEXT_SIZE,
            // SAFETY: `GetSysColor` accepts every SYS_COLOR_INDEX.
            accent_text: COLORREF(unsafe { GetSysColor(COLOR_HOTLIGHT) }),
        })
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the text scale factor is between 1.0 and 2.25"
    )]
    fn read() -> Result<Self> {
        let ui = UISettings::new()?;
        let accent = ui.GetColorValue(UIColorType::AccentDark1)?;
        Ok(Self {
            text_size_percent: (ui.TextScaleFactor()? * f64::from(STANDARD_TEXT_SIZE)).round()
                as u16,
            accent_text: rgb(accent.R, accent.G, accent.B),
        })
    }
}

/// Whether the user's display language reads right to left, which flips where text
/// aligned to the end goes.
pub(crate) fn reads_right_to_left() -> bool {
    let mut name = [0u16; LOCALE_NAME_MAX_LENGTH as usize];
    // SAFETY: `name` is a buffer of the length passed; the language id comes from Windows.
    let written =
        unsafe { LCIDToLocaleName(u32::from(GetUserDefaultUILanguage()), Some(&mut name), 0) };
    if written == 0 {
        return false;
    }
    let mut layout = [0u16; 2];
    // SAFETY: with LOCALE_RETURN_NUMBER the call writes one DWORD into the two-word buffer.
    let read = unsafe {
        GetLocaleInfoEx(
            PCWSTR(name.as_ptr()),
            LOCALE_IREADINGLAYOUT | LOCALE_RETURN_NUMBER,
            Some(&mut layout),
        )
    };
    read != 0 && (u32::from(layout[0]) | u32::from(layout[1]) << 16) == RIGHT_TO_LEFT_LAYOUT
}

pub(crate) const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF(red as u32 | (green as u32) << 8 | (blue as u32) << 16)
}
