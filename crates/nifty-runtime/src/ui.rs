//! The retained UI that apps drive: create native widgets, arrange them, update them.
//! Changes are laid out together when [`Ui::flush`] runs, once per event.

use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;

use crate::backend::{
    Backend, ClickHandler, MenuHandler, NumberHandler, Parent, TextHandler, ToggleHandler,
    WakeHandler,
};
use crate::dialog::ShowDialog;
use crate::geometry::Size;
use crate::layout::{Layout, Placed};
use crate::menu::{Menu, MenuEntry};
use crate::paths::expand_home;
use crate::style::{
    Backdrop, ButtonStyle, Icon, ImageSize, LabelStyle, StackStyle, Surface, ToggleStyle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(usize);

/// A widget, stack or spacer inside one window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Node {
    window: WindowId,
    id: taffy::NodeId,
}

/// Where a new node goes: at `index` among the children of `parent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Position {
    pub parent: Node,
    pub index: usize,
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

const OPEN_WINDOW: &str = "windows are not used after they are closed";
/// Sizes closer than this, in points, look the same on screen.
const UNSEEN_CHANGE: f32 = 0.5;
/// The percentage at which a progress bar is full.
const FULL: i64 = 100;

/// Decides what happens when the user asks to close a window.
pub type WindowCloseHandler = Rc<dyn Fn(WindowId)>;

struct UiState<B: Backend> {
    backend: Rc<B>,
    layout: Layout<B::Widget>,
    /// Indexed by [`WindowId`]; `None` once closed, so the ids of other windows stay valid.
    windows: Vec<Option<WindowState<B>>>,
    /// What runs when the user asks to close a window; without one, it closes.
    on_close: Option<WindowCloseHandler>,
}

struct WindowState<B: Backend> {
    native: Rc<B::Window>,
    root: taffy::NodeId,
    /// `None` until the window is shown and reports its size.
    viewport: Option<Size>,
    /// The narrowest content width last given to the backend.
    min_width: f32,
    /// The width the window's scroll areas last left for their scroll bars.
    scroll_bar_width: f32,
    needs_layout: bool,
}

impl<B: Backend> Ui<B> {
    pub fn new(backend: B) -> Self {
        Self {
            state: Rc::new(RefCell::new(UiState {
                backend: Rc::new(backend),
                layout: Layout::new(),
                windows: Vec::new(),
                on_close: None,
            })),
        }
    }

    /// Opens a hidden window; fill it with [`Ui::set_content`], then [`Ui::show`] it.
    #[must_use = "a window must be filled with `set_content` and shown"]
    pub fn create_window(&self, title: &str, size: Size) -> WindowId {
        let mut state = self.state.borrow_mut();
        let window = WindowId(state.windows.len());
        let weak_state = Rc::downgrade(&self.state);
        let on_resize = Box::new(move |size| {
            if let Some(state) = weak_state.upgrade() {
                state.borrow_mut().resize(window, size);
            }
        });
        let weak_state = Rc::downgrade(&self.state);
        let on_close = Box::new(move || {
            if let Some(state) = weak_state.upgrade() {
                Self { state }.close_requested(window);
            }
        });
        let native = state.backend.open_window(title, size, on_resize, on_close);
        let scroll_bar_width = state.backend.scroll_bar_width(&native);
        let root = state.layout.add_root();
        state.windows.push(Some(WindowState {
            native: Rc::new(native),
            root,
            viewport: None,
            min_width: 0.0,
            scroll_bar_width,
            needs_layout: true,
        }));
        window
    }

