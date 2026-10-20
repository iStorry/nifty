//! The colors a window paints with in the user's appearance, and the brushes Win32
//! controls are handed to paint their backgrounds.

use windows::Win32::Foundation::COLORREF;
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_HIGHLIGHTTEXT,
    COLOR_HOTLIGHT, COLOR_WINDOW, COLOR_WINDOWTEXT, CreateSolidBrush, GetSysColor, HBRUSH,
    SYS_COLOR_INDEX,
};
use windows::core::Result;

use nifty_runtime::{Surface, TextColor};

use crate::gdi::Owned;
use crate::settings::{Accent, Appearance, Settings, rgb};

const WHITE: COLORREF = rgb(0xFF, 0xFF, 0xFF);
const BLACK: COLORREF = rgb(0x00, 0x00, 0x00);

/// Fluent's colors for one mode, flattened onto its solid window background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Palette {
    pub(crate) background: COLORREF,
    pub(crate) text: COLORREF,
    secondary_text: COLORREF,
    tertiary_text: COLORREF,
    accent_text: COLORREF,
    success_text: COLORREF,
    warning_text: COLORREF,
    danger_text: COLORREF,
    pub(crate) divider: COLORREF,
    pub(crate) field: COLORREF,
    sidebar: COLORREF,
    content: COLORREF,
    card: COLORREF,
    pub(crate) card_border: COLORREF,
    pub(crate) control: Fill,
    pub(crate) accent_control: Fill,
    pub(crate) control_border: COLORREF,
    /// The outline of an unchecked checkbox.
    pub(crate) strong_border: COLORREF,
}

/// A button-like surface in each state, and the text drawn on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Fill {
    pub(crate) rest: COLORREF,
    pub(crate) hot: COLORREF,
    pub(crate) pressed: COLORREF,
    pub(crate) text: COLORREF,
}

impl Fill {
    fn solid(color: COLORREF, text: COLORREF) -> Self {
        Self {
            rest: color,
            hot: color,
            pressed: color,
            text,
        }
    }
}

impl Palette {
    pub(crate) fn of(settings: &Settings) -> Self {
        match settings.appearance {
            Appearance::Light => Self::light(settings.accent),
            Appearance::Dark => Self::dark(settings.accent),
            Appearance::HighContrast => Self::high_contrast(),
        }
    }

    fn light(accent: Accent) -> Self {
        Self {
            background: rgb(0xF3, 0xF3, 0xF3),
            text: rgb(0x1A, 0x1A, 0x1A),
            secondary_text: rgb(0x5F, 0x5F, 0x5F),
            tertiary_text: rgb(0x8A, 0x8A, 0x8A),
            accent_text: accent.on_light,
            success_text: rgb(0x0F, 0x7B, 0x0F),
            warning_text: rgb(0x9D, 0x5D, 0x00),
            danger_text: rgb(0xC4, 0x2B, 0x1C),
            divider: rgb(0xE5, 0xE5, 0xE5),
            field: WHITE,
            sidebar: rgb(0xEB, 0xEB, 0xEB),
            content: rgb(0xF9, 0xF9, 0xF9),
            card: WHITE,
            card_border: rgb(0xE5, 0xE5, 0xE5),
            control: Fill {
                rest: rgb(0xFB, 0xFB, 0xFB),
                hot: rgb(0xF6, 0xF6, 0xF6),
                pressed: rgb(0xF0, 0xF0, 0xF0),
                text: rgb(0x1A, 0x1A, 0x1A),
            },
            accent_control: accent_fill(accent.on_light, rgb(0xF3, 0xF3, 0xF3), WHITE),
            control_border: rgb(0xD5, 0xD5, 0xD5),
            strong_border: rgb(0x86, 0x86, 0x86),
        }
    }

    fn dark(accent: Accent) -> Self {
        Self {
            background: rgb(0x20, 0x20, 0x20),
            text: WHITE,
            secondary_text: rgb(0xCE, 0xCE, 0xCE),
            tertiary_text: rgb(0x96, 0x96, 0x96),
            accent_text: accent.on_dark,
            success_text: rgb(0x6C, 0xCB, 0x5F),
            warning_text: rgb(0xFC, 0xE1, 0x00),
            danger_text: rgb(0xFF, 0x99, 0xA4),
            divider: rgb(0x33, 0x33, 0x33),
            field: rgb(0x2D, 0x2D, 0x2D),
            sidebar: rgb(0x1C, 0x1C, 0x1C),
            content: rgb(0x27, 0x27, 0x27),
            card: rgb(0x2B, 0x2B, 0x2B),
            card_border: rgb(0x1A, 0x1A, 0x1A),
            control: Fill {
                rest: rgb(0x2D, 0x2D, 0x2D),
                hot: rgb(0x32, 0x32, 0x32),
                pressed: rgb(0x27, 0x27, 0x27),
                text: WHITE,
            },
            accent_control: accent_fill(accent.on_dark, rgb(0x20, 0x20, 0x20), BLACK),
            control_border: rgb(0x3A, 0x3A, 0x3A),
            strong_border: rgb(0x9A, 0x9A, 0x9A),
        }
    }

