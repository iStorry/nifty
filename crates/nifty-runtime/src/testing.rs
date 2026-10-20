//! An in-memory backend for tests: widgets are records, and tests play the user's part.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::backend::{
    Backend, ClickHandler, NumberHandler, Parent, ResizeHandler, TextHandler, ToggleHandler,
    WakeHandler, Waker,
};
use crate::dialog::{DialogHandler, FileDialog, Picked};
use crate::geometry::{Frame, Point, Size};
use crate::style::{Backdrop, ButtonStyle, Icon, LabelStyle, Surface, ToggleStyle};

/// Text is measured as this many points per byte, one line high.
const CHAR_WIDTH: f32 = 10.0;
const LINE_HEIGHT: f32 = 20.0;
const TEXT_FIELD_SIZE: Size = Size::new(100.0, LINE_HEIGHT);
const CHECKBOX_BOX_WIDTH: f32 = 20.0;
const SWITCH_WIDTH: f32 = 40.0;
const SLIDER_SIZE: Size = Size::new(100.0, LINE_HEIGHT);
/// A picker's arrow, after its widest option.
const PICKER_ARROW_WIDTH: f32 = 20.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);
const PROGRESS_SIZE: Size = Size::new(100.0, 4.0);
/// An icon is drawn one line high and as wide.
const ICON_WIDTH: f32 = LINE_HEIGHT;
/// Scroll bars take this much of a scroll area's width.
pub const SCROLL_BAR_WIDTH: f32 = 10.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeKind {
    Label,
    Button,
    TextField,
    Checkbox,
    Switch,
    Slider,
    Picker,
    NavItem,
    Progress,
    Divider,
    Surface,
    Scroll,
}

enum Handler {
    None,
    Click(Rc<dyn Fn()>),
    Text(Rc<dyn Fn(&str)>),
    Toggle(Rc<dyn Fn(bool)>),
    Number(Rc<dyn Fn(i64)>),
}

struct FakeWidget {
    kind: FakeKind,
    text: String,
    checked: bool,
    style: Option<String>,
    icon: Icon,
    /// A button that shows only its icon, measured without its title.
    icon_only: bool,
    /// Whether a button made with a selection shows as selected.
    selected: Option<bool>,
    badge: String,
    placeholder: String,
    /// A progress bar's percentage, or `None` while it is busy without one; a slider's value.
    value: Option<i64>,
    /// A slider's lowest and highest values.
    range: Option<(i64, i64)>,
    /// A picker's options.
    options: Vec<String>,
    /// The scroll area whose content it is in, or `None` straight in its window.
    host: Option<usize>,
    /// A scroll area's content size.
    content: Option<Size>,
    frame: Option<Frame>,
    removed: bool,
    handler: Handler,
}

struct FakeWindow {
    title: String,
    size: Size,
    shown: bool,
    closed: bool,
    title_bar_inset: f32,
    backdrop: Backdrop,
    /// Every minimum width the window was given, in order.
    min_widths: Vec<f32>,
    on_resize: Rc<dyn Fn(Size)>,
}

/// A wake handler and whether its waker has asked for it since it last ran.
struct FakeWake {
    pending: Arc<AtomicBool>,
    on_wake: WakeHandler,
}

/// A dialog shown so far; `on_close` is taken when the test closes it.
struct FakeDialog {
    dialog: FileDialog,
    on_close: Option<DialogHandler>,
}

#[derive(Default)]
struct FakeState {
    /// What scroll bars take beside a scroll area's content, once a test changes it from
    /// [`SCROLL_BAR_WIDTH`].
    scroll_bar_width: Cell<Option<f32>>,
    widgets: RefCell<Vec<FakeWidget>>,
    windows: RefCell<Vec<FakeWindow>>,
    wakes: RefCell<Vec<Rc<FakeWake>>>,
    dialogs: RefCell<Vec<FakeDialog>>,
}

/// Marks its handler pending; [`FakeBackend::run_pending_wakes`] plays the UI thread.
#[derive(Debug)]
pub struct FakeWaker(Arc<AtomicBool>);

impl Waker for FakeWaker {
    fn wake(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Widgets are numbered in creation order. Clones share state, so a test keeps one clone
/// while the `Ui` owns another.
#[derive(Clone, Default)]
pub struct FakeBackend(Rc<FakeState>);

impl std::fmt::Debug for FakeBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FakeBackend").finish_non_exhaustive()
    }
}

