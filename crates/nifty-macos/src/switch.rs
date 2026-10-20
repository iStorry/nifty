//! A switch with its title, laid out as System Settings does: the title at the start of
//! the row, and the switch at its end.

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_app_kit::{
    NSAccessibility, NSControlStateValueOff, NSControlStateValueOn, NSLineBreakMode, NSSwitch,
    NSTextField, NSView,
};
use objc2_foundation::NSString;

use nifty_runtime::{Frame, Size, TextStyle, ToggleHandler};

use crate::classes::{FlippedView, ToggleTarget};
use crate::convert::{centered, to_ns_rect, to_size};
use crate::widgets::{release_after_event, system_font};

/// The space between the title and the switch, in points.
const GAP: f32 = 12.0;

#[derive(Debug)]
pub(crate) struct SwitchRow {
    row: Retained<FlippedView>,
    title: Retained<NSTextField>,
    switch: Retained<NSSwitch>,
    target: Retained<ToggleTarget>,
}

impl SwitchRow {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        title: &str,
        on: bool,
        on_toggle: ToggleHandler,
    ) -> Self {
        let row = FlippedView::new(mtm);
        let title_label = NSTextField::labelWithString(&NSString::from_str(title), mtm);
        title_label.setFont(Some(&system_font(TextStyle::Body)));
        title_label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let target = ToggleTarget::new(mtm, on_toggle);
        let switch = NSSwitch::new(mtm);
        // SAFETY: `ToggleTarget` implements the action selector, and the row that owns the
        // switch keeps its target alive.
        unsafe {
            switch.setTarget(Some(&target));
            switch.setAction(Some(ToggleTarget::action()));
        }
        row.addSubview(&title_label);
        row.addSubview(&switch);
        let item = Self {
            row,
            title: title_label,
            switch,
            target,
        };
        item.set_title(title);
        item.set_on(on);
        item
    }

    pub(crate) fn view(&self) -> &NSView {
        &self.row
    }

    /// The switch's screen reader label is its title, which sits beside it as a label.
    pub(crate) fn set_title(&self, title: &str) {
        let title = NSString::from_str(title);
        self.title.setStringValue(&title);
        self.title.setHidden(title.length() == 0);
        self.switch.setAccessibilityLabel(Some(&title));
    }

    pub(crate) fn set_on(&self, on: bool) {
        self.switch.setState(if on {
            NSControlStateValueOn
        } else {
            NSControlStateValueOff
        });
    }

    pub(crate) fn measure(&self) -> Size {
        let switch = to_size(self.switch.fittingSize());
        let title = self.title_size();
        let gap = if title.width > 0.0 { GAP } else { 0.0 };
        Size::new(
            title.width + gap + switch.width,
            title.height.max(switch.height),
        )
    }

    /// Places the row, with its title at the start and its switch at the end, each
    /// centered vertically.
    pub(crate) fn set_frame(&self, frame: Frame) {
        self.row.setFrame(to_ns_rect(frame));
        let height = frame.size.height;
        let switch = to_size(self.switch.fittingSize());
        let switch_left = frame.size.width - switch.width;
        self.switch
            .setFrame(centered(switch_left, switch.width, switch, height));
        let title_width = (switch_left - GAP).max(0.0);
        self.title
            .setFrame(centered(0.0, title_width, self.title_size(), height));
    }

    /// Released at the end of the current event, since the toggle may be running now.
    pub(crate) fn remove(self) {
        self.row.removeFromSuperview();
        release_after_event(self.row);
        release_after_event(self.title);
        release_after_event(self.switch);
        release_after_event(self.target);
    }

    fn title_size(&self) -> Size {
        if self.title.isHidden() {
            Size::ZERO
        } else {
            to_size(self.title.fittingSize())
        }
    }
}
