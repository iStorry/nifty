//! The system UI font at each Nifty text style, created for one window's DPI.

use std::mem;

use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, FW_NORMAL, FW_SEMIBOLD, GetDC, GetTextFaceW, HFONT, LOGFONTW, ReleaseDC,
};
use windows::Win32::UI::HiDpi::SystemParametersInfoForDpi;
use windows::Win32::UI::WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS};
use windows::core::Result;

use nifty_runtime::TextStyle;

use crate::convert::Scale;
use crate::convert::wide;
use crate::gdi::{Owned, Selected};
use crate::settings::STANDARD_TEXT_SIZE;

/// Each style's size in DIPs and weight, from the Windows 11 type ramp.
const TYPE_RAMP: [(TextStyle, f32, u32); 5] = [
    (TextStyle::Body, 14.0, FW_NORMAL.0),
    (TextStyle::Title, 28.0, FW_SEMIBOLD.0),
    (TextStyle::Headline, 14.0, FW_SEMIBOLD.0),
    (TextStyle::Caption, 12.0, FW_NORMAL.0),
    (TextStyle::LargeTitle, 40.0, FW_SEMIBOLD.0),
];

/// Windows 11's icon font, then Windows 10's, which shares its code points.
const ICON_FACES: [&str; 2] = ["Segoe Fluent Icons", "Segoe MDL2 Assets"];
/// Fluent draws icons in controls at this size, in DIPs.
const ICON_SIZE: f32 = 16.0;

#[derive(Debug)]
pub(crate) struct Fonts {
    text: Vec<(TextStyle, Owned<HFONT>)>,
    icons: Owned<HFONT>,
}

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
        let mut text = Vec::with_capacity(TYPE_RAMP.len());
        for (style, size, weight) in TYPE_RAMP {
            let mut face = metrics.lfMessageFont;
            face.lfHeight = -scale.to_pixels(size * text_scale);
            face.lfWeight = i32::try_from(weight).expect("font weights are at most 1000");
            text.push((style, create_font(&face)?));
        }
        let mut icon_face = metrics.lfMessageFont;
        icon_face.lfHeight = -scale.to_pixels(ICON_SIZE * text_scale);
        Ok(Self {
            text,
            icons: icon_font(icon_face)?,
        })
    }

    pub(crate) fn get(&self, style: TextStyle) -> HFONT {
        self.text
            .iter()
            .find_map(|(font_style, font)| (*font_style == style).then(|| font.get()))
            .expect("the type ramp has a font for every text style")
    }

    /// The font whose glyphs are Nifty's icons.
    pub(crate) fn icons(&self) -> HFONT {
        self.icons.get()
    }
}

/// The first of `ICON_FACES` this system has. Windows substitutes another font for a face
/// it lacks, so each one is checked by the face it actually made.
fn icon_font(mut face: LOGFONTW) -> Result<Owned<HFONT>> {
    let mut fallback = None;
    for name in ICON_FACES {
        set_face_name(&mut face, name);
        let font = create_font(&face)?;
        if face_of(&font) == name {
            return Ok(font);
        }
        fallback.get_or_insert(font);
    }
    Ok(fallback.expect("there is at least one icon face"))
}

fn set_face_name(face: &mut LOGFONTW, name: &str) {
    let name = wide(name);
    face.lfFaceName = [0; 32];
    face.lfFaceName[..name.len()].copy_from_slice(&name);
}

fn face_of(font: &Owned<HFONT>) -> String {
    // SAFETY: a null window asks for the screen's device context, released below.
    let dc = unsafe { GetDC(None) };
    let mut name = [0_u16; 32];
    let length = {
        let _font = Selected::new(dc, font.get());
        // SAFETY: `name` is a buffer of the length passed, and `dc` has the font selected.
        unsafe { GetTextFaceW(dc, Some(&mut name)) }
    };
    // SAFETY: `dc` came from `GetDC` above and is released once.
    unsafe { ReleaseDC(None, dc) };
    let length = usize::try_from(length).unwrap_or(0).saturating_sub(1);
    String::from_utf16_lossy(&name[..length])
}

fn create_font(face: &LOGFONTW) -> Result<Owned<HFONT>> {
    // SAFETY: `face` is a fully initialized LOGFONTW.
    Owned::new(unsafe { CreateFontIndirectW(face) })
}
