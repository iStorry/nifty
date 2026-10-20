//! The native control behind each Nifty widget, and the objects that report its events.

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, Message};
use objc2_app_kit::{
    NSBezelStyle, NSBorderType, NSBox, NSBoxType, NSButton, NSCellImagePosition, NSColor,
    NSControlSize, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSFontTextStyleBody,
    NSFontTextStyleCaption1, NSFontTextStyleHeadline, NSFontTextStyleLargeTitle,
    NSFontTextStyleTitle1, NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSScroller,
    NSScrollerStyle, NSTextAlignment, NSTextField, NSTextFieldBezelStyle, NSTitlePosition,
    NSUserInterfaceLayoutDirection, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView,
};
use objc2_foundation::{NSDictionary, NSSize, NSString, ns_string};

use nifty_runtime::{
    ButtonStyle, ClickHandler, Frame, Icon, LabelStyle, Size, Surface, TextAlign, TextColor,
    TextHandler, TextStyle, ToggleHandler,
};

use crate::classes::{ClickTarget, FlippedView, TextFieldDelegate, ToggleTarget};
use crate::convert::{to_double, to_ns_rect, to_ns_size, to_points, to_size};
use crate::icons::symbol;
use crate::nav_item::NavItem;

/// Text fields have no useful natural width when empty, so they start at least this wide.
const MIN_TEXT_FIELD_WIDTH: f32 = 200.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);
/// A progress bar has no natural width, so it starts at least this wide.
const MIN_PROGRESS_WIDTH: f32 = 100.0;
/// A progress bar is full at this value.
const FULL_PROGRESS: f64 = 100.0;
/// Matches the corner radius of macOS's own grouped panels, such as System Settings'.
const CARD_CORNER_RADIUS: f64 = 10.0;
const CARD_BORDER_WIDTH: f64 = 1.0;

#[derive(Debug)]
pub struct MacWidget(WidgetKind);

/// `AppKit` holds targets and delegates weakly, so each widget owns its own.
#[derive(Debug)]
enum WidgetKind {
    Label(Retained<NSTextField>),
    Button {
        button: Retained<NSButton>,
        target: Retained<ClickTarget>,
        /// Shows only its icon, so its title is the tooltip.
        icon_only: bool,
    },
    TextField {
        field: Retained<NSTextField>,
        delegate: Retained<TextFieldDelegate>,
    },
    Checkbox {
        checkbox: Retained<NSButton>,
        target: Retained<ToggleTarget>,
    },
    NavItem(NavItem),
    Progress(Retained<NSProgressIndicator>),
    Divider(Retained<NSBox>),
    Surface(Retained<NSView>),
    /// Its content is a flipped view, so content starts at the top as Nifty lays it out.
    Scroll {
        view: Retained<NSScrollView>,
        content: Retained<FlippedView>,
    },
}

impl MacWidget {
    pub(crate) fn label(mtm: MainThreadMarker, text: &str, style: LabelStyle) -> Self {
        let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
        label.setFont(Some(&system_font(style.font)));
        label.setTextColor(Some(&text_color(style.color)));
        label.setAlignment(text_alignment(
            style.align,
            label.userInterfaceLayoutDirection(),
        ));
        label.setSelectable(style.selectable);
        Self(WidgetKind::Label(label))
    }

    pub(crate) fn button(
        mtm: MainThreadMarker,
        title: &str,
        style: ButtonStyle,
        icon: Icon,
        on_click: ClickHandler,
    ) -> Self {
        let target = ClickTarget::new(mtm, on_click);
        let button = click_button(mtm, title, &target);
        if let Some(symbol) = symbol(icon) {
            button.setImage(Some(&symbol));
            button.setImagePosition(NSCellImagePosition::ImageLeading);
        }
        let icon_only = style == ButtonStyle::Icon;
        match style {
            // Return is the key equivalent of a window's default button, drawn in the accent color.
            ButtonStyle::Primary => button.setKeyEquivalent(ns_string!("\r")),
            ButtonStyle::Plain => {
                border_on_hover(&button);
                button.setContentTintColor(Some(&NSColor::controlAccentColor()));
            }
            ButtonStyle::Icon => {
                border_on_hover(&button);
                button.setImagePosition(NSCellImagePosition::ImageOnly);
                button.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
                button.setToolTip(Some(&NSString::from_str(title)));
            }
            ButtonStyle::Normal => {}
        }
        Self(WidgetKind::Button {
            button,
            target,
            icon_only,
        })
    }