    /// High contrast themes allow only their own colors, so every role maps to one.
    fn high_contrast() -> Self {
        let text = system_color(COLOR_WINDOWTEXT);
        let button_text = system_color(COLOR_BTNTEXT);
        Self {
            background: system_color(COLOR_WINDOW),
            text,
            secondary_text: text,
            tertiary_text: text,
            accent_text: system_color(COLOR_HOTLIGHT),
            success_text: text,
            warning_text: text,
            danger_text: text,
            divider: system_color(COLOR_GRAYTEXT),
            field: system_color(COLOR_WINDOW),
            sidebar: system_color(COLOR_WINDOW),
            content: system_color(COLOR_WINDOW),
            card: system_color(COLOR_WINDOW),
            card_border: text,
            control: Fill::solid(system_color(COLOR_BTNFACE), button_text),
            accent_control: Fill::solid(
                system_color(COLOR_HIGHLIGHT),
                system_color(COLOR_HIGHLIGHTTEXT),
            ),
            control_border: button_text,
            strong_border: text,
        }
    }

    /// The color a surface fills its area with, or the window's own for no surface.
    pub(crate) fn surface(&self, surface: Surface) -> COLORREF {
        match surface {
            Surface::None => self.background,
            Surface::Sidebar => self.sidebar,
            Surface::Content => self.content,
            Surface::Card => self.card,
        }
    }

    pub(crate) fn text_color(&self, color: TextColor) -> COLORREF {
        match color {
            TextColor::Primary => self.text,
            TextColor::Secondary => self.secondary_text,
            TextColor::Tertiary => self.tertiary_text,
            TextColor::Accent => self.accent_text,
            TextColor::Success => self.success_text,
            TextColor::Warning => self.warning_text,
            TextColor::Danger => self.danger_text,
        }
    }
}

/// Fluent's accent button: hovering and pressing fade the accent toward the background.
fn accent_fill(accent: COLORREF, background: COLORREF, text: COLORREF) -> Fill {
    Fill {
        rest: accent,
        hot: blend(accent, background, 90),
        pressed: blend(accent, background, 80),
        text,
    }
}

/// `over` drawn at `percent` opacity on `under`.
pub(crate) fn blend(over: COLORREF, under: COLORREF, percent: u32) -> COLORREF {
    let channel = |shift: u32| {
        let over = (over.0 >> shift) & 0xFF;
        let under = (under.0 >> shift) & 0xFF;
        ((over * percent + under * (100 - percent) + 50) / 100) << shift
    };
    COLORREF(channel(0) | channel(8) | channel(16))
}

fn system_color(index: SYS_COLOR_INDEX) -> COLORREF {
    // SAFETY: `GetSysColor` accepts every SYS_COLOR_INDEX.
    COLORREF(unsafe { GetSysColor(index) })
}

/// A palette with the brushes `WM_CTLCOLOR*` returns, which must outlive the paint.
#[derive(Debug)]
pub(crate) struct Theme {
    pub(crate) palette: Palette,
    /// One brush per surface, indexed by its code, `Surface::None` being the window's own.
    surfaces: Vec<Owned<HBRUSH>>,
    pub(crate) field: Owned<HBRUSH>,
    pub(crate) divider: Owned<HBRUSH>,
}

impl Theme {
    pub(crate) fn new(palette: Palette) -> Result<Self> {
        Ok(Self {
            palette,
            surfaces: Surface::ALL
                .iter()
                .map(|&surface| solid_brush(palette.surface(surface)))
                .collect::<Result<_>>()?,
            field: solid_brush(palette.field)?,
            divider: solid_brush(palette.divider)?,
        })
    }

    /// The brush that fills `surface`.
    pub(crate) fn surface_brush(&self, surface: Surface) -> HBRUSH {
        self.surfaces[usize::from(surface.code())].get()
    }
}

fn solid_brush(color: COLORREF) -> Result<Owned<HBRUSH>> {
    // SAFETY: `CreateSolidBrush` accepts any color.
    Owned::new(unsafe { CreateSolidBrush(color) })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACCENT: Accent = Accent {
        on_light: rgb(0x00, 0x5F, 0xB8),
        on_dark: rgb(0x99, 0xEB, 0xFF),
    };

    #[test]
    fn dark_mode_draws_light_text_on_a_dark_background() {
        let dark = Palette::dark(ACCENT);
        let light = Palette::light(ACCENT);
        assert_eq!(dark.text, WHITE);
        assert_ne!(dark.background, light.background);
        assert_eq!(dark.text_color(TextColor::Accent), ACCENT.on_dark);
        assert_eq!(light.text_color(TextColor::Accent), ACCENT.on_light);
    }

    #[test]
    fn every_text_color_differs_between_light_and_dark() {
        let dark = Palette::dark(ACCENT);
        let light = Palette::light(ACCENT);
        for color in [
            TextColor::Primary,
            TextColor::Secondary,
            TextColor::Tertiary,
            TextColor::Accent,
            TextColor::Success,
            TextColor::Warning,
            TextColor::Danger,
        ] {
            assert_ne!(dark.text_color(color), light.text_color(color), "{color:?}");
        }
    }

    #[test]
    fn every_surface_differs_between_light_and_dark() {
        let dark = Palette::dark(ACCENT);
        let light = Palette::light(ACCENT);
        for &surface in Surface::ALL {
            assert_ne!(dark.surface(surface), light.surface(surface), "{surface:?}");
        }
        assert_ne!(dark.card_border, light.card_border);
    }

    #[test]
    fn surfaces_stand_apart_from_the_window() {
        for palette in [Palette::light(ACCENT), Palette::dark(ACCENT)] {
            for &surface in Surface::ALL.iter().filter(|&&s| s != Surface::None) {
                assert_ne!(palette.surface(surface), palette.background, "{surface:?}");
            }
        }
    }

    #[test]
    fn blending_mixes_each_channel() {
        assert_eq!(blend(WHITE, BLACK, 100), WHITE);
        assert_eq!(blend(WHITE, BLACK, 0), BLACK);
        assert_eq!(blend(WHITE, BLACK, 50), rgb(0x80, 0x80, 0x80));
    }
}