impl FakeBackend {
    fn push(
        &self,
        parent: Parent<'_, Self>,
        kind: FakeKind,
        text: &str,
        style: Option<String>,
        handler: Handler,
    ) -> usize {
        let mut widgets = self.0.widgets.borrow_mut();
        widgets.push(FakeWidget {
            kind,
            text: text.to_owned(),
            checked: false,
            style,
            icon: Icon::None,
            icon_only: false,
            selected: None,
            badge: String::new(),
            placeholder: String::new(),
            value: None,
            range: None,
            options: Vec::new(),
            host: match parent {
                Parent::Window(_) => None,
                Parent::Scroll(scroll) => Some(*scroll),
            },
            content: None,
            frame: None,
            removed: false,
            handler,
        });
        widgets.len() - 1
    }

    fn with<R>(&self, widget: usize, read: impl FnOnce(&FakeWidget) -> R) -> R {
        read(&self.0.widgets.borrow()[widget])
    }

    #[must_use]
    pub fn text(&self, widget: usize) -> String {
        self.with(widget, |w| w.text.clone())
    }

    /// Whether a checkbox or switch is on, or a nav item selected.
    #[must_use]
    pub fn is_checked(&self, widget: usize) -> bool {
        self.with(widget, |w| w.checked)
    }

    /// A button's selection, or `None` for one that shows none.
    #[must_use]
    pub fn selection(&self, widget: usize) -> Option<bool> {
        self.with(widget, |w| w.selected)
    }

    /// The style a widget was created with, as its `Debug` name.
    #[must_use]
    pub fn style(&self, widget: usize) -> Option<String> {
        self.with(widget, |w| w.style.clone())
    }

    #[must_use]
    pub fn icon(&self, widget: usize) -> Icon {
        self.with(widget, |w| w.icon)
    }

    /// A nav item's badge, or an empty string for other widgets.
    #[must_use]
    pub fn badge(&self, widget: usize) -> String {
        self.with(widget, |w| w.badge.clone())
    }

    /// A text field's placeholder, or an empty string for other widgets.
    #[must_use]
    pub fn placeholder(&self, widget: usize) -> String {
        self.with(widget, |w| w.placeholder.clone())
    }

    /// A progress bar's percentage, or `None` for a busy bar and other widgets.
    #[must_use]
    pub fn value(&self, widget: usize) -> Option<i64> {
        self.with(widget, |w| w.value)
    }

    /// A slider's lowest and highest values.
    #[must_use]
    pub fn range(&self, widget: usize) -> Option<(i64, i64)> {
        self.with(widget, |w| w.range)
    }

    /// A picker's options, or none for other widgets.
    #[must_use]
    pub fn options(&self, widget: usize) -> Vec<String> {
        self.with(widget, |w| w.options.clone())
    }

    /// The frame the widget was given, relative to its parent's content.
    #[must_use]
    pub fn frame(&self, widget: usize) -> Option<Frame> {
        self.with(widget, |w| w.frame)
    }

    /// The scroll area the widget is in, or `None` straight in its window.
    #[must_use]
    pub fn host(&self, widget: usize) -> Option<usize> {
        self.with(widget, |w| w.host)
    }

    /// A scroll area's content size, once laid out.
    #[must_use]
    pub fn scroll_content(&self, scroll: usize) -> Option<Size> {
        self.with(scroll, |w| w.content)
    }

    /// Where the widget is in its window, with every scroll area scrolled to the top.
    #[must_use]
    pub fn window_frame(&self, widget: usize) -> Frame {
        let (frame, host) = self.with(widget, |w| (w.frame.unwrap_or_default(), w.host));
        let Some(host) = host else {
            return frame;
        };
        let content = self.window_frame(host).origin;
        Frame {
            origin: Point {
                x: content.x + frame.origin.x,
                y: content.y + frame.origin.y,
            },
            size: frame.size,
        }
    }

    #[must_use]
    pub fn is_removed(&self, widget: usize) -> bool {
        self.with(widget, |w| w.removed)
    }

    #[must_use]
    pub fn window_title(&self, window: usize) -> String {
        self.0.windows.borrow()[window].title.clone()
    }

    #[must_use]
    pub fn is_shown(&self, window: usize) -> bool {
        self.0.windows.borrow()[window].shown
    }

    #[must_use]
    pub fn window_size(&self, window: usize) -> Size {
        self.0.windows.borrow()[window].size
    }

    #[must_use]
    pub fn backdrop(&self, window: usize) -> Backdrop {
        self.0.windows.borrow()[window].backdrop
    }

    /// Every minimum content width the window was given, in order.
    #[must_use]
    pub fn min_widths(&self, window: usize) -> Vec<f32> {
        self.0.windows.borrow()[window].min_widths.clone()
    }

