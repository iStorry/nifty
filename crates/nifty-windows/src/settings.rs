//! The user's appearance settings that Win32 controls don't follow by themselves: light
//! or dark mode, high contrast, the accent color, the text size chosen under
//! Accessibility, and the reading direction.

use std::mem;

use windows::UI::Color;
use windows::UI::ViewManagement::{UIColorType, UISettings};
use windows::Win32::Foundation::COLORREF;
use windows::Win32::Globalization::{
    GetLocaleInfoEx, GetUserDefaultUILanguage, LCIDToLocaleName, LOCALE_IREADINGLAYOUT,
    LOCALE_RETURN_NUMBER,
};
use windows::Win32::Graphics::Gdi::{COLOR_HOTLIGHT, GetSysColor};
use windows::Win32::System::SystemServices::LOCALE_NAME_MAX_LENGTH;
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::{PCWSTR, Result};

/// `LOCALE_IREADINGLAYOUT`'s value for languages read right to left, such as Arabic.
const RIGHT_TO_LEFT_LAYOUT: u32 = 1;

/// Text at the standard size, as Settings shows it.
pub(crate) const STANDARD_TEXT_SIZE: u16 = 100;

/// Which set of colors the user chose for apps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Appearance {
    Light,
    Dark,
    /// A high-contrast theme, whose colors only the system colors carry.
    HighContrast,
}

/// The two accent shades Fluent uses, one for each mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Accent {
    pub(crate) on_light: COLORREF,
    pub(crate) on_dark: COLORREF,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Settings {
    /// The Accessibility text size as a percentage, from 100 up to 225.
    pub(crate) text_size_percent: u16,
    pub(crate) appearance: Appearance,
    pub(crate) accent: Accent,
}

impl Settings {
    /// The current settings. Windows without `UISettings`, such as Server Core, gets the
    /// standard text size, light mode and the system hyperlink color.
    pub(crate) fn current() -> Self {
        let settings = Self::read().unwrap_or_else(|_| {
            // SAFETY: `GetSysColor` accepts every SYS_COLOR_INDEX.
            let hyperlink = COLORREF(unsafe { GetSysColor(COLOR_HOTLIGHT) });
            Self {
                text_size_percent: STANDARD_TEXT_SIZE,
                appearance: Appearance::Light,
                accent: Accent {
                    on_light: hyperlink,
                    on_dark: hyperlink,
                },
            }
        });
        if high_contrast_is_on() {
            Self {
                appearance: Appearance::HighContrast,
                ..settings
            }
        } else {
            settings
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "the text scale factor is between 1.0 and 2.25"
    )]
    fn read() -> Result<Self> {
        let ui = UISettings::new()?;
        let appearance = if is_dark(ui.GetColorValue(UIColorType::Background)?) {
            Appearance::Dark
        } else {
            Appearance::Light
        };
        Ok(Self {
            text_size_percent: (ui.TextScaleFactor()? * f64::from(STANDARD_TEXT_SIZE)).round()
                as u16,
            appearance,
            accent: Accent {
                on_light: color_ref(ui.GetColorValue(UIColorType::AccentDark1)?),
                on_dark: color_ref(ui.GetColorValue(UIColorType::AccentLight2)?),
            },
        })
    }
}

/// Microsoft's documented test for the app background `UISettings` reports: the theme is
/// dark when the background's weighted brightness is in the lower half.
fn is_dark(background: Color) -> bool {
    5 * u32::from(background.G) + 2 * u32::from(background.R) + u32::from(background.B) <= 8 * 128
}

fn high_contrast_is_on() -> bool {
    let mut contrast = HIGHCONTRASTW {
        cbSize: u32::try_from(mem::size_of::<HIGHCONTRASTW>()).expect("HIGHCONTRASTW is small"),
        ..Default::default()
    };
    // SAFETY: `contrast` is a HIGHCONTRASTW whose `cbSize` matches the buffer passed.
    let read = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            contrast.cbSize,
            Some(std::ptr::from_mut(&mut contrast).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    };
    read.is_ok() && contrast.dwFlags.contains(HCF_HIGHCONTRASTON)
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

fn color_ref(color: Color) -> COLORREF {
    rgb(color.R, color.G, color.B)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque(red: u8, green: u8, blue: u8) -> Color {
        Color {
            A: 0xFF,
            R: red,
            G: green,
            B: blue,
        }
    }

    #[test]
    fn the_dark_theme_is_told_apart_by_its_background() {
        assert!(is_dark(opaque(0x00, 0x00, 0x00)));
        assert!(!is_dark(opaque(0xFF, 0xFF, 0xFF)));
    }
}
