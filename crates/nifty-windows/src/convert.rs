//! Nifty lays out in device-independent pixels (1/96 inch); Win32 places controls in
//! physical pixels, so every size and position crosses this boundary.

use std::ptr;

use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::USER_DEFAULT_SCREEN_DPI;

use nifty_runtime::{Frame, Size};

/// A window's DPI, which converts between Nifty's DIPs and that window's pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Scale {
    dpi: u32,
}

impl Scale {
    pub(crate) fn new(dpi: u32) -> Self {
        Self { dpi }
    }

    pub(crate) fn dpi(self) -> u32 {
        self.dpi
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "on-screen coordinates are far inside i32 once rounded"
    )]
    pub(crate) fn to_pixels(self, dips: f32) -> i32 {
        (dips * self.pixels_per_dip()).round() as i32
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "on-screen pixel counts fit f32 exactly"
    )]
    pub(crate) fn to_dips(self, pixels: i32) -> f32 {
        pixels as f32 / self.pixels_per_dip()
    }

    pub(crate) fn size_to_dips(self, width: i32, height: i32) -> Size {
        Size::new(self.to_dips(width), self.to_dips(height))
    }

    /// Rounds each edge rather than the origin and size, so neighbouring frames that touch
    /// in DIPs still touch in pixels.
    pub(crate) fn frame_to_pixels(self, frame: Frame) -> RECT {
        RECT {
            left: self.to_pixels(frame.origin.x),
            top: self.to_pixels(frame.origin.y),
            right: self.to_pixels(frame.origin.x + frame.size.width),
            bottom: self.to_pixels(frame.origin.y + frame.size.height),
        }
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "DPI values are small integers that f32 holds exactly"
    )]
    fn pixels_per_dip(self) -> f32 {
        self.dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32
    }
}

pub(crate) fn rect_size(rect: &RECT) -> (i32, i32) {
    (rect.right - rect.left, rect.bottom - rect.top)
}

/// A NUL-terminated UTF-16 copy of `text`, the form Win32 takes strings in.
pub(crate) fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain([0]).collect()
}

/// Win32 packs two 16-bit values into one message parameter.
#[expect(
    clippy::cast_possible_truncation,
    reason = "the mask keeps exactly the low 16 bits"
)]
pub(crate) fn low_word(value: usize) -> u16 {
    (value & 0xFFFF) as u16
}

pub(crate) fn high_word(value: usize) -> u16 {
    low_word(value >> 16)
}

/// Passes `value` to a message that writes its result through a pointer parameter.
pub(crate) fn out_param<T>(value: &mut T) -> LPARAM {
    LPARAM(ptr::from_mut(value).expose_provenance().cast_signed())
}

/// Passes a handle, such as a font, as a message's `WPARAM`.
pub(crate) fn handle_param(handle: *mut core::ffi::c_void) -> WPARAM {
    WPARAM(handle.expose_provenance())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nifty_runtime::Point;

    #[test]
    fn dips_are_pixels_at_the_default_dpi_and_scale_with_it() {
        assert_eq!(Scale::new(96).to_pixels(10.0), 10);
        assert_eq!(Scale::new(144).to_pixels(10.0), 15);
        assert!((Scale::new(192).to_dips(30) - 15.0).abs() < f32::EPSILON);
    }

    #[test]
    fn touching_frames_still_touch_after_rounding() {
        let scale = Scale::new(120);
        let left = Frame {
            origin: Point { x: 0.0, y: 0.0 },
            size: Size::new(10.3, 5.0),
        };
        let right = Frame {
            origin: Point { x: 10.3, y: 0.0 },
            size: Size::new(10.3, 5.0),
        };
        assert_eq!(
            scale.frame_to_pixels(left).right,
            scale.frame_to_pixels(right).left
        );
    }

    #[test]
    fn message_words_split_a_parameter() {
        assert_eq!(low_word(0x1234_5678), 0x5678);
        assert_eq!(high_word(0x1234_5678), 0x1234);
    }
}
