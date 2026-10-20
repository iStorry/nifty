//! The native control behind each Nifty widget, and the objects that report its events.

use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, Message};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSBorderType, NSBox, NSBoxType, NSButton, NSButtonType,
    NSCellImagePosition, NSColor, NSControlSize, NSFont, NSFontTextStyleBody,
    NSFontTextStyleCaption1, NSFontTextStyleHeadline, NSFontTextStyleLargeTitle,
    NSFontTextStyleTitle1, NSImage, NSImageScaling, NSImageView, NSPopUpButton,
    NSProgressIndicator, NSProgressIndicatorStyle, NSScrollView, NSScroller, NSScrollerStyle,
    NSSlider, NSTextAlignment, NSTextField, NSTextFieldBezelStyle, NSTextView, NSTitlePosition,
    NSUserInterfaceLayoutDirection, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView,
};
use objc2_foundation::{NSDictionary, NSRect, NSSize, NSString, ns_string};

use nifty_runtime::{
    ButtonStyle, ClickHandler, DropHandler, Frame, Icon, ImageSize, LabelStyle, MenuEntry,
    MenuHandler, NumberHandler, Size, Surface, TextAlign, TextColor, TextHandler, TextStyle,
    ToggleHandler,
};

use crate::classes::{
    ClickTarget, FlippedView, NumberTarget, PickTarget, TextFieldDelegate, TextViewDelegate,
    ToggleTarget, control_state,
};
use crate::context_menu::MenuArea;
use crate::convert::{to_double, to_ns_rect, to_ns_size, to_points, to_size};
use crate::drop::DropArea;
use crate::icons::symbol;
use crate::nav_item::NavItem;
use crate::switch::SwitchRow;

