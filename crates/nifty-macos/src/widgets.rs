//! The native control behind each Nifty widget, and the objects that report its events.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSBox, NSBoxType, NSButton, NSColor, NSControlStateValueOff, NSControlStateValueOn, NSFont,
    NSFontTextStyleBody, NSFontTextStyleCaption1, NSFontTextStyleHeadline, NSFontTextStyleTitle1,
    NSTextField, NSView,
};
use objc2_foundation::{NSDictionary, NSString, ns_string};

use nifty_runtime::{ButtonStyle, ClickHandler, Size, TextHandler, TextStyle, ToggleHandler};

use crate::classes::{ClickTarget, TextFieldDelegate, ToggleTarget};
use crate::convert::to_size;

/// Text fields have no useful natural width when empty, so they start at least this wide.
const MIN_TEXT_FIELD_WIDTH: f32 = 200.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);

#[derive(Debug)]
pub struct MacWidget(WidgetKind);

/// `AppKit` holds targets and delegates weakly, so each widget owns its own.
#[derive(Debug)]
enum WidgetKind {
    Label(Retained<NSTextField>),
    Button {
        button: Retained<NSButton>,
        target: Retained<ClickTarget>,
    },
    TextField {
        field: Retained<NSTextField>,
        delegate: Retained<TextFieldDelegate>,
    },
    Checkbox {
        checkbox: Retained<NSButton>,
        target: Retained<ToggleTarget>,
    },
    Divider(Retained<NSBox>),
}

impl MacWidget {
    pub(crate) fn label(mtm: MainThreadMarker, text: &str, style: TextStyle) -> Self {
        let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        label.setFont(Some(&system_font(style)));
        if style == TextStyle::Caption {
            label.setTextColor(Some(&NSColor::secondaryLabelColor()));
        }
        Self(WidgetKind::Label(label))
    }

    pub(crate) fn button(
        mtm: MainThreadMarker,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> Self {
        let target = ClickTarget::new(mtm, on_click);
        // SAFETY: `ClickTarget` implements the action selector, and this widget keeps it alive.
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(&target),
                Some(ClickTarget::action()),
                mtm,
            )
        };
        if style == ButtonStyle::Primary {
            // Return is the key equivalent of a window's default button, drawn in the accent color.
            button.setKeyEquivalent(ns_string!("\r"));
        }
        Self(WidgetKind::Button { button, target })
    }

    pub(crate) fn text_field(
        mtm: MainThreadMarker,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> Self {
        let field = NSTextField::textFieldWithString(&NSString::from_str(text), mtm);
        field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
        let delegate = TextFieldDelegate::new(mtm, &field, on_change);
        // SAFETY: the delegate lives as long as this widget, which owns both.
        unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        Self(WidgetKind::TextField { field, delegate })
    }

    pub(crate) fn checkbox(
        mtm: MainThreadMarker,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> Self {
        let target = ToggleTarget::new(mtm, on_toggle);
        // SAFETY: `ToggleTarget` implements the action selector, and this widget keeps it alive.
        let checkbox = unsafe {
            NSButton::checkboxWithTitle_target_action(
                &NSString::from_str(title),
                Some(&target),
                Some(ToggleTarget::action()),
                mtm,
            )
        };
        checkbox.setState(control_state(checked));
        Self(WidgetKind::Checkbox { checkbox, target })
    }

    pub(crate) fn divider(mtm: MainThreadMarker) -> Self {
        let divider = NSBox::new(mtm);
        divider.setBoxType(NSBoxType::Separator);
        Self(WidgetKind::Divider(divider))
    }

    pub(crate) fn view(&self) -> &NSView {
        match &self.0 {
            WidgetKind::Label(label) => label,
            WidgetKind::Button { button, .. } => button,
            WidgetKind::TextField { field, .. } => field,
            WidgetKind::Checkbox { checkbox, .. } => checkbox,
            WidgetKind::Divider(divider) => divider,
        }
    }

    pub(crate) fn set_text(&self, text: &str) {
        match &self.0 {
            WidgetKind::Label(label) => label.setStringValue(&NSString::from_str(text)),
            WidgetKind::Button { button, .. } => button.setTitle(&NSString::from_str(text)),
            WidgetKind::Checkbox { checkbox, .. } => checkbox.setTitle(&NSString::from_str(text)),
            // Re-setting an unchanged value would move the cursor while the user types.
            WidgetKind::TextField { field, .. } => {
                if field.stringValue().to_string() != text {
                    field.setStringValue(&NSString::from_str(text));
                }
            }
            WidgetKind::Divider(_) => {}
        }
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        if let WidgetKind::Checkbox { checkbox, .. } = &self.0 {
            checkbox.setState(control_state(checked));
        }
    }

    pub(crate) fn measure(&self) -> Size {
        let fitting = to_size(self.view().fittingSize());
        match &self.0 {
            WidgetKind::TextField { .. } => {
                Size::new(fitting.width.max(MIN_TEXT_FIELD_WIDTH), fitting.height)
            }
            WidgetKind::Divider(_) => DIVIDER_SIZE,
            _ => fitting,
        }
    }

    /// Takes the control off screen. Its handler may be running right now (a button that
    /// removes itself), so the control and its handler are released at the end of the
    /// current event instead of immediately.
    pub(crate) fn remove(self) {
        self.view().removeFromSuperview();
        match self.0 {
            WidgetKind::Label(label) => release_after_event(label),
            WidgetKind::Button { button, target } => {
                release_after_event(button);
                release_after_event(target);
            }
            WidgetKind::TextField { field, delegate } => {
                release_after_event(field);
                release_after_event(delegate);
            }
            WidgetKind::Checkbox { checkbox, target } => {
                release_after_event(checkbox);
                release_after_event(target);
            }
            WidgetKind::Divider(divider) => release_after_event(divider),
        }
    }
}

/// Hands the object to the run loop's autorelease pool, which drains after each event.
fn release_after_event<T: Message>(object: Retained<T>) {
    let _ = Retained::autorelease_ptr(object);
}

fn control_state(checked: bool) -> isize {
    if checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    }
}

fn system_font(style: TextStyle) -> Retained<NSFont> {
    // SAFETY: AppKit defines these text style names as immutable constants.
    let text_style = unsafe {
        match style {
            TextStyle::Body => NSFontTextStyleBody,
            TextStyle::Title => NSFontTextStyleTitle1,
            TextStyle::Headline => NSFontTextStyleHeadline,
            TextStyle::Caption => NSFontTextStyleCaption1,
        }
    };
    // SAFETY: an empty options dictionary asks for the style's default font.
    unsafe { NSFont::preferredFontForTextStyle_options(text_style, &NSDictionary::new()) }
}