    pub(crate) fn nav_item(
        mtm: MainThreadMarker,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> Self {
        Self(WidgetKind::NavItem(NavItem::new(
            mtm, title, icon, badge, selected, on_click,
        )))
    }

    pub(crate) fn text_field(
        mtm: MainThreadMarker,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> Self {
        let field = NSTextField::textFieldWithString(&NSString::from_str(text), mtm);
        field.setBezelStyle(NSTextFieldBezelStyle::RoundedBezel);
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

    /// A bar filled to `percent`, or an animated one while work goes on without one.
    pub(crate) fn progress(mtm: MainThreadMarker, percent: Option<i64>) -> Self {
        let bar = NSProgressIndicator::new(mtm);
        bar.setStyle(NSProgressIndicatorStyle::Bar);
        bar.setMinValue(0.0);
        bar.setMaxValue(FULL_PROGRESS);
        bar.setIndeterminate(percent.is_none());
        match percent {
            Some(percent) => bar.setDoubleValue(to_double(percent)),
            // SAFETY: `startAnimation:` takes any sender, and nil is allowed.
            None => unsafe { bar.startAnimation(None) },
        }
        Self(WidgetKind::Progress(bar))
    }

    pub(crate) fn divider(mtm: MainThreadMarker) -> Self {
        let divider = NSBox::new(mtm);
        divider.setBoxType(NSBoxType::Separator);
        Self(WidgetKind::Divider(divider))
    }

    pub(crate) fn surface(mtm: MainThreadMarker, surface: Surface) -> Self {
        let view = match surface {
            Surface::Sidebar => {
                Retained::into_super(behind_window(mtm, NSVisualEffectMaterial::Sidebar))
            }
            Surface::Content => {
                Retained::into_super(panel(mtm, &NSColor::controlBackgroundColor()))
            }
            Surface::Card => {
                let card = panel(mtm, &NSColor::controlBackgroundColor());
                card.setCornerRadius(CARD_CORNER_RADIUS);
                card.setBorderWidth(CARD_BORDER_WIDTH);
                card.setBorderColor(&NSColor::separatorColor());
                Retained::into_super(card)
            }
            Surface::None => unreachable!("the runtime only creates surfaces it draws"),
        };
        Self(WidgetKind::Surface(view))
    }

    /// A borderless area that scrolls its content up and down, on whatever is behind it.
    /// Where scroll bars take space, the bar always shows, in the strip kept for it.
    pub(crate) fn scroll(mtm: MainThreadMarker) -> Self {
        let view = NSScrollView::new(mtm);
        view.setBorderType(NSBorderType::NoBorder);
        view.setDrawsBackground(false);
        view.setHasVerticalScroller(true);
        // Legacy scroll bars stay in the strip kept for them; overlay ones hide whenever
        // the content isn't scrolling, either way. The scroll view follows the system's
        // preferred style by itself.
        view.setAutohidesScrollers(false);
        let content = FlippedView::new(mtm);
        view.setDocumentView(Some(&content));
        Self(WidgetKind::Scroll { view, content })
    }

    /// The width a scroll bar takes from the content beside it; overlay scroll bars, the
    /// default with a trackpad, take none.
    pub(crate) fn scroll_bar_width(mtm: MainThreadMarker) -> f32 {
        match NSScroller::preferredScrollerStyle(mtm) {
            NSScrollerStyle::Overlay => 0.0,
            style => to_points(NSScroller::scrollerWidthForControlSize_scrollerStyle(
                NSControlSize::Regular,
                style,
                mtm,
            )),
        }
    }

    /// The view a scroll area's content goes in.
    ///
    /// # Panics
    /// When this isn't a scroll area, which only a runtime bug would ask.
    pub(crate) fn content_view(&self) -> &NSView {
        match &self.0 {
            WidgetKind::Scroll { content, .. } => content,
            _ => panic!("only scroll areas hold content"),
        }
    }

    pub(crate) fn set_scroll_content(&self, size: Size) {
        if let WidgetKind::Scroll { content, .. } = &self.0 {
            content.setFrameSize(to_ns_size(size));
        }
    }

    pub(crate) fn view(&self) -> &NSView {
        match &self.0 {
            WidgetKind::Label(label) => label,
            WidgetKind::Button { button, .. } => button,
            WidgetKind::TextField { field, .. } => field,
            WidgetKind::Checkbox { checkbox, .. } => checkbox,
            WidgetKind::NavItem(item) => item.view(),
            WidgetKind::Progress(bar) => bar,
            WidgetKind::Divider(divider) => divider,
            WidgetKind::Surface(view) => view,
            WidgetKind::Scroll { view, .. } => view,
        }
    }

    pub(crate) fn set_text(&self, text: &str) {
        match &self.0 {
            WidgetKind::Label(label) => label.setStringValue(&NSString::from_str(text)),
            WidgetKind::Button {
                button, icon_only, ..
            } => {
                let text = NSString::from_str(text);
                button.setTitle(&text);
                if *icon_only {
                    button.setToolTip(Some(&text));
                }
            }
            WidgetKind::NavItem(item) => item.set_title(text),
            WidgetKind::Checkbox { checkbox, .. } => checkbox.setTitle(&NSString::from_str(text)),
            // Re-setting an unchanged value would move the cursor while the user types.
            WidgetKind::TextField { field, .. } => {
                if field.stringValue().to_string() != text {
                    field.setStringValue(&NSString::from_str(text));
                }
            }
            WidgetKind::Progress(_)
            | WidgetKind::Divider(_)
            | WidgetKind::Surface(_)
            | WidgetKind::Scroll { .. } => {}
        }
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        match &self.0 {
            WidgetKind::Checkbox { checkbox, .. } => checkbox.setState(control_state(checked)),
            WidgetKind::NavItem(item) => item.set_selected(checked),
            _ => {}
        }
    }

    pub(crate) fn set_badge(&self, badge: &str) {
        if let WidgetKind::NavItem(item) = &self.0 {
            item.set_badge(badge);
        }
    }

    pub(crate) fn set_value(&self, value: i64) {
        if let WidgetKind::Progress(bar) = &self.0 {
            bar.setDoubleValue(to_double(value));
        }
    }

    pub(crate) fn measure(&self) -> Size {
        match &self.0 {
            WidgetKind::TextField { .. } => {
                let fitting = to_size(self.view().fittingSize());
                Size::new(fitting.width.max(MIN_TEXT_FIELD_WIDTH), fitting.height)
            }
            WidgetKind::NavItem(item) => item.measure(),
            WidgetKind::Progress(_) => {
                let fitting = to_size(self.view().fittingSize());
                Size::new(fitting.width.max(MIN_PROGRESS_WIDTH), fitting.height)
            }
            WidgetKind::Divider(_) => DIVIDER_SIZE,
            _ => to_size(self.view().fittingSize()),
        }
    }

    pub(crate) fn set_frame(&self, frame: Frame) {
        match &self.0 {
            WidgetKind::NavItem(item) => item.set_frame(frame),
            _ => self.view().setFrame(to_ns_rect(frame)),
        }
    }

    /// Takes the control off screen. Its handler may be running right now (a button that
    /// removes itself), so the control and its handler are released at the end of the
    /// current event instead of immediately.
    pub(crate) fn remove(self) {
        self.view().removeFromSuperview();
        match self.0 {
            WidgetKind::Label(label) => release_after_event(label),
            WidgetKind::NavItem(item) => item.remove(),
            WidgetKind::Button { button, target, .. } => {
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
            WidgetKind::Progress(bar) => {
                // SAFETY: `stopAnimation:` takes any sender, and nil is allowed.
                unsafe { bar.stopAnimation(None) };
                release_after_event(bar);
            }
            WidgetKind::Divider(divider) => release_after_event(divider),
            WidgetKind::Surface(view) => release_after_event(view),
            WidgetKind::Scroll { view, content } => {
                release_after_event(view);
                release_after_event(content);
            }
        }
    }
}

/// A translucent `material` that shows the desktop through the window and dims with the
/// window when it is inactive.
pub(crate) fn behind_window(
    mtm: MainThreadMarker,
    material: NSVisualEffectMaterial,
) -> Retained<NSVisualEffectView> {
    let view = NSVisualEffectView::new(mtm);
    view.setMaterial(material);
    view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    view.setState(NSVisualEffectState::FollowsWindowActiveState);
    view
}

/// A plain filled box. `NSBox` resolves its colors when it draws, so it follows dark mode
/// and accessibility contrast without being told.
pub(crate) fn panel(mtm: MainThreadMarker, fill: &NSColor) -> Retained<NSBox> {
    let panel = NSBox::new(mtm);
    panel.setBoxType(NSBoxType::Custom);
    panel.setTitlePosition(NSTitlePosition::NoTitle);
    panel.setContentViewMargins(NSSize::ZERO);
    panel.setBorderWidth(0.0);
    panel.setFillColor(fill);
    panel
}

/// Draws no bezel until the pointer is over the button, as toolbar buttons do.
fn border_on_hover(button: &NSButton) {
    button.setBezelStyle(NSBezelStyle::AccessoryBar);
    button.setShowsBorderOnlyWhileMouseInside(true);
}

/// A push button that tells `target` when it is clicked.
pub(crate) fn click_button(
    mtm: MainThreadMarker,
    title: &str,
    target: &ClickTarget,
) -> Retained<NSButton> {
    // SAFETY: `ClickTarget` implements the action selector, and the widget that owns the
    // button keeps its target alive.
    unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(title),
            Some(target),
            Some(ClickTarget::action()),
            mtm,
        )
    }
}

/// Hands the object to the run loop's autorelease pool, which drains after each event.
pub(crate) fn release_after_event<T: Message>(object: Retained<T>) {
    let _ = Retained::autorelease_ptr(object);
}

fn control_state(checked: bool) -> isize {
    if checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    }
}

pub(crate) fn system_font(style: TextStyle) -> Retained<NSFont> {
    // SAFETY: AppKit defines these text style names as immutable constants.
    let text_style = unsafe {
        match style {
            TextStyle::Body => NSFontTextStyleBody,
            TextStyle::Title => NSFontTextStyleTitle1,
            TextStyle::Headline => NSFontTextStyleHeadline,
            TextStyle::Caption => NSFontTextStyleCaption1,
            TextStyle::LargeTitle => NSFontTextStyleLargeTitle,
        }
    };
    // SAFETY: an empty options dictionary asks for the style's default font.
    unsafe { NSFont::preferredFontForTextStyle_options(text_style, &NSDictionary::new()) }
}

fn text_color(color: TextColor) -> Retained<NSColor> {
    match color {
        TextColor::Primary => NSColor::labelColor(),
        TextColor::Secondary => NSColor::secondaryLabelColor(),
        TextColor::Tertiary => NSColor::tertiaryLabelColor(),
        TextColor::Accent => NSColor::controlAccentColor(),
        TextColor::Success => NSColor::systemGreenColor(),
        TextColor::Warning => NSColor::systemOrangeColor(),
        TextColor::Danger => NSColor::systemRedColor(),
    }
}

/// `AppKit` has a natural alignment but no opposite of it, so the end follows the direction.
fn text_alignment(align: TextAlign, direction: NSUserInterfaceLayoutDirection) -> NSTextAlignment {
    match align {
        TextAlign::Start => NSTextAlignment::Natural,
        TextAlign::Center => NSTextAlignment::Center,
        TextAlign::End if direction == NSUserInterfaceLayoutDirection::RightToLeft => {
            NSTextAlignment::Left
        }
        TextAlign::End => NSTextAlignment::Right,
    }
}