/// Text fields have no useful natural width when empty, so they start at least this wide.
const MIN_TEXT_FIELD_WIDTH: f32 = 200.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);
/// Progress bars and sliders have no natural width, so they start at least this wide.
const MIN_TRACK_WIDTH: f32 = 100.0;
/// A progress bar is full at this value.
const FULL_PROGRESS: f64 = 100.0;
/// The space between a text area's border and its text, in points.
const TEXT_AREA_INSET: NSSize = NSSize::new(2.0, 4.0);
/// The index that selects no item of a pop-up button.
const NO_ITEM: isize = -1;
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
    /// A text field several lines tall: a text view in a scroll view with a border.
    TextArea {
        scroll: Retained<NSScrollView>,
        text_view: Retained<NSTextView>,
        delegate: Retained<TextViewDelegate>,
        lines: u32,
    },
    Checkbox {
        checkbox: Retained<NSButton>,
        target: Retained<ToggleTarget>,
    },
    Switch(SwitchRow),
    Slider {
        slider: Retained<NSSlider>,
        target: Retained<NumberTarget>,
    },
    Picker {
        popup: Retained<NSPopUpButton>,
        target: Retained<PickTarget>,
    },
    NavItem(NavItem),
    Progress(Retained<NSProgressIndicator>),
    Image {
        view: Retained<NSImageView>,
        size: ImageSize,
    },
    Divider(Retained<NSBox>),
    /// The hidden area a stack's context menu covers.
    ContextMenu(Retained<MenuArea>),
    /// The hidden area a stack's drop target covers.
    DropTarget(Retained<DropArea>),
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

    /// A button made with a selection is an on-off button, shown on while selected.
    pub(crate) fn button(
        mtm: MainThreadMarker,
        title: &str,
        (style, icon): (ButtonStyle, Icon),
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> Self {
        let target = match selected {
            Some(selected) => ClickTarget::selecting(mtm, on_click, selected),
            None => ClickTarget::new(mtm, on_click),
        };
        let button = click_button(mtm, title, &target);
        if let Some(selected) = selected {
            button.setButtonType(NSButtonType::PushOnPushOff);
            button.setState(control_state(selected));
        }
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
                show_title_beside(&button, &NSString::from_str(title));
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
        on_submit: Option<ClickHandler>,
    ) -> Self {
        let field = NSTextField::textFieldWithString(&NSString::from_str(text), mtm);
        field.setBezelStyle(NSTextFieldBezelStyle::RoundedBezel);
        field.setPlaceholderString(Some(&NSString::from_str(placeholder)));
        let delegate = TextFieldDelegate::new(mtm, &field, on_change, on_submit);
        // SAFETY: the delegate lives as long as this widget, which owns both.
        unsafe { field.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };
        Self(WidgetKind::TextField { field, delegate })
    }

    /// Plain text over `lines` lines, which wraps and scrolls; **Return** starts a new line.
    pub(crate) fn text_area(
        mtm: MainThreadMarker,
        text: &str,
        lines: u32,
        on_change: TextHandler,
    ) -> Self {
        let scroll = NSTextView::scrollableTextView(mtm);
        scroll.setBorderType(NSBorderType::BezelBorder);
        let text_view = scroll
            .documentView()
            .and_then(|view| view.downcast::<NSTextView>().ok())
            .expect("a scrollable text view holds a text view");
        text_view.setRichText(false);
        text_view.setAllowsUndo(true);
        text_view.setFont(Some(&system_font(TextStyle::Body)));
        text_view.setTextContainerInset(TEXT_AREA_INSET);
        text_view.setString(&NSString::from_str(text));
        let delegate = TextViewDelegate::new(mtm, &text_view, on_change);
        text_view.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Self(WidgetKind::TextArea {
            scroll,
            text_view,
            delegate,
            lines,
        })
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

    pub(crate) fn switch(
        mtm: MainThreadMarker,
        title: &str,
        on: bool,
        on_toggle: ToggleHandler,
    ) -> Self {
        Self(WidgetKind::Switch(SwitchRow::new(
            mtm, title, on, on_toggle,
        )))
    }

    /// Slides smoothly, reporting the whole number nearest the knob as it changes.
    pub(crate) fn slider(
        mtm: MainThreadMarker,
        value: i64,
        min: i64,
        max: i64,
        on_change: NumberHandler,
    ) -> Self {
        let target = NumberTarget::new(mtm, value, on_change);
        // SAFETY: `NumberTarget` implements the action selector, and this widget keeps it
        // alive.
        let slider = unsafe {
            NSSlider::sliderWithValue_minValue_maxValue_target_action(
                to_double(value),
                to_double(min),
                to_double(max),
                Some(&target),
                Some(NumberTarget::action()),
                mtm,
            )
        };
        slider.setContinuous(true);
        Self(WidgetKind::Slider { slider, target })
    }

    pub(crate) fn picker(
        mtm: MainThreadMarker,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> Self {
        let target = PickTarget::new(mtm, on_pick);
        let popup =
            NSPopUpButton::initWithFrame_pullsDown(NSPopUpButton::alloc(mtm), NSRect::ZERO, false);
        // SAFETY: `PickTarget` implements the action selector, and this widget keeps it
        // alive.
        unsafe {
            popup.setTarget(Some(&target));
            popup.setAction(Some(PickTarget::action()));
        }
        let picker = Self(WidgetKind::Picker { popup, target });
        picker.set_options(options);
        picker.set_text(selected);
        picker
    }

    /// An image view that scales its image to fit, keeping its shape, and centres it.
    pub(crate) fn image(mtm: MainThreadMarker, path: Option<&Path>, size: ImageSize) -> Self {
        let view = NSImageView::new(mtm);
        view.setImageScaling(NSImageScaling::ScaleProportionallyUpOrDown);
        let image = Self(WidgetKind::Image { view, size });
        image.set_image(path);
        image
    }

    /// Shows the image in the file at `path`, or nothing when there is none or it can't be
    /// read.
    pub(crate) fn set_image(&self, path: Option<&Path>) {
        if let WidgetKind::Image { view, .. } = &self.0 {
            let image = path.and_then(|path| {
                let path = NSString::from_str(&path.to_string_lossy());
                NSImage::initWithContentsOfFile(NSImage::alloc(), &path)
            });
            view.setImage(image.as_deref());
        }
    }

    pub(crate) fn divider(mtm: MainThreadMarker) -> Self {
        let divider = NSBox::new(mtm);
        divider.setBoxType(NSBoxType::Separator);
        Self(WidgetKind::Divider(divider))
    }

    pub(crate) fn context_menu(
        mtm: MainThreadMarker,
        entries: &[MenuEntry<usize>],
        on_select: MenuHandler,
    ) -> Self {
        Self(WidgetKind::ContextMenu(MenuArea::new(
            mtm, entries, on_select,
        )))
    }

    pub(crate) fn drop_target(mtm: MainThreadMarker, on_drop: DropHandler) -> Self {
        Self(WidgetKind::DropTarget(DropArea::new(mtm, on_drop)))
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
            WidgetKind::TextArea { scroll, .. } => scroll,
            WidgetKind::Checkbox { checkbox, .. } => checkbox,
            WidgetKind::Switch(row) => row.view(),
            WidgetKind::Slider { slider, .. } => slider,
            WidgetKind::Picker { popup, .. } => popup,
            WidgetKind::NavItem(item) => item.view(),
            WidgetKind::Progress(bar) => bar,
            WidgetKind::Image { view, .. } => view,
            WidgetKind::Divider(divider) => divider,
            WidgetKind::ContextMenu(area) => area,
            WidgetKind::DropTarget(area) => area,
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
                if *icon_only {
                    show_title_beside(button, &text);
                } else {
                    button.setTitle(&text);
                }
            }
            WidgetKind::NavItem(item) => item.set_title(text),
            WidgetKind::Checkbox { checkbox, .. } => checkbox.setTitle(&NSString::from_str(text)),
            WidgetKind::Switch(row) => row.set_title(text),
            WidgetKind::Picker { popup, .. } => {
                let index = (0..popup.numberOfItems())
                    .find(|&index| popup.itemTitleAtIndex(index).to_string() == text)
                    .unwrap_or(NO_ITEM);
                popup.selectItemAtIndex(index);
            }
            // Re-setting an unchanged value would move the cursor while the user types.
            WidgetKind::TextField { field, .. } => {
                if field.stringValue().to_string() != text {
                    field.setStringValue(&NSString::from_str(text));
                }
            }
            WidgetKind::TextArea { text_view, .. } => {
                if text_view.string().to_string() != text {
                    text_view.setString(&NSString::from_str(text));
                }
            }
            WidgetKind::Progress(_)
            | WidgetKind::Image { .. }
            | WidgetKind::Slider { .. }
            | WidgetKind::Divider(_)
            | WidgetKind::ContextMenu(_)
            | WidgetKind::DropTarget(_)
            | WidgetKind::Surface(_)
            | WidgetKind::Scroll { .. } => {}
        }
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        match &self.0 {
            WidgetKind::Checkbox { checkbox, .. } => checkbox.setState(control_state(checked)),
            WidgetKind::Button { button, target, .. } => {
                target.set_selected(checked);
                button.setState(control_state(checked));
            }
            WidgetKind::Switch(row) => row.set_on(checked),
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
        match &self.0 {
            WidgetKind::Progress(bar) => bar.setDoubleValue(to_double(value)),
            WidgetKind::Slider { slider, target } => {
                target.shown(value);
                slider.setDoubleValue(to_double(value));
            }
            _ => {}
        }
    }

    /// Replaces a picker's items, keeping the one it shows if it is still there. Items
    /// are added through the menu, which keeps any with the same title.
    pub(crate) fn set_options(&self, options: &[String]) {
        let WidgetKind::Picker { popup, .. } = &self.0 else {
            return;
        };
        let shown = popup.titleOfSelectedItem();
        let menu = popup.menu().expect("a pop-up button has a menu");
        menu.removeAllItems();
        for option in options {
            // SAFETY: the item has no action of its own; picking it sends the button's.
            unsafe {
                menu.addItemWithTitle_action_keyEquivalent(
                    &NSString::from_str(option),
                    None,
                    ns_string!(""),
                );
            }
        }
        self.set_text(&shown.map(|title| title.to_string()).unwrap_or_default());
    }

    pub(crate) fn measure(&self) -> Size {
        match &self.0 {
            WidgetKind::TextField { .. } => {
                let fitting = to_size(self.view().fittingSize());
                Size::new(fitting.width.max(MIN_TEXT_FIELD_WIDTH), fitting.height)
            }
            WidgetKind::TextArea {
                text_view, lines, ..
            } => text_area_size(text_view, *lines),
            WidgetKind::NavItem(item) => item.measure(),
            WidgetKind::Switch(row) => row.measure(),
            WidgetKind::Progress(_) | WidgetKind::Slider { .. } => {
                let fitting = to_size(self.view().fittingSize());
                Size::new(fitting.width.max(MIN_TRACK_WIDTH), fitting.height)
            }
            WidgetKind::Image { view, size } => {
                let natural = view
                    .image()
                    .map_or(Size::ZERO, |image| to_size(image.size()));
                size.frame(natural)
            }
            WidgetKind::Divider(_) => DIVIDER_SIZE,
            _ => to_size(self.view().fittingSize()),
        }
    }

    pub(crate) fn set_frame(&self, frame: Frame) {
        match &self.0 {
            WidgetKind::NavItem(item) => item.set_frame(frame),
            WidgetKind::Switch(row) => row.set_frame(frame),
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
            WidgetKind::TextArea {
                scroll,
                text_view,
                delegate,
                ..
            } => {
                release_after_event(scroll);
                release_after_event(text_view);
                release_after_event(delegate);
            }
            WidgetKind::Checkbox { checkbox, target } => {
                release_after_event(checkbox);
                release_after_event(target);
            }
            WidgetKind::Switch(row) => row.remove(),
            WidgetKind::Slider { slider, target } => {
                release_after_event(slider);
                release_after_event(target);
            }
            WidgetKind::Picker { popup, target } => {
                release_after_event(popup);
                release_after_event(target);
            }
            WidgetKind::Progress(bar) => {
                // SAFETY: `stopAnimation:` takes any sender, and nil is allowed.
                unsafe { bar.stopAnimation(None) };
                release_after_event(bar);
            }
            WidgetKind::Image { view, .. } => release_after_event(view),
            WidgetKind::Divider(divider) => release_after_event(divider),
            WidgetKind::ContextMenu(area) => release_after_event(area),
            WidgetKind::DropTarget(area) => release_after_event(area),
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

/// A text area's natural size: the text field's least width, and `lines` lines of its font
/// inside its inset and border.
fn text_area_size(text_view: &NSTextView, lines: u32) -> Size {
    let font = system_font(TextStyle::Body);
    // SAFETY: a text view made by `scrollableTextView` lays out with a layout manager.
    let line_height = unsafe { text_view.layoutManager() }
        .expect("a scrollable text view has a layout manager")
        .defaultLineHeightForFont(&font);
    let content = NSSize::new(
        f64::from(MIN_TEXT_FIELD_WIDTH),
        f64::from(lines) * line_height + 2.0 * TEXT_AREA_INSET.height,
    );
    let mtm = MainThreadMarker::from(text_view);
    // SAFETY: no scroller classes are passed, so only the border adds to the size.
    let frame = unsafe {
        NSScrollView::frameSizeForContentSize_horizontalScrollerClass_verticalScrollerClass_borderType_controlSize_scrollerStyle(
            content,
            None,
            None,
            NSBorderType::BezelBorder,
            NSControlSize::Regular,
            NSScroller::preferredScrollerStyle(mtm),
            mtm,
        )
    };
    to_size(frame)
}

/// Gives an icon-only button its title as its tooltip and what screen readers say. The
/// button itself keeps no title: setting one on an image-only button once it is shown
/// makes `AppKit` draw it over the image.
fn show_title_beside(button: &NSButton, title: &NSString) {
    button.setTitle(ns_string!(""));
    button.setToolTip(Some(title));
    button.setAccessibilityLabel(Some(title));
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