    /// Plays the user changing how wide scroll bars are.
    pub fn set_scroll_bar_width(&self, width: f32) {
        self.0.scroll_bar_width.set(Some(width));
    }

    /// Plays a platform whose title bar covers the top `inset` points of the content area.
    pub fn set_title_bar_inset(&self, window: usize, inset: f32) {
        self.0.windows.borrow_mut()[window].title_bar_inset = inset;
    }

    #[must_use]
    pub fn is_closed(&self, window: usize) -> bool {
        self.0.windows.borrow()[window].closed
    }

    /// Widgets created so far, removed ones included.
    #[must_use]
    pub fn widget_count(&self) -> usize {
        self.0.widgets.borrow().len()
    }

    /// Windows opened so far, closed ones included.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.0.windows.borrow().len()
    }

    /// Runs the handler of every waker woken since its handler last ran.
    pub fn run_pending_wakes(&self) {
        let wakes = self.0.wakes.borrow().clone();
        for wake in wakes {
            if wake.pending.swap(false, Ordering::SeqCst) {
                (wake.on_wake)();
            }
        }
    }

    /// Every file dialog shown so far, closed ones included, in the order shown.
    #[must_use]
    pub fn dialogs(&self) -> Vec<FileDialog> {
        self.0
            .dialogs
            .borrow()
            .iter()
            .map(|shown| shown.dialog.clone())
            .collect()
    }

    /// Closes the `index`th dialog shown as the user would, answering `picked`.
    ///
    /// # Panics
    /// When that dialog was never shown or is already closed.
    pub fn close_dialog(&self, index: usize, picked: Picked) {
        let on_close = self.0.dialogs.borrow_mut()[index]
            .on_close
            .take()
            .unwrap_or_else(|| panic!("dialog {index} is already closed"));
        on_close(picked);
    }

    /// Live widgets of `kind` in reading order: top to bottom, then left to right.
    #[must_use]
    pub fn find(&self, kind: FakeKind) -> Vec<usize> {
        let live: Vec<usize> = self
            .0
            .widgets
            .borrow()
            .iter()
            .enumerate()
            .filter(|(_, w)| w.kind == kind && !w.removed)
            .map(|(index, _)| index)
            .collect();
        let mut found: Vec<(usize, Frame)> = live
            .into_iter()
            .map(|index| (index, self.window_frame(index)))
            .collect();
        found.sort_by(|(_, a), (_, b)| {
            a.origin
                .y
                .total_cmp(&b.origin.y)
                .then(a.origin.x.total_cmp(&b.origin.x))
        });
        found.into_iter().map(|(index, _)| index).collect()
    }

    /// The text of live widgets of `kind`, in reading order.
    #[must_use]
    pub fn visible_texts(&self, kind: FakeKind) -> Vec<String> {
        self.find(kind)
            .into_iter()
            .map(|widget| self.text(widget))
            .collect()
    }

    /// # Panics
    /// When the widget is not a button or nav item.
    pub fn click(&self, widget: usize) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Click(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a button or nav item"),
        });
        handler();
    }

    /// Replaces the field's text as if the user typed it.
    ///
    /// # Panics
    /// When the widget is not a text field.
    pub fn type_text(&self, widget: usize, text: &str) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Text(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a text field"),
        });
        text.clone_into(&mut self.0.widgets.borrow_mut()[widget].text);
        handler(text);
    }

    /// Drags a slider to `value`, as the user would.
    ///
    /// # Panics
    /// When the widget is not a slider.
    pub fn slide(&self, widget: usize, value: i64) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Number(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a slider"),
        });
        self.0.widgets.borrow_mut()[widget].value = Some(value);
        handler(value);
    }

    /// Picks `option` from a picker's menu, as the user would.
    ///
    /// # Panics
    /// When the widget is not a picker, or doesn't offer `option`.
    pub fn pick(&self, widget: usize, option: &str) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Text(handler) if w.kind == FakeKind::Picker => {
                assert!(
                    w.options.iter().any(|offered| offered == option),
                    "picker {widget} doesn't offer {option:?}"
                );
                Rc::clone(handler)
            }
            _ => panic!("widget {widget} is not a picker"),
        });
        option.clone_into(&mut self.0.widgets.borrow_mut()[widget].text);
        handler(option);
    }

    /// # Panics
    /// When the widget is not a checkbox or switch.
    pub fn toggle(&self, widget: usize) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Toggle(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a checkbox or switch"),
        });
        let checked = {
            let mut widgets = self.0.widgets.borrow_mut();
            widgets[widget].checked = !widgets[widget].checked;
            widgets[widget].checked
        };
        handler(checked);
    }

    pub fn resize(&self, window: usize, size: Size) {
        let on_resize = {
            let mut windows = self.0.windows.borrow_mut();
            windows[window].size = size;
            Rc::clone(&windows[window].on_resize)
        };
        on_resize(size);
    }
}

