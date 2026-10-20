//! A sidebar row the way Finder draws one: an accent-colored symbol and a title, with a
//! quieter badge such as a count at the end, on a rounded highlight that shows the row is
//! selected, pressed or under the pointer.

use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use objc2::rc::{Retained, Weak};
use objc2::{AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSAccessibility, NSBox, NSButton, NSColor, NSControl, NSEvent, NSImageView, NSLineBreakMode,
    NSResponder, NSTextAlignment, NSTextField, NSTrackingArea, NSTrackingAreaOptions, NSView,
};
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};

use nifty_runtime::{ClickHandler, Frame, Icon, Size, TextStyle};

use crate::classes::ClickTarget;
use crate::convert::{centered, to_ns_rect, to_points, to_size};
use crate::icons::tinted_symbol;
use crate::widgets::{panel, release_after_event, system_font};

/// The height and corner radius of Finder's sidebar rows, the space at each end, the
/// symbol's square and the space after it, in points.
const ROW_HEIGHT: f32 = 28.0;
const CORNER_RADIUS: f64 = 6.0;
const INSET: f32 = 8.0;
const ICON_SIZE: f32 = 16.0;
const ICON_GAP: f32 = 6.0;
/// How strongly the text color shows through the row while under the pointer, and while
/// pressed. Tints of the label color follow dark mode.
const HOVER_TINT: f64 = 0.06;
const PRESSED_TINT: f64 = 0.12;

/// The row's views, under a button that covers the whole row and draws nothing itself:
/// it takes clicks anywhere on the row, tracks the pointer, and carries the title for
/// screen readers.
#[derive(Debug)]
pub(crate) struct NavItem {
    row: Retained<NSBox>,
    icon: Option<Retained<NSImageView>>,
    title: Retained<NSTextField>,
    badge: Retained<NSTextField>,
    button: Retained<RowButton>,
    target: Retained<ClickTarget>,
    look: Rc<RowLook>,
}

impl NavItem {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> Self {
        let row = panel(mtm, &NSColor::clearColor());
        row.setCornerRadius(CORNER_RADIUS);
        let look = Rc::new(RowLook {
            row: Weak::from_retained(&row),
            selected: Cell::new(selected),
            hovered: Cell::new(false),
            pressed: Cell::new(false),
        });

        let icon = tinted_symbol(icon, &NSColor::controlAccentColor())
            .map(|symbol| NSImageView::imageViewWithImage(&symbol, mtm));
        let title_label = label(mtm, title, &NSColor::labelColor());
        title_label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
        let badge_label = label(mtm, badge, &NSColor::secondaryLabelColor());
        badge_label.setAlignment(NSTextAlignment::Right);
        let target = ClickTarget::new(mtm, on_click);
        let button = RowButton::new(mtm, title, &target, Rc::clone(&look));

        let content = row.contentView().expect("a box has a content view");
        if let Some(icon) = &icon {
            content.addSubview(icon);
        }
        content.addSubview(&title_label);
        content.addSubview(&badge_label);
        content.addSubview(&button);

        let item = Self {
            row,
            icon,
            title: title_label,
            badge: badge_label,
            button,
            target,
            look,
        };
        item.look.show();
        item.set_badge(badge);
        item
    }

    pub(crate) fn view(&self) -> &NSView {
        &self.row
    }

    pub(crate) fn set_title(&self, title: &str) {
        let title = NSString::from_str(title);
        self.title.setStringValue(&title);
        self.button.setAccessibilityLabel(Some(&title));
    }

    pub(crate) fn set_selected(&self, selected: bool) {
        self.look.selected.set(selected);
        self.look.show();
    }

    pub(crate) fn set_badge(&self, badge: &str) {
        self.badge.setStringValue(&NSString::from_str(badge));
        self.badge.setHidden(badge.is_empty());
    }

    pub(crate) fn measure(&self) -> Size {
        let title = to_size(self.title.fittingSize());
        Size::new(
            INSET + self.icon_width() + title.width + self.badge_width() + INSET,
            ROW_HEIGHT.max(title.height),
        )
    }

    /// Places the row, then lays out its views across it, each centered vertically.
    pub(crate) fn set_frame(&self, frame: Frame) {
        self.row.setFrame(to_ns_rect(frame));
        let height = frame.size.height;
        let mut left = INSET;
        if let Some(icon) = &self.icon {
            icon.setFrame(centered(
                left,
                ICON_SIZE,
                Size::new(ICON_SIZE, ICON_SIZE),
                height,
            ));
            left += ICON_SIZE + ICON_GAP;
        }
        let badge = to_size(self.badge.fittingSize());
        let badge_left = frame.size.width - INSET - badge.width;
        self.badge
            .setFrame(centered(badge_left, badge.width, badge, height));
        let title_height = to_size(self.title.fittingSize()).height;
        let title_width = (frame.size.width - left - INSET - self.badge_width()).max(0.0);
        self.title.setFrame(centered(
            left,
            title_width,
            Size::new(title_width, title_height),
            height,
        ));
        self.button.setFrame(NSRect::new(
            NSPoint::ZERO,
            NSSize::new(f64::from(frame.size.width), f64::from(height)),
        ));
    }

