use objc2_foundation::{NSPoint, NSRect, NSSize};

use nifty_runtime::{Frame, Size};

pub(crate) fn to_ns_size(size: Size) -> NSSize {
    NSSize::new(f64::from(size.width), f64::from(size.height))
}

pub(crate) fn to_ns_rect(frame: Frame) -> NSRect {
    NSRect::new(
        NSPoint::new(f64::from(frame.origin.x), f64::from(frame.origin.y)),
        to_ns_size(frame.size),
    )
}

pub(crate) fn to_size(size: NSSize) -> Size {
    Size::new(to_points(size.width), to_points(size.height))
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "AppKit uses f64 points and Taffy uses f32; on-screen sizes fit f32 exactly"
)]
pub(crate) fn to_points(value: f64) -> f32 {
    value as f32
}