    /// A stack to fill the window with, through [`Ui::set_content`].
    #[must_use = "the content must be shown with `set_content`"]
    pub fn content(&self, window: WindowId, style: &StackStyle) -> Node {
        let mut state = self.state.borrow_mut();
        let UiState {
            backend,
            layout,
            windows,
            ..
        } = &mut *state;
        let native = &*live_window(windows, window).native;
        let surface = surface(&**backend, Parent::Window(native), style);
        let id = layout.add_stack(style, surface);
        Node { window, id }
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn stack(&self, at: Position, style: &StackStyle) -> Node {
        self.add(
            at,
            |backend, parent, _| surface(backend, parent, style),
            |layout, surface| layout.add_stack(style, surface),
        )
    }

    /// A stack whose children scroll up and down when they are taller than it.
    #[must_use = "the node is needed to change or remove it"]
    pub fn scroll(&self, at: Position, style: &StackStyle) -> Node {
        self.add(
            at,
            |backend, parent, window| {
                (
                    surface(backend, parent, style),
                    backend.create_scroll(parent),
                    backend.scroll_bar_width(window),
                )
            },
            |layout, (surface, scroll, bar_width)| {
                layout.add_scroll(style, surface, scroll, bar_width)
            },
        )
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn spacer(&self, at: Position) -> Node {
        self.add(at, |_, _, _| (), |layout, ()| layout.add_spacer())
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn label(&self, at: Position, text: &str, style: LabelStyle) -> Node {
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_label(parent, text, style)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn button(
        &self,
        at: Position,
        title: &str,
        look: (ButtonStyle, Icon),
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> Node {
        let (style, icon) = look;
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_button(parent, title, style, icon, selected, on_click)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn nav_item(
        &self,
        at: Position,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> Node {
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_nav_item(parent, title, icon, badge, selected, on_click)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn text_field(
        &self,
        at: Position,
        text: &str,
        (placeholder, lines): (&str, u32),
        on_change: TextHandler,
        on_submit: Option<ClickHandler>,
    ) -> Node {
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_text_field(parent, text, placeholder, lines, on_change, on_submit)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn checkbox(
        &self,
        at: Position,
        title: &str,
        checked: bool,
        style: ToggleStyle,
        on_toggle: ToggleHandler,
    ) -> Node {
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_checkbox(parent, title, checked, style, on_toggle)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn slider(
        &self,
        at: Position,
        value: i64,
        range: (i64, i64),
        on_change: NumberHandler,
    ) -> Node {
        let (min, max) = range;
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_slider(parent, value, min, max, on_change)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn picker(
        &self,
        at: Position,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> Node {
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_picker(parent, options, selected, on_pick)
        })
    }

    /// A progress bar at `percent`, kept between 0 and 100, or busy without one.
    #[must_use = "the node is needed to change or remove it"]
    pub fn progress(&self, at: Position, percent: Option<i64>) -> Node {
        let percent = percent.map(as_percent);
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_progress(parent, percent)
        })
    }

    /// The image in the file at `path`, where `~` stands for the home folder, shown at
    /// `size`. An empty path, or one whose home folder can't be found, shows nothing, as a
    /// file that can't be read as an image does.
    #[must_use = "the node is needed to change or remove it"]
    pub fn image(&self, at: Position, path: &str, size: ImageSize) -> Node {
        let path = image_path(path);
        self.add_widget(at, Layout::add_widget, |backend, parent| {
            backend.create_image(parent, path.as_deref(), size)
        })
    }

    /// An area covering the stack `at` is in, which shows a menu of `entries` on a right
    /// click.
    #[must_use = "the node is needed to change or remove it"]
    pub fn context_menu(
        &self,
        at: Position,
        entries: &[MenuEntry<usize>],
        on_select: MenuHandler,
    ) -> Node {
        self.add_widget(at, Layout::add_overlay, |backend, parent| {
            backend.create_context_menu(parent, entries, on_select)
        })
    }

    #[must_use = "the node is needed to change or remove it"]
    pub fn divider(&self, at: Position) -> Node {
        self.add_widget(at, Layout::add_divider, |backend, parent| {
            backend.create_divider(parent)
        })
    }

    /// Creates a widget in the parent `at` names, and places it there.
    fn add_widget(
        &self,
        at: Position,
        place: fn(&mut Layout<B::Widget>, B::Widget, Size) -> taffy::NodeId,
        create: impl FnOnce(&B, Parent<'_, B>) -> B::Widget,
    ) -> Node {
        self.add(
            at,
            |backend, parent, _| {
                let widget = create(backend, parent);
                let natural = backend.measure(&widget);
                (widget, natural)
            },
            |layout, (widget, natural)| place(layout, widget, natural),
        )
    }

    /// Makes a node in two steps, `create` making its native parts in the parent `at`
    /// names, and `place` adding them to the layout, then places it there.
    fn add<Made>(
        &self,
        at: Position,
        create: impl FnOnce(&B, Parent<'_, B>, &B::Window) -> Made,
        place: impl FnOnce(&mut Layout<B::Widget>, Made) -> taffy::NodeId,
    ) -> Node {
        let window = at.parent.window;
        let mut state = self.state.borrow_mut();
        let UiState {
            backend,
            layout,
            windows,
            ..
        } = &mut *state;
        let native = &*live_window(windows, window).native;
        let parent = layout
            .scroll_around(at.parent.id)
            .map_or(Parent::Window(native), Parent::Scroll);
        let made = create(&**backend, parent, native);
        let id = place(layout, made);
        layout.insert(at.parent.id, at.index, id);
        state.invalidate(window);
        Node { window, id }
    }

    /// Makes `content` fill the window, removing whatever it showed before.
    pub fn set_content(&self, window: WindowId, content: Node) {
        let mut state = self.state.borrow_mut();
        let root = state.window(window).root;
        let previous = state.layout.set_content(root, content.id);
        state.invalidate(window);
        if let Some(previous) = previous {
            state.remove_tree(previous);
        }
    }

    /// Removes `node` and everything inside it from the window.
    pub fn remove(&self, node: Node) {
        let mut state = self.state.borrow_mut();
        state.remove_tree(node.id);
        state.invalidate(node.window);
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_text(&self, node: Node, text: &str) {
        self.change_content(node, |backend, widget| backend.set_text(widget, text));
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_badge(&self, node: Node, badge: &str) {
        self.change_content(node, |backend, widget| backend.set_badge(widget, badge));
    }

    /// Shows the image at `path` instead, as [`Ui::image`] does.
    ///
    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_image(&self, node: Node, path: &str) {
        let path = image_path(path);
        self.change_content(node, |backend, widget| {
            backend.set_image(widget, path.as_deref());
        });
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_options(&self, node: Node, options: &[String]) {
        self.change_content(node, |backend, widget| {
            backend.set_options(widget, options);
        });
    }

    /// Changes what a widget shows, then lays its window out again if that changed its size.
    fn change_content(&self, node: Node, change: impl FnOnce(&B, &B::Widget)) {
        let mut state = self.state.borrow_mut();
        let state = &mut *state;
        let widget = state
            .layout
            .widget(node.id)
            .expect("only widgets show content");
        change(&state.backend, widget);
        let natural = state.backend.measure(widget);
        if state.layout.set_natural(node.id, natural) {
            state.invalidate(node.window);
        }
    }

    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_checked(&self, node: Node, checked: bool) {
        self.change_state(node, |backend, widget| backend.set_checked(widget, checked));
    }

    /// Fills a progress bar to `percent`, kept between 0 and 100.
    ///
    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_progress(&self, node: Node, percent: i64) {
        self.change_state(node, |backend, widget| {
            backend.set_value(widget, as_percent(percent));
        });
    }

    /// Moves a slider to `value`.
    ///
    /// # Panics
    /// When `node` is a stack or spacer rather than a widget.
    pub fn set_value(&self, node: Node, value: i64) {
        self.change_state(node, |backend, widget| backend.set_value(widget, value));
    }

    /// Changes a widget's state, such as whether it's checked, which leaves its size alone.
    fn change_state(&self, node: Node, change: impl FnOnce(&B, &B::Widget)) {
        let state = self.state.borrow();
        let widget = state
            .layout
            .widget(node.id)
            .expect("only widgets have a state");
        change(&state.backend, widget);
    }

    /// Lays out every window changed since the last flush.
    pub fn flush(&self) {
        let mut state = self.state.borrow_mut();
        for index in 0..state.windows.len() {
            if state.windows[index]
                .as_ref()
                .is_some_and(|window| window.needs_layout)
            {
                state.layout_window(WindowId(index));
            }
        }
    }

    /// Shows the window, first widening it if its content doesn't fit.
    pub fn show(&self, window: WindowId) {
        let (backend, native, size, min_width) = {
            let mut state = self.state.borrow_mut();
            let native = Rc::clone(&state.window(window).native);
            let size = state.backend.content_size(&native);
            state.resize(window, size);
            let min_width = state.window(window).min_width;
            (Rc::clone(&state.backend), native, size, min_width)
        };
        if size.width < min_width {
            // The backend reports the resize through `on_resize`, which borrows the state.
            backend.set_content_size(&native, Size::new(min_width, size.height));
        }
        backend.show_window(&native);
    }

    pub fn set_title(&self, window: WindowId, title: &str) {
        let state = self.state.borrow();
        state.backend.set_title(&state.window(window).native, title);
    }

    pub fn set_backdrop(&self, window: WindowId, backdrop: Backdrop) {
        let state = self.state.borrow();
        state
            .backend
            .set_backdrop(&state.window(window).native, backdrop);
    }

    /// Resizes the window's content area and lays it out at the new size.
    pub fn set_size(&self, window: WindowId, size: Size) {
        // The backend reports the resize through `on_resize`, which borrows the state again.
        let (backend, native) = {
            let state = self.state.borrow();
            (
                Rc::clone(&state.backend),
                Rc::clone(&state.window(window).native),
            )
        };
        backend.set_content_size(&native, size);
    }

    /// Removes the window's widgets and closes it. Its id must not be used again.
    ///
    /// # Panics
    /// When the window is already closed.
    /// Has `on_close` decide what happens when the user asks to close a window, in place
    /// of closing it.
    pub fn set_close_handler(&self, on_close: WindowCloseHandler) {
        self.state.borrow_mut().on_close = Some(on_close);
    }

    /// The user asked to close `window`, which may have closed since the request set out.
    fn close_requested(&self, window: WindowId) {
        let on_close = {
            let state = self.state.borrow();
            if state.windows[window.0].is_none() {
                return;
            }
            state.on_close.clone()
        };
        match on_close {
            Some(on_close) => on_close(window),
            None => self.close(window),
        }
    }

    /// Brings a window already shown to the front.
    pub fn focus(&self, window: WindowId) {
        let state = self.state.borrow();
        state.backend.focus_window(&state.window(window).native);
    }

    /// Closes the window and removes everything in it.
    ///
    /// # Panics
    /// When the window is closed already.
    pub fn close(&self, window: WindowId) {
        let mut state = self.state.borrow_mut();
        let closed = state.windows[window.0].take().expect(OPEN_WINDOW);
        state.remove_tree(closed.root);
        let native = Rc::into_inner(closed.native)
            .expect("`show` and `set_size` hold a window only while they run");
        state.backend.close_window(native);
    }

    /// Gives the app `menus`; see [`Backend::set_menu_bar`].
    pub fn set_menu_bar(&self, menus: &[Menu<usize>], on_select: MenuHandler) {
        let backend = Rc::clone(&self.state.borrow().backend);
        backend.set_menu_bar(menus, on_select);
    }

    /// Returns something any thread can use to have `on_wake` run on the UI thread.
    #[must_use = "dropping the waker means `on_wake` never runs"]
    pub fn waker(&self, on_wake: WakeHandler) -> B::Waker {
        self.state.borrow().backend.waker(on_wake)
    }

    /// The platform's file dialogs, for code that isn't generic over the backend.
    #[must_use]
    pub fn dialogs(&self) -> Rc<dyn ShowDialog> {
        Rc::clone(&self.state.borrow().backend) as Rc<dyn ShowDialog>
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        let backend = Rc::clone(&self.state.borrow().backend);
        backend.run();
    }
}

/// The surface drawn behind a stack's children, if it has one.
fn surface<B: Backend>(
    backend: &B,
    parent: Parent<'_, B>,
    style: &StackStyle,
) -> Option<B::Widget> {
    (style.surface != Surface::None).then(|| backend.create_surface(parent, style.surface))
}

fn live_window<B: Backend>(
    windows: &[Option<WindowState<B>>],
    window: WindowId,
) -> &WindowState<B> {
    windows[window.0].as_ref().expect(OPEN_WINDOW)
}

/// Where an image's file is, with `~` standing for the home folder; `None` for no file.
fn image_path(path: &str) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    expand_home(path).ok()
}

/// Progress bars show a share of their width, so a percentage out of range shows as full or
/// empty.
fn as_percent(value: i64) -> i64 {
    value.clamp(0, FULL)
}

impl<B: Backend> UiState<B> {
    fn window(&self, window: WindowId) -> &WindowState<B> {
        live_window(&self.windows, window)
    }

    fn invalidate(&mut self, window: WindowId) {
        if let Some(window) = &mut self.windows[window.0] {
            window.needs_layout = true;
        }
    }

    /// A closed window can still report a resize that was already on its way, so that is
    /// ignored rather than treated as a mistake.
    fn resize(&mut self, window: WindowId, size: Size) {
        let Some(open) = &mut self.windows[window.0] else {
            return;
        };
        open.viewport = Some(size);
        self.layout_window(window);
    }

    /// Removes `node` and everything under it, taking their widgets off screen.
    fn remove_tree(&mut self, node: taffy::NodeId) {
        for widget in self.layout.remove(node) {
            self.backend.remove_widget(widget);
        }
    }

    fn layout_window(&mut self, window: WindowId) {
        let Self {
            backend,
            layout,
            windows,
            ..
        } = self;
        let window = windows[window.0].as_mut().expect(OPEN_WINDOW);
        let Some(viewport) = window.viewport else {
            return;
        };
        window.needs_layout = false;
        let scroll_bar_width = backend.scroll_bar_width(&window.native);
        if (scroll_bar_width - window.scroll_bar_width).abs() >= UNSEEN_CHANGE {
            window.scroll_bar_width = scroll_bar_width;
            layout.set_scroll_bar_width(window.root, scroll_bar_width);
        }
        let title_bar_inset = backend.title_bar_inset(&window.native);
        let min_width = layout.min_width(window.root, title_bar_inset);
        if (min_width - window.min_width).abs() >= UNSEEN_CHANGE {
            window.min_width = min_width;
            backend.set_min_content_width(&window.native, min_width);
        }
        layout.layout_window(
            window.root,
            viewport,
            title_bar_inset,
            |placed| match placed {
                Placed::Frame(widget, frame) => backend.set_frame(widget, frame),
                Placed::ScrollContent(scroll, size) => backend.set_scroll_content(scroll, size),
            },
        );
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
    use crate::testing::{FakeBackend, FakeKind, SCROLL_BAR_WIDTH};

    fn frame(x: f32, y: f32, width: f32, height: f32) -> Frame {
        Frame {
            origin: Point { x, y },
            size: Size::new(width, height),
        }
    }

    fn at(parent: Node, index: usize) -> Position {
        Position { parent, index }
    }

    fn open(ui: &Ui<FakeBackend>, width: f32, height: f32) -> WindowId {
        ui.create_window("test", Size::new(width, height))
    }

    /// Fills the window with a stack of `style`.
    fn fill(ui: &Ui<FakeBackend>, window: WindowId, style: &StackStyle) -> Node {
        let content = ui.content(window, style);
        ui.set_content(window, content);
        content
    }

    /// A centred column with some spacing, filling the window.
    fn column(ui: &Ui<FakeBackend>, window: WindowId) -> Node {
        let style = StackStyle {
            spacing: 10.0,
            align: Align::Center,
            justify: Justify::Center,
            ..StackStyle::new(Axis::Vertical)
        };
        fill(ui, window, &style)
    }

    fn label(ui: &Ui<FakeBackend>, at: Position, text: &str) -> Node {
        ui.label(at, text, LabelStyle::default())
    }

    #[test]
    fn centers_children_with_spacing() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let content = column(&ui, window);
        label(&ui, at(content, 0), "abc");
        label(&ui, at(content, 1), "hello");
        ui.show(window);

        assert_eq!(fake.frame(0), Some(frame(85.0, 25.0, 30.0, 20.0)));
        assert_eq!(fake.frame(1), Some(frame(75.0, 55.0, 50.0, 20.0)));
    }

    #[test]
    fn align_places_children_across_the_stack() {
        let cases = [
            (Align::Stretch, frame(0.0, 0.0, 200.0, 20.0)),
            (Align::Start, frame(0.0, 0.0, 30.0, 20.0)),
            (Align::Center, frame(85.0, 0.0, 30.0, 20.0)),
            (Align::End, frame(170.0, 0.0, 30.0, 20.0)),
        ];
        for (align, expected) in cases {
            let fake = FakeBackend::default();
            let ui = Ui::new(fake.clone());
            let window = open(&ui, 200.0, 100.0);
            let style = StackStyle {
                align,
                ..StackStyle::new(Axis::Vertical)
            };
            let content = fill(&ui, window, &style);
            label(&ui, at(content, 0), "abc");
            ui.show(window);

            assert_eq!(fake.frame(0), Some(expected), "{align:?}");
        }
    }

    #[test]
    fn surfaces_reach_under_the_title_bar_only_from_the_top() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 300.0, 200.0);
        fake.set_title_bar_inset(0, 28.0);
        let row = fill(&ui, window, &StackStyle::new(Axis::Horizontal));
        let sidebar = ui.stack(
            at(row, 0),
            &StackStyle {
                surface: Surface::Sidebar,
                width: 100.0,
                padding: 10.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        label(&ui, at(sidebar, 0), "abc");
        let main = ui.stack(
            at(row, 1),
            &StackStyle {
                surface: Surface::Content,
                grow: true,
                padding: 20.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        let card = ui.stack(
            at(main, 0),
            &StackStyle {
                surface: Surface::Card,
                padding: 5.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        label(&ui, at(card, 0), "x");
        ui.show(window);

        assert_eq!(fake.style(0).as_deref(), Some("Sidebar"));
        assert_eq!(fake.frame(0), Some(frame(0.0, 0.0, 100.0, 200.0)));
        assert_eq!(fake.frame(1), Some(frame(10.0, 38.0, 80.0, 20.0)));
        assert_eq!(fake.frame(2), Some(frame(100.0, 0.0, 200.0, 200.0)));
        assert_eq!(fake.frame(3), Some(frame(120.0, 48.0, 160.0, 30.0)));
        assert_eq!(fake.frame(4), Some(frame(125.0, 53.0, 150.0, 20.0)));
    }

    #[test]
    fn stacks_take_a_fixed_width_or_fill_up_to_a_limit() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 400.0, 100.0);
        let centered = fill(
            &ui,
            window,
            &StackStyle {
                align: Align::Center,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        let _limited = ui.stack(
            at(centered, 0),
            &StackStyle {
                surface: Surface::Card,
                max_width: 200.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        let _fixed = ui.stack(
            at(centered, 1),
            &StackStyle {
                surface: Surface::Card,
                width: 50.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        ui.show(window);

        assert_eq!(fake.frame(0), Some(frame(100.0, 0.0, 200.0, 0.0)));
        assert_eq!(fake.frame(1), Some(frame(175.0, 0.0, 50.0, 0.0)));

        fake.resize(0, Size::new(150.0, 100.0));
        assert_eq!(fake.frame(0), Some(frame(0.0, 0.0, 150.0, 0.0)));
    }

    #[test]
    fn scroll_areas_take_the_space_left_and_scroll_their_content() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let content = fill(&ui, window, &StackStyle::new(Axis::Vertical));
        label(&ui, at(content, 0), "title");
        let list = ui.scroll(
            at(content, 1),
            &StackStyle {
                padding: 5.0,
                spacing: 10.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        for index in 0..4 {
            label(&ui, at(list, index), "row");
        }
        label(&ui, at(content, 2), "footer");
        ui.show(window);

        let scroll = fake.find(FakeKind::Scroll)[0];
        assert_eq!(fake.frame(scroll), Some(frame(0.0, 20.0, 200.0, 60.0)));
        let rows: Vec<usize> = (0..fake.widget_count())
            .filter(|&widget| fake.host(widget) == Some(scroll))
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            fake.frame(rows[1]),
            Some(frame(5.0, 35.0, 200.0 - SCROLL_BAR_WIDTH - 10.0, 20.0)),
            "rows are placed in the scroll area's content, beside its scroll bar"
        );
        assert_eq!(
            fake.scroll_content(scroll),
            Some(Size::new(
                200.0 - SCROLL_BAR_WIDTH,
                5.0 + 4.0 * 20.0 + 3.0 * 10.0 + 5.0
            ))
        );
        assert_eq!(
            fake.window_frame(rows[3]).origin.y,
            20.0 + 95.0,
            "past the bottom of the area until it scrolls"
        );

        fake.set_scroll_bar_width(0.0);
        fake.resize(0, Size::new(200.0, 300.0));
        assert_eq!(fake.frame(scroll), Some(frame(0.0, 20.0, 200.0, 260.0)));
        assert_eq!(
            fake.scroll_content(scroll),
            Some(Size::new(200.0, 260.0)),
            "short content still fills the area, and overlay scroll bars leave it all"
        );
    }

    #[test]
    fn windows_stay_wide_enough_for_their_content() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 50.0, 100.0);
        let content = column(&ui, window);
        let text = label(&ui, at(content, 0), "abcdefghij");
        ui.show(window);

        assert_eq!(fake.min_widths(0), [100.0]);
        assert_eq!(fake.window_size(0), Size::new(100.0, 100.0));

        ui.set_text(text, "abcdefghijk");
        ui.flush();
        ui.set_text(text, "abcdefghijk");
        ui.flush();
        assert_eq!(fake.min_widths(0), [100.0, 110.0]);
    }

    #[test]
    fn text_changes_are_laid_out_on_flush() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let content = column(&ui, window);
        let text = label(&ui, at(content, 0), "n=9");
        ui.show(window);

        ui.set_text(text, "n=10");
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
        let content = column(&ui, window);
        label(&ui, at(content, 0), "a");
        label(&ui, at(content, 1), "c");
        ui.show(window);

        let middle = label(&ui, at(content, 1), "b");
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
        let content = column(&ui, window);
        let inner = ui.stack(at(content, 0), &StackStyle::new(Axis::Vertical));
        label(&ui, at(inner, 0), "a");
        ui.show(window);

        ui.remove(inner);
        assert!(fake.is_removed(0));
    }

    #[test]
    fn spacer_pushes_siblings_apart_and_divider_stretches() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let content = fill(&ui, window, &StackStyle::new(Axis::Vertical));
        label(&ui, at(content, 0), "a");
        let _spacer = ui.spacer(at(content, 1));
        let _divider = ui.divider(at(content, 2));
        ui.show(window);

        assert_eq!(fake.frame(1), Some(frame(0.0, 99.0, 100.0, 1.0)));
    }

    #[test]
    fn resizing_lays_out_immediately() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let style = StackStyle {
            justify: Justify::End,
            ..StackStyle::new(Axis::Vertical)
        };
        let content = fill(&ui, window, &style);
        label(&ui, at(content, 0), "a");
        ui.show(window);
        assert_eq!(fake.frame(0).map(|f| f.origin.y), Some(80.0));

        fake.resize(0, Size::new(100.0, 300.0));
        assert_eq!(fake.frame(0).map(|f| f.origin.y), Some(280.0));
    }

    #[test]
    fn new_content_replaces_and_removes_the_old() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let old_content = column(&ui, window);
        label(&ui, at(old_content, 0), "old");
        ui.show(window);

        let new_content = column(&ui, window);
        label(&ui, at(new_content, 0), "new");
        ui.flush();

        assert!(fake.is_removed(0));
        assert_eq!(fake.visible_texts(FakeKind::Label), ["new"]);
        assert_eq!(fake.frame(1), Some(frame(85.0, 40.0, 30.0, 20.0)));
    }

    #[test]
    fn windows_can_be_retitled_resized_and_closed() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let first = open(&ui, 100.0, 100.0);
        let second = open(&ui, 100.0, 100.0);
        let content = column(&ui, second);
        label(&ui, at(content, 0), "a");
        ui.show(second);

        ui.set_title(first, "Renamed");
        ui.set_size(second, Size::new(300.0, 100.0));
        assert_eq!(fake.window_title(0), "Renamed");
        assert_eq!(fake.frame(0).map(|f| f.origin.x), Some(145.0));

        ui.close(second);
        assert!(fake.is_closed(1));
        assert!(fake.is_removed(0));
        fake.resize(1, Size::new(50.0, 50.0));
        assert!(!fake.is_closed(0));
    }

    #[test]
    fn wakers_run_their_handler_on_the_ui_thread() {
        use crate::backend::Waker as _;

        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let wake_count = Rc::new(RefCell::new(0));
        let waker = ui.waker({
            let wake_count = Rc::clone(&wake_count);
            Box::new(move || *wake_count.borrow_mut() += 1)
        });

        std::thread::scope(|scope| {
            scope.spawn(|| waker.wake());
        });
        assert_eq!(
            *wake_count.borrow(),
            0,
            "the handler waits for the UI thread"
        );
        fake.run_pending_wakes();
        fake.run_pending_wakes();
        assert_eq!(*wake_count.borrow(), 1);
    }

    #[test]
    fn progress_stays_between_empty_and_full() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let content = column(&ui, window);
        let bar = ui.progress(at(content, 0), Some(150));
        let _busy = ui.progress(at(content, 1), None);
        assert_eq!(fake.value(0), Some(100));
        assert_eq!(fake.value(1), None);

        ui.set_progress(bar, -5);
        assert_eq!(fake.value(0), Some(0));
        ui.set_progress(bar, 42);
        assert_eq!(fake.value(0), Some(42));
    }

    #[test]
    fn context_menus_cover_their_stack_and_take_no_room() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 200.0, 100.0);
        let content = fill(&ui, window, &StackStyle::new(Axis::Vertical));
        let card = ui.stack(
            at(content, 0),
            &StackStyle {
                padding: 10.0,
                spacing: 5.0,
                ..StackStyle::new(Axis::Vertical)
            },
        );
        let _menu = ui.context_menu(at(card, 0), &[], Box::new(|_| {}));
        label(&ui, at(card, 1), "a");
        label(&ui, at(card, 2), "b");
        ui.show(window);

        let menu = fake.find(FakeKind::ContextMenu)[0];
        assert_eq!(fake.frame(menu), Some(frame(0.0, 0.0, 200.0, 65.0)));
        let labels = fake.find(FakeKind::Label);
        assert_eq!(fake.frame(labels[0]).map(|f| f.origin.y), Some(10.0));
        assert_eq!(fake.frame(labels[1]).map(|f| f.origin.y), Some(35.0));
    }

    #[test]
    fn controls_report_user_input() {
        let fake = FakeBackend::default();
        let ui = Ui::new(fake.clone());
        let window = open(&ui, 100.0, 100.0);
        let content = column(&ui, window);
        let typed = Rc::new(RefCell::new(String::new()));
        let toggled = Rc::new(RefCell::new(None));
        let _field = ui.text_field(
            at(content, 0),
            "",
            ("Name", 1),
            {
                let typed = Rc::clone(&typed);
                Box::new(move |text| text.clone_into(&mut typed.borrow_mut()))
            },
            None,
        );
        let _checkbox = ui.checkbox(at(content, 1), "Done", false, ToggleStyle::Checkbox, {
            let toggled = Rc::clone(&toggled);
            Box::new(move |checked| *toggled.borrow_mut() = Some(checked))
        });

        fake.type_text(0, "Ada");
        fake.toggle(1);

        assert_eq!(*typed.borrow(), "Ada");
        assert_eq!(*toggled.borrow(), Some(true));
    }
}
