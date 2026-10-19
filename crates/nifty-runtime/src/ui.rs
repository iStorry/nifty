//! The retained UI that apps drive: create native widgets, arrange them, update them.
//! Changes are laid out together when [`Ui::flush`] runs, once per event.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::backend::{Backend, ClickHandler, TextHandler, ToggleHandler};
use crate::geometry::Size;
use crate::layout::Layout;
use crate::style::{ButtonStyle, StackStyle, TextStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

/// A widget, stack or spacer inside one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    window: WindowId,
    id: taffy::NodeId,
}

impl Node {
    #[must_use]
    pub fn window(self) -> WindowId {
        self.window
    }
}

/// A shared handle to the app's windows; clones refer to the same UI.
pub struct Ui<B: Backend> {
    state: Rc<RefCell<UiState<B>>>,
}

struct UiState<B: Backend> {
    backend: Rc<B>,
    layout: Layout<B::Widget>,
    windows: Vec<WindowState<B>>,
}

struct WindowState<B: Backend> {
    native: Rc<B::Window>,
    root: taffy::NodeId,
    /// `None` until the window is shown and reports its size.
    viewport: Option<Size>,
    needs_layout: bool,
}

impl<B: Backend> Ui<B> {
    pub fn new(backend: B) -> Self {
        Self {
            state: Rc::new(RefCell::new(UiState {
                backend: Rc::new(backend),
                layout: Layout::new(),
                windows: Vec::new(),
            })),
        }
    }

