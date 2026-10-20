//! The system UI font at each Nifty text style, created for one window's DPI.

use std::mem;

use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, FW_NORMAL, FW_SEMIBOLD, HFONT};
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS};
use windows::core::Result;

use nifty_runtime::TextStyle;

use crate::convert::Scale;
use crate::gdi::Owned;
use crate::settings::STANDARD_TEXT_SIZE;

/// Each style's size in DIPs and weight, from the Windows 11 type ramp.
const TYPE_RAMP: [(TextStyle, f32, u32); 4] = [
    (TextStyle::Body, 14.0, FW_NORMAL.0),
    (TextStyle::Title, 28.0, FW_SEMIBOLD.0),
    (TextStyle::Headline, 14.0, FW_SEMIBOLD.0),
    (TextStyle::Caption, 12.0, FW_NORMAL.0),
];

#[derive(Debug)]
pub(crate) struct Fonts(Vec<(TextStyle, Owned<HFONT>)>);

impl Fonts {
    /// Takes the face from the system's message font, so apps follow the user's UI font,
    /// and grows each size to the user's Accessibility text size.
    pub(crate) fn new(scale: Scale, text_size_percent: u16) -> Result<Self> {
        let text_scale = f32::from(text_size_percent) / f32::from(STANDARD_TEXT_SIZE);
        let mut metrics = NONCLIENTMETRICSW {
            cbSize: u32::try_from(mem::size_of::<NONCLIENTMETRICSW>())
                .expect("NONCLIENTMETRICSW is a few hundred bytes"),
            ..Default::default()
        };
        // SAFETY: `metrics` is a NONCLIENTMETRICSW whose `cbSize` matches the buffer passed.
        unsafe {
            SystemParametersInfoForDpi(
                SPI_GETNONCLIENTMETRICS.0,
                metrics.cbSize,
                Some(std::ptr::from_mut(&mut metrics).cast()),
                0,
                scale.dpi(),
            )?;
        }
        // Filled one font at a time, so a failure part-way deletes the fonts already made.
        let mut fonts = Self(Vec::with_capacity(TYPE_RAMP.len()));
        for (style, size, weight) in TYPE_RAMP {
            let mut face = metrics.lfMessageFont;
            face.lfHeight = -scale.to_pixels(size * text_scale);
            face.lfWeight = i32::try_from(weight).expect("font weights are at most 1000");
            // SAFETY: `face` is a fully initialized LOGFONTW.
            let font = Owned::new(unsafe { CreateFontIndirectW(&raw const face) })?;
            fonts.0.push((style, font));
        }
        Ok(fonts)
    }

    pub(crate) fn get(&self, style: TextStyle) -> HFONT {
        self.0
            .iter()
            .find_map(|(font_style, font)| (*font_style == style).then(|| font.get()))
            .expect("the type ramp has a font for every text style")
    }
}