fn icon_width(icon: Icon) -> f32 {
    if icon == Icon::None { 0.0 } else { ICON_WIDTH }
}

fn text_width(text: &str) -> f32 {
    let bytes = u16::try_from(text.len()).expect("test strings are short");
    f32::from(bytes) * CHAR_WIDTH
}

impl Backend for FakeBackend {
    type Window = usize;
    type Widget = usize;
    type Waker = FakeWaker;

    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> usize {
        let mut windows = self.0.windows.borrow_mut();
        windows.push(FakeWindow {
            title: title.to_owned(),
            size,
            shown: false,
            closed: false,
            title_bar_inset: 0.0,
            backdrop: Backdrop::default(),
            min_widths: Vec::new(),
            on_resize: Rc::from(on_resize),
        });
        windows.len() - 1
    }

    fn content_size(&self, window: &usize) -> Size {
        self.0.windows.borrow()[*window].size
    }

    fn show_window(&self, window: &usize) {
        self.0.windows.borrow_mut()[*window].shown = true;
    }

    fn set_title(&self, window: &usize, title: &str) {
        title.clone_into(&mut self.0.windows.borrow_mut()[*window].title);
    }

    fn set_backdrop(&self, window: &usize, backdrop: Backdrop) {
        self.0.windows.borrow_mut()[*window].backdrop = backdrop;
    }

    fn set_content_size(&self, window: &usize, size: Size) {
        self.resize(*window, size);
    }

    fn close_window(&self, window: usize) {
        self.0.windows.borrow_mut()[window].closed = true;
    }

    fn title_bar_inset(&self, window: &usize) -> f32 {
        self.0.windows.borrow()[*window].title_bar_inset
    }

    fn set_min_content_width(&self, window: &usize, width: f32) {
        self.0.windows.borrow_mut()[*window].min_widths.push(width);
    }

    fn create_surface(&self, parent: Parent<'_, Self>, surface: Surface) -> usize {
        self.push(
            parent,
            FakeKind::Surface,
            "",
            Some(format!("{surface:?}")),
            Handler::None,
        )
    }

    fn create_scroll(&self, parent: Parent<'_, Self>) -> usize {
        self.push(parent, FakeKind::Scroll, "", None, Handler::None)
    }

    fn scroll_bar_width(&self, _window: &usize) -> f32 {
        self.0.scroll_bar_width.get().unwrap_or(SCROLL_BAR_WIDTH)
    }

    fn set_scroll_content(&self, scroll: &usize, size: Size) {
        self.0.widgets.borrow_mut()[*scroll].content = Some(size);
    }

    fn create_label(&self, parent: Parent<'_, Self>, text: &str, style: LabelStyle) -> usize {
        self.push(
            parent,
            FakeKind::Label,
            text,
            Some(format!("{style:?}")),
            Handler::None,
        )
    }