    /// Opens a hidden window; fill it with [`Ui::set_content`], then [`Ui::show`] it.
    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn create_window(&self, title: &str, size: Size) -> WindowId {
        let weak_state = Rc::downgrade(&self.state);
        let mut state = self.state.borrow_mut();
        let window = WindowId(state.windows.len());
        let on_resize = Box::new(move |size| {
            if let Some(state) = weak_state.upgrade() {
                state.borrow_mut().resize(window, size);
            }
        });
        let native = state.backend.open_window(title, size, on_resize);
        let root = state.layout.add_root();
        state.windows.push(WindowState {
            native: Rc::new(native),
            root,
            viewport: None,
            needs_layout: true,
        });
        window
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn stack(&self, window: WindowId, style: &StackStyle) -> Node {
        let id = self.state.borrow_mut().layout.add_stack(style);
        Node { window, id }
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn spacer(&self, window: WindowId) -> Node {
        let id = self.state.borrow_mut().layout.add_spacer();
        Node { window, id }
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn label(&self, window: WindowId, text: &str, style: TextStyle) -> Node {
        self.add_widget(window, Layout::add_widget, |backend, native| {
            backend.create_label(native, text, style)
        })
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn button(
        &self,
        window: WindowId,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> Node {
        self.add_widget(window, Layout::add_widget, |backend, native| {
            backend.create_button(native, title, style, on_click)
        })
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn text_field(
        &self,
        window: WindowId,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> Node {
        self.add_widget(window, Layout::add_widget, |backend, native| {
            backend.create_text_field(native, text, placeholder, on_change)
        })
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn checkbox(
        &self,
        window: WindowId,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> Node {
        self.add_widget(window, Layout::add_widget, |backend, native| {
            backend.create_checkbox(native, title, checked, on_toggle)
        })
    }

    #[must_use = "a new node must be placed with `insert` or `set_content`"]
    pub fn divider(&self, window: WindowId) -> Node {
        self.add_widget(window, Layout::add_divider, |backend, native| {
            backend.create_divider(native)
        })
    }

    fn add_widget(
        &self,
        window: WindowId,
        place: fn(&mut Layout<B::Widget>, B::Widget, Size) -> taffy::NodeId,
        create: impl FnOnce(&B, &B::Window) -> B::Widget,
    ) -> Node {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let widget = create(&state.backend, &state.windows[window.0].native);
        let natural = state.backend.measure(&widget);
        let id = place(&mut state.layout, widget, natural);
        Node { window, id }
    }

    /// Places `child` at `index` among `parent`'s children.
    ///
    /// # Panics
    /// When the two nodes belong to different windows.
    pub fn insert(&self, parent: Node, index: usize, child: Node) {
        assert_eq!(
            parent.window, child.window,
            "a node can only be placed in the window it was created for"
        );
        let mut state = self.state.borrow_mut();
        state.layout.insert(parent.id, index, child.id);
        state.invalidate(parent.window);
    }

    pub fn set_content(&self, window: WindowId, content: Node) {
        let mut state = self.state.borrow_mut();
        let root = state.windows[window.0].root;
        state.layout.set_content(root, content.id);
        state.invalidate(window);
    }

    /// Removes `node` and everything inside it from the window.
    pub fn remove(&self, node: Node) {
        let mut state = self.state.borrow_mut();
        let widgets = state.layout.remove(node.id);
        state.invalidate(node.window);
        for widget in widgets {
            state.backend.remove_widget(widget);
        }
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_text(&self, node: Node, text: &str) {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let widget = state
            .layout
            .widget(node.id)
            .expect("only widgets have text");
        state.backend.set_text(widget, text);
        let natural = state.backend.measure(widget);
        if state.layout.set_natural(node.id, natural) {
            state.invalidate(node.window);
        }
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_checked(&self, node: Node, checked: bool) {
        let state = self.state.borrow();
        let widget = state
            .layout
            .widget(node.id)
            .expect("only widgets can be checked");
        state.backend.set_checked(widget, checked);
    }

    /// Lays out every window changed since the last flush.
    pub fn flush(&self) {
        let mut state = self.state.borrow_mut();
        for index in 0..state.windows.len() {
            if state.windows[index].needs_layout {
                state.layout_window(WindowId(index));
            }
        }
    }

    pub fn show(&self, window: WindowId) {
        let (backend, native) = {
            let mut state = self.state.borrow_mut();
            let native = Rc::clone(&state.windows[window.0].native);
            let size = state.backend.content_size(&native);
            state.resize(window, size);
            (Rc::clone(&state.backend), native)
        };
        backend.show_window(&native);
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        let backend = Rc::clone(&self.state.borrow().backend);
        backend.run();
    }
}

impl<B: Backend> UiState<B> {
    fn invalidate(&mut self, window: WindowId) {
        self.windows[window.0].needs_layout = true;
    }

    fn resize(&mut self, window: WindowId, size: Size) {
        self.windows[window.0].viewport = Some(size);
        self.layout_window(window);
    }

    fn layout_window(&mut self, window: WindowId) {
        let Self {
            backend,
            layout,
            windows,
        } = self;
        let window = &mut windows[window.0];
        let Some(viewport) = window.viewport else {
            return;
        };
        window.needs_layout = false;
        layout.layout_window(window.root, viewport, |widget, frame| {
            backend.set_frame(widget, frame);
        });
    }
}

impl<B: Backend> Clone for Ui<B> {
    fn clone(&self) -> Self {
        Self {
            state: Rc::clone(&self.state),
        }
    }
}

impl<B: Backend> fmt::Debug for Ui<B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ui").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Frame, Point};
    use crate::style::{Align, Axis, Justify};
    use crate::testing::{FakeBackend, FakeKind};

    fn frame(x: f32, y: f32, width: f32, height: f32) -> Frame {
        Frame {
            origin: Point { x, y },
            size: Size::new(width, height),
        }
    }

    fn column(ui: &Ui<FakeBackend>, window: WindowId, children: &[Node]) -> Node {
        let style = StackStyle {
            spacing: 10.0,
            align: Align::Center,
            justify: Justify::Center,
            ..StackStyle::new(Axis::Vertical)
        };
        let stack = ui.stack(window, &style);
        for (index, child) in children.iter().enumerate() {
            ui.insert(stack, index, *child);
        }
        stack
    }

    fn open(ui: &Ui<FakeBackend>, width: f32, height: f32) -> WindowId {
        ui.create_window("test", Size::new(width, height))
    }

    #[test]
    fn centers_children_with_spacing() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let abc = ui.label(window, "abc", TextStyle::Body);
        let hello = ui.label(window, "hello", TextStyle::Body);
        let content = column(&ui, window, &[abc, hello]);
        ui.set_content(window, content);
        ui.show(window);

        assert_eq!(fake.frame(0), Some(frame(85.0, 25.0, 30.0, 20.0)));
        assert_eq!(fake.frame(1), Some(frame(75.0, 55.0, 50.0, 20.0)));
    }

    #[test]
    fn text_changes_are_laid_out_on_flush() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let label = ui.label(window, "n=9", TextStyle::Body);
        let content = column(&ui, window, &[label]);
        ui.set_content(window, content);
        ui.show(window);

        ui.set_text(label, "n=10");
        assert_eq!(fake.text(0), "n=10");
        assert_eq!(fake.frame(0).map(|f| f.size.width), Some(30.0));

        ui.flush();
        assert_eq!(fake.frame(0).map(|f| f.size.width), Some(40.0));
    }

    #[test]
    fn inserts_and_removes_children() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 200.0);
        let first = ui.label(window, "a", TextStyle::Body);
        let last = ui.label(window, "c", TextStyle::Body);
        let content = column(&ui, window, &[first, last]);
        ui.set_content(window, content);
        ui.show(window);

        let middle = ui.label(window, "b", TextStyle::Body);
        ui.insert(content, 1, middle);
        ui.flush();
        assert_eq!(fake.visible_texts(FakeKind::Label), ["a", "b", "c"]);

        ui.remove(middle);
        ui.flush();
        assert_eq!(fake.visible_texts(FakeKind::Label), ["a", "c"]);
        assert!(fake.is_removed(2));
    }

    #[test]
    fn removing_a_stack_removes_its_widgets() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 200.0);
        let label = ui.label(window, "a", TextStyle::Body);
        let inner = column(&ui, window, &[label]);
        let content = column(&ui, window, &[inner]);
        ui.set_content(window, content);
        ui.show(window);

        ui.remove(inner);
        assert!(fake.is_removed(0));
    }

    #[test]
    fn spacer_pushes_siblings_apart_and_divider_stretches() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let top = ui.label(window, "a", TextStyle::Body);
        let spacer = ui.spacer(window);
        let divider = ui.divider(window);
        let content = ui.stack(window, &StackStyle::new(Axis::Vertical));
        for (index, child) in [top, spacer, divider].into_iter().enumerate() {
            ui.insert(content, index, child);
        }
        ui.set_content(window, content);
        ui.show(window);

        assert_eq!(fake.frame(1), Some(frame(0.0, 99.0, 100.0, 1.0)));
    }

    #[test]
    fn resizing_lays_out_immediately() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let label = ui.label(window, "a", TextStyle::Body);
        let style = StackStyle {
            justify: Justify::End,
            ..StackStyle::new(Axis::Vertical)
        };
        let content = ui.stack(window, &style);
        ui.insert(content, 0, label);
        ui.set_content(window, content);
        ui.show(window);
        assert_eq!(fake.frame(0).map(|f| f.origin.y), Some(80.0));

        fake.resize(0, Size::new(100.0, 300.0));
        assert_eq!(fake.frame(0).map(|f| f.origin.y), Some(280.0));
    }

    #[test]
    fn controls_report_user_input() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let typed = Rc::new(RefCell::new(String::new()));
        let toggled = Rc::new(RefCell::new(None));
        let _field = ui.text_field(window, "", "Name", {
            let typed = Rc::clone(&typed);
            Box::new(move |text| text.clone_into(&mut typed.borrow_mut()))
        });
        let _checkbox = ui.checkbox(window, "Done", false, {
            let toggled = Rc::clone(&toggled);
            Box::new(move |checked| *toggled.borrow_mut() = Some(checked))
        });

        fake.type_text(0, "Ada");
        fake.toggle(1);

        assert_eq!(*typed.borrow(), "Ada");
        assert_eq!(*toggled.borrow(), Some(true));
    }
}
