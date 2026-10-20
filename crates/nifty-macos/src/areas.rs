//! Areas that cover a stack without drawing anything, such as a context menu's or a drop
//! target's: hidden views among a host's subviews, found by where they are.

use objc2::rc::Retained;
use objc2::{DowncastTarget, MainThreadOnly};
use objc2_app_kit::NSView;
use objc2_foundation::{NSPoint, NSRect};

/// Hides `area`, a fresh view, so it takes no clicks and draws nothing.
pub(crate) fn hidden<T: MainThreadOnly + AsRef<NSView>>(area: Retained<T>) -> Retained<T> {
    (*area).as_ref().setHidden(true);
    area
}

/// The smallest area of type `T` among `host`'s subviews at `point`, in `host`'s
/// coordinates, which is the innermost stack's; of areas the same size, the one added last.
pub(crate) fn innermost<T: DowncastTarget>(host: &NSView, point: NSPoint) -> Option<Retained<T>> {
    let mut found: Option<(f64, Retained<T>)> = None;
    for subview in host.subviews() {
        let frame = subview.frame();
        let Ok(area) = subview.downcast::<T>() else {
            continue;
        };
        if !contains(frame, point) {
            continue;
        }
        let size = frame.size.width * frame.size.height;
        if found.as_ref().is_none_or(|(smallest, _)| size <= *smallest) {
            found = Some((size, area));
        }
    }
    found.map(|(_, area)| area)
}

fn contains(frame: NSRect, point: NSPoint) -> bool {
    let (left, top) = (frame.origin.x, frame.origin.y);
    (left..left + frame.size.width).contains(&point.x)
        && (top..top + frame.size.height).contains(&point.y)
}