    /// Released at the end of the current event, since the click may be running now.
    pub(crate) fn remove(self) {
        self.row.removeFromSuperview();
        release_after_event(self.row);
        if let Some(icon) = self.icon {
            release_after_event(icon);
        }
        release_after_event(self.title);
        release_after_event(self.badge);
        release_after_event(self.button);
        release_after_event(self.target);
    }

    fn icon_width(&self) -> f32 {
        if self.icon.is_some() {
            ICON_SIZE + ICON_GAP
        } else {
            0.0
        }
    }

    fn badge_width(&self) -> f32 {
        if self.badge.isHidden() {
            0.0
        } else {
            INSET + to_points(self.badge.fittingSize().width)
        }
    }
}

fn label(mtm: MainThreadMarker, text: &str, color: &NSColor) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&system_font(TextStyle::Body)));
    label.setTextColor(Some(color));
    label
}

/// What the row's highlight shows. The row is held weakly, since the row owns the button
/// that owns this.
pub(crate) struct RowLook {
    row: Weak<NSBox>,
    selected: Cell<bool>,
    hovered: Cell<bool>,
    pressed: Cell<bool>,
}

impl RowLook {
    /// Pressing shows over selection, which shows over hovering.
    fn show(&self) {
        let Some(row) = self.row.load() else {
            return;
        };
        let fill = if self.pressed.get() {
            Some(NSColor::labelColor().colorWithAlphaComponent(PRESSED_TINT))
        } else if self.selected.get() {
            Some(NSColor::unemphasizedSelectedContentBackgroundColor())
        } else if self.hovered.get() {
            Some(NSColor::labelColor().colorWithAlphaComponent(HOVER_TINT))
        } else {
            None
        };
        match fill {
            Some(fill) => {
                row.setFillColor(&fill);
                row.setTransparent(false);
            }
            None => row.setTransparent(true),
        }
    }

    fn set(&self, state: &Cell<bool>, on: bool) {
        if state.replace(on) != on {
            self.show();
        }
    }
}

impl fmt::Debug for RowLook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RowLook")
            .field("selected", &self.selected)
            .field("hovered", &self.hovered)
            .field("pressed", &self.pressed)
            .finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSButton has no subclassing requirements, and `RowButton` does not implement
    // Drop. The overrides call through to NSButton's own handling.
    #[unsafe(super(NSButton, NSControl, NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyRowButton"]
    #[ivars = Rc<RowLook>]
    #[derive(Debug)]
    pub(crate) struct RowButton;

    impl RowButton {
        #[unsafe(method(mouseEntered:))]
        fn mouse_entered(&self, _event: &NSEvent) {
            let look = self.ivars();
            look.set(&look.hovered, true);
        }

        #[unsafe(method(mouseExited:))]
        fn mouse_exited(&self, _event: &NSEvent) {
            let look = self.ivars();
            look.set(&look.hovered, false);
        }

        /// NSButton tracks the click inside `mouseDown:` and returns once the mouse is up.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let look = Rc::clone(self.ivars());
            look.set(&look.pressed, true);
            // SAFETY: NSButton's `mouseDown:` takes the event that was passed here.
            let () = unsafe { msg_send![super(self), mouseDown: event] };
            look.set(&look.pressed, false);
        }
    }
);

impl RowButton {
    fn new(
        mtm: MainThreadMarker,
        title: &str,
        target: &ClickTarget,
        look: Rc<RowLook>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(look);
        // SAFETY: `initWithFrame:` is NSView's designated initializer.
        let button: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: NSRect::ZERO] };
        button.setBordered(false);
        button.setTitle(&NSString::new());
        button.setAccessibilityLabel(Some(&NSString::from_str(title)));
        // SAFETY: `ClickTarget` implements the action selector, and the nav item that owns
        // the button keeps its target alive.
        unsafe {
            button.setTarget(Some(target));
            button.setAction(Some(ClickTarget::action()));
        }
        let options = NSTrackingAreaOptions::MouseEnteredAndExited
            | NSTrackingAreaOptions::ActiveInActiveApp
            | NSTrackingAreaOptions::InVisibleRect;
        // SAFETY: the button owns the area it tracks, and `InVisibleRect` keeps the area's
        // rect in step with the button, so the rect passed is ignored.
        let area = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                NSRect::ZERO,
                options,
                Some(&button),
                None,
            )
        };
        button.addTrackingArea(&area);
        button
    }
}
