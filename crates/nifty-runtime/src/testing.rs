//! An in-memory backend for tests: widgets are records, and tests play the user's part.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::backend::{
    Backend, ClickHandler, ResizeHandler, TextHandler, ToggleHandler, WakeHandler, Waker,
};
use crate::geometry::{Frame, Size};
use crate::style::{ButtonStyle, TextStyle};

/// Text is measured as this many points per byte, one line high.
const CHAR_WIDTH: f32 = 10.0;
const LINE_HEIGHT: f32 = 20.0;
const TEXT_FIELD_SIZE: Size = Size::new(100.0, LINE_HEIGHT);
const CHECKBOX_BOX_WIDTH: f32 = 20.0;
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FakeKind {
    Label,
    Button,
    TextField,
    Checkbox,
    Divider,
}

enum Handler {
    None,
    Click(Rc<dyn Fn()>),
    Text(Rc<dyn Fn(&str)>),
    Toggle(Rc<dyn Fn(bool)>),
}

struct FakeWidget {
    kind: FakeKind,
    text: String,
    checked: bool,
    style: Option<String>,
    frame: Option<Frame>,
    removed: bool,
    handler: Handler,
}

struct FakeWindow {
    title: String,
    size: Size,
    shown: bool,
    closed: bool,
    on_resize: Rc<dyn Fn(Size)>,
}

/// A wake handler and whether its waker has asked for it since it last ran.
struct FakeWake {
    pending: Arc<AtomicBool>,
    on_wake: WakeHandler,
}

#[derive(Default)]
struct FakeState {
    widgets: RefCell<Vec<FakeWidget>>,
    windows: RefCell<Vec<FakeWindow>>,
    wakes: RefCell<Vec<Rc<FakeWake>>>,
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
    fn push(&self, kind: FakeKind, text: &str, style: Option<String>, handler: Handler) -> usize {
        let mut widgets = self.0.widgets.borrow_mut();
        widgets.push(FakeWidget {
            kind,
            text: text.to_owned(),
            checked: false,
            style,
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

    #[must_use]
    pub fn is_checked(&self, widget: usize) -> bool {
        self.with(widget, |w| w.checked)
    }

    /// The style a widget was created with, as its `Debug` name.
    #[must_use]
    pub fn style(&self, widget: usize) -> Option<String> {
        self.with(widget, |w| w.style.clone())
    }

    #[must_use]
    pub fn frame(&self, widget: usize) -> Option<Frame> {
        self.with(widget, |w| w.frame)
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
    pub fn is_closed(&self, window: usize) -> bool {
        self.0.windows.borrow()[window].closed
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

    /// Live widgets of `kind` in reading order: top to bottom, then left to right.
    #[must_use]
    pub fn find(&self, kind: FakeKind) -> Vec<usize> {
        let widgets = self.0.widgets.borrow();
        let mut found: Vec<(usize, Frame)> = widgets
            .iter()
            .enumerate()
            .filter(|(_, w)| w.kind == kind && !w.removed)
            .map(|(index, w)| (index, w.frame.unwrap_or_default()))
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
    /// When the widget is not a button.
    pub fn click(&self, widget: usize) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Click(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a button"),
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

    /// # Panics
    /// When the widget is not a checkbox.
    pub fn toggle(&self, widget: usize) {
        let handler = self.with(widget, |w| match &w.handler {
            Handler::Toggle(handler) => Rc::clone(handler),
            _ => panic!("widget {widget} is not a checkbox"),
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

    fn set_content_size(&self, window: &usize, size: Size) {
        self.resize(*window, size);
    }

    fn close_window(&self, window: usize) {
        self.0.windows.borrow_mut()[window].closed = true;
    }

    fn create_label(&self, _window: &usize, text: &str, style: TextStyle) -> usize {
        self.push(
            FakeKind::Label,
            text,
            Some(format!("{style:?}")),
            Handler::None,
        )
    }

    fn create_button(
        &self,
        _window: &usize,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> usize {
        self.push(
            FakeKind::Button,
            title,
            Some(format!("{style:?}")),
            Handler::Click(Rc::from(on_click)),
        )
    }

    fn create_text_field(
        &self,
        _window: &usize,
        text: &str,
        _placeholder: &str,
        on_change: TextHandler,
    ) -> usize {
        self.push(
            FakeKind::TextField,
            text,
            None,
            Handler::Text(Rc::from(on_change)),
        )
    }

    fn create_checkbox(
        &self,
        _window: &usize,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> usize {
        let widget = self.push(
            FakeKind::Checkbox,
            title,
            None,
            Handler::Toggle(Rc::from(on_toggle)),
        );
        self.0.widgets.borrow_mut()[widget].checked = checked;
        widget
    }

    fn create_divider(&self, _window: &usize) -> usize {
        self.push(FakeKind::Divider, "", None, Handler::None)
    }

    fn set_text(&self, widget: &usize, text: &str) {
        text.clone_into(&mut self.0.widgets.borrow_mut()[*widget].text);
    }

    fn set_checked(&self, widget: &usize, checked: bool) {
        self.0.widgets.borrow_mut()[*widget].checked = checked;
    }

    fn measure(&self, widget: &usize) -> Size {
        self.with(*widget, |w| match w.kind {
            FakeKind::Label | FakeKind::Button => Size::new(text_width(&w.text), LINE_HEIGHT),
            FakeKind::TextField => TEXT_FIELD_SIZE,
            FakeKind::Checkbox => Size::new(CHECKBOX_BOX_WIDTH + text_width(&w.text), LINE_HEIGHT),
            FakeKind::Divider => DIVIDER_SIZE,
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

    fn run(&self) {}
}