    fn create_button(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        style: ButtonStyle,
        icon: Icon,
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> usize {
        let button = self.push(
            parent,
            FakeKind::Button,
            title,
            Some(format!("{style:?}")),
            Handler::Click(Rc::from(on_click)),
        );
        let mut widgets = self.0.widgets.borrow_mut();
        widgets[button].icon = icon;
        widgets[button].icon_only = style == ButtonStyle::Icon;
        widgets[button].selected = selected;
        button
    }

    fn create_nav_item(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> usize {
        let item = self.push(
            parent,
            FakeKind::NavItem,
            title,
            None,
            Handler::Click(Rc::from(on_click)),
        );
        let mut widgets = self.0.widgets.borrow_mut();
        widgets[item].icon = icon;
        badge.clone_into(&mut widgets[item].badge);
        widgets[item].checked = selected;
        item
    }

    fn create_text_field(
        &self,
        parent: Parent<'_, Self>,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> usize {
        let field = self.push(
            parent,
            FakeKind::TextField,
            text,
            None,
            Handler::Text(Rc::from(on_change)),
        );
        placeholder.clone_into(&mut self.0.widgets.borrow_mut()[field].placeholder);
        field
    }

    fn create_checkbox(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        checked: bool,
        style: ToggleStyle,
        on_toggle: ToggleHandler,
    ) -> usize {
        let kind = match style {
            ToggleStyle::Checkbox => FakeKind::Checkbox,
            ToggleStyle::Switch => FakeKind::Switch,
        };
        let widget = self.push(
            parent,
            kind,
            title,
            None,
            Handler::Toggle(Rc::from(on_toggle)),
        );
        self.0.widgets.borrow_mut()[widget].checked = checked;
        widget
    }

    fn create_slider(
        &self,
        parent: Parent<'_, Self>,
        value: i64,
        min: i64,
        max: i64,
        on_change: NumberHandler,
    ) -> usize {
        let slider = self.push(
            parent,
            FakeKind::Slider,
            "",
            None,
            Handler::Number(Rc::from(on_change)),
        );
        let mut widgets = self.0.widgets.borrow_mut();
        widgets[slider].value = Some(value);
        widgets[slider].range = Some((min, max));
        slider
    }

    fn create_picker(
        &self,
        parent: Parent<'_, Self>,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> usize {
        let picker = self.push(
            parent,
            FakeKind::Picker,
            selected,
            None,
            Handler::Text(Rc::from(on_pick)),
        );
        options.clone_into(&mut self.0.widgets.borrow_mut()[picker].options);
        picker
    }

    fn create_divider(&self, parent: Parent<'_, Self>) -> usize {
        self.push(parent, FakeKind::Divider, "", None, Handler::None)
    }

    fn create_progress(&self, parent: Parent<'_, Self>, percent: Option<i64>) -> usize {
        let bar = self.push(parent, FakeKind::Progress, "", None, Handler::None);
        self.0.widgets.borrow_mut()[bar].value = percent;
        bar
    }

    fn set_text(&self, widget: &usize, text: &str) {
        text.clone_into(&mut self.0.widgets.borrow_mut()[*widget].text);
    }

    fn set_checked(&self, widget: &usize, checked: bool) {
        let mut widgets = self.0.widgets.borrow_mut();
        let widget = &mut widgets[*widget];
        match &mut widget.selected {
            Some(selected) => *selected = checked,
            None => widget.checked = checked,
        }
    }

    fn set_badge(&self, widget: &usize, badge: &str) {
        badge.clone_into(&mut self.0.widgets.borrow_mut()[*widget].badge);
    }

    fn set_value(&self, widget: &usize, value: i64) {
        self.0.widgets.borrow_mut()[*widget].value = Some(value);
    }

    fn set_options(&self, widget: &usize, options: &[String]) {
        options.clone_into(&mut self.0.widgets.borrow_mut()[*widget].options);
    }

    fn measure(&self, widget: &usize) -> Size {
        self.with(*widget, |w| match w.kind {
            FakeKind::Label => Size::new(text_width(&w.text), LINE_HEIGHT),
            FakeKind::Button if w.icon_only => Size::new(ICON_WIDTH, LINE_HEIGHT),
            FakeKind::Button => Size::new(icon_width(w.icon) + text_width(&w.text), LINE_HEIGHT),
            FakeKind::NavItem => Size::new(
                icon_width(w.icon) + text_width(&w.text) + text_width(&w.badge),
                LINE_HEIGHT,
            ),
            FakeKind::TextField => TEXT_FIELD_SIZE,
            FakeKind::Checkbox => Size::new(CHECKBOX_BOX_WIDTH + text_width(&w.text), LINE_HEIGHT),
            FakeKind::Switch => Size::new(text_width(&w.text) + SWITCH_WIDTH, LINE_HEIGHT),
            FakeKind::Slider => SLIDER_SIZE,
            FakeKind::Picker => Size::new(
                w.options
                    .iter()
                    .map(|option| text_width(option))
                    .fold(0.0, f32::max)
                    + PICKER_ARROW_WIDTH,
                LINE_HEIGHT,
            ),
            FakeKind::Progress => PROGRESS_SIZE,
            FakeKind::Divider => DIVIDER_SIZE,
            FakeKind::Surface | FakeKind::Scroll => Size::ZERO,
        })
    }

    fn set_frame(&self, widget: &usize, frame: Frame) {
        self.0.widgets.borrow_mut()[*widget].frame = Some(frame);
    }

    fn remove_widget(&self, widget: usize) {
        self.0.widgets.borrow_mut()[widget].removed = true;
    }

    fn waker(&self, on_wake: WakeHandler) -> FakeWaker {
        let pending = Arc::new(AtomicBool::new(false));
        self.0.wakes.borrow_mut().push(Rc::new(FakeWake {
            pending: Arc::clone(&pending),
            on_wake,
        }));
        FakeWaker(pending)
    }

    fn show_file_dialog(&self, dialog: &FileDialog, on_close: DialogHandler) {
        self.0.dialogs.borrow_mut().push(FakeDialog {
            dialog: dialog.clone(),
            on_close: Some(on_close),
        });
    }

    fn run(&self) {}
}
