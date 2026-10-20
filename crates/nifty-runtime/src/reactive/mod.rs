//! Shows an app's windows from templates and keeps them in step with its state.
//!
//! Each dynamic part of a template records which state it reads. After every event the
//! engine updates only the widgets whose state changed, and rebuilds regions in place when
//! the state deciding their contents changed. The [`Logic`] gives everything its values,
//! so the interpreter and compiled apps share this engine.

mod template;

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

pub use template::{Logic, Shown, Template, Templates, Widget, WindowTemplate};

use crate::backend::{Backend, ClickHandler, TextHandler, ToggleHandler};
use crate::geometry::Size;
use crate::state::StateSet;
use crate::style::Backdrop;
use crate::ui::{Node, Ui, WindowId};

pub type ErrorHandler<E> = Rc<dyn Fn(E)>;

/// A running app: its logic, its windows, and what each widget shows.
pub struct Engine<L: Logic, B: Backend> {
    logic: L,
    ui: Ui<B>,
    windows: RefCell<Vec<MountedWindow<L>>>,
    on_error: ErrorHandler<L::Error>,
    this: Weak<Self>,
}

struct MountedWindow<L: Logic> {
    window: WindowId,
    /// The size its template asked for, which a reload compares against.
    declared_size: Size,
    content: Node,
    children: Vec<Mounted<L>>,
}

/// A window of the app being replaced, and the size its template asked for.
#[derive(Clone, Copy)]
struct ReusedWindow {
    window: WindowId,
    declared_size: Size,
}

/// A window whose new content is built but not yet showing.
struct PreparedWindow<L: Logic> {
    mounted: MountedWindow<L>,
    title: String,
    backdrop: Backdrop,
    reused: Option<ReusedWindow>,
}

/// A template and the list holding it, so mounted UI can re-read it later.
struct TemplateRef<L: Logic> {
    templates: Templates<L>,
    index: usize,
}

impl<L: Logic> TemplateRef<L> {
    fn template(&self) -> &Template<L> {
        &self.templates[self.index]
    }
}

impl<L: Logic> Clone for TemplateRef<L> {
    fn clone(&self) -> Self {
        Self {
            templates: Rc::clone(&self.templates),
            index: self.index,
        }
    }
}

enum Mounted<L: Logic> {
    /// A widget; `live` is kept when what it shows can change.
    Widget {
        node: Node,
        live: Option<(TemplateRef<L>, Rc<L::Frame>)>,
    },
    Stack {
        node: Node,
        children: Vec<Mounted<L>>,
    },
    /// What a region shows, placed directly among its parent's children.
    Region {
        template: TemplateRef<L>,
        frame: Rc<L::Frame>,
        children: Vec<Mounted<L>>,
    },
}

impl<L: Logic, B: Backend> Engine<L, B> {
    /// Shows every window of `windows`, with `make_logic` giving their dynamic parts values.
    /// It receives the UI and the engine-to-be, for logic that wakes the app later.
    ///
    /// # Errors
    /// When building the first UI fails.
    pub fn start(
        ui: Ui<B>,
        windows: Vec<WindowTemplate<L>>,
        on_error: ErrorHandler<L::Error>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
    ) -> Result<Rc<Self>, L::Error> {
        Self::open(ui, windows, on_error, make_logic, &[])
    }

    /// Shows `windows` in this app's windows, in order, in place of this app. Windows it
    /// has beyond them close; extra ones open.
    ///
    /// # Errors
    /// When building the new UI fails. Nothing has changed then, and this app keeps running.
    pub fn reload(
        &self,
        windows: Vec<WindowTemplate<L>>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
    ) -> Result<Rc<Self>, L::Error> {
        let reused: Vec<ReusedWindow> = self
            .windows
            .borrow()
            .iter()
            .map(|mounted| ReusedWindow {
                window: mounted.window,
                declared_size: mounted.declared_size,
            })
            .collect();
        let next = Self::open(
            self.ui.clone(),
            windows,
            Rc::clone(&self.on_error),
            make_logic,
            &reused,
        )?;

        // The new app has replaced this one's content, so this one must not touch it again.
        self.windows.borrow_mut().clear();
        let kept = next.windows.borrow().len();
        for surplus in reused.iter().skip(kept) {
            self.ui.close(surplus.window);
        }
        Ok(next)
    }

    #[must_use]
    pub fn logic(&self) -> &L {
        &self.logic
    }

    /// Runs one event, then updates the UI; errors go to the app's error handler.
    pub fn handle_event(&self, event: impl FnOnce(&L) -> Result<(), L::Error>) {
        if let Err(error) = event(&self.logic).and_then(|()| self.refresh()) {
            (self.on_error)(error);
        }
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        self.ui.run();
    }

    /// Builds every window's content, reusing `reused` in order, and shows it all only
    /// once everything is built, so a failure leaves the windows as they were.
    fn open(
        ui: Ui<B>,
        windows: Vec<WindowTemplate<L>>,
        on_error: ErrorHandler<L::Error>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
        reused: &[ReusedWindow],
    ) -> Result<Rc<Self>, L::Error> {
        let engine = Rc::new_cyclic(|this: &Weak<Self>| Self {
            logic: make_logic(&ui, this.clone()),
            ui,
            windows: RefCell::new(Vec::new()),
            on_error,
            this: this.clone(),
        });

        let mut prepared = Vec::with_capacity(windows.len());
        for (index, template) in windows.into_iter().enumerate() {
            let reused = reused.get(index).copied();
            let window = match reused {
                Some(reused) => reused.window,
                None => engine.ui.create_window(&template.title, template.size),
            };
            let title = template.title.clone();
            let backdrop = template.backdrop;
            match engine.mount_window(window, template) {
                Ok(mounted) => prepared.push(PreparedWindow {
                    mounted,
                    title,
                    backdrop,
                    reused,
                }),
                Err(error) => {
                    if reused.is_none() {
                        engine.ui.close(window);
                    }
                    engine.discard(prepared);
                    return Err(error);
                }
            }
        }

        for PreparedWindow {
            mounted,
            title,
            backdrop,
            reused,
        } in prepared
        {
            engine.ui.set_backdrop(mounted.window, backdrop);
            engine.ui.set_content(mounted.window, mounted.content);
            match reused {
                Some(reused) => {
                    engine.ui.set_title(mounted.window, &title);
                    if reused.declared_size != mounted.declared_size {
                        engine.ui.set_size(mounted.window, mounted.declared_size);
                    }
                }
                None => engine.ui.show(mounted.window),
            }
            engine.windows.borrow_mut().push(mounted);
        }
        engine.ui.flush();
        Ok(engine)
    }

    /// Builds a window's content without placing it in the window yet.
    fn mount_window(
        &self,
        window: WindowId,
        template: WindowTemplate<L>,
    ) -> Result<MountedWindow<L>, L::Error> {
        let content = self.ui.stack(window, &template.style);
        let frame = Rc::new(template.frame);
        match self.mount_list(window, &template.children, &frame, content, &mut 0) {
            Ok(children) => Ok(MountedWindow {
                window,
                declared_size: template.size,
                content,
                children,
            }),
            Err(error) => {
                self.ui.remove(content);
                Err(error)
            }
        }
    }

    /// Throws away content that was built but never shown, closing windows opened for it.
    fn discard(&self, prepared: Vec<PreparedWindow<L>>) {
        for PreparedWindow {
            mounted, reused, ..
        } in prepared
        {
            self.ui.remove(mounted.content);
            if reused.is_none() {
                self.ui.close(mounted.window);
            }
        }
    }

    fn mount_list(
        &self,
        window: WindowId,
        templates: &Templates<L>,
        frame: &Rc<L::Frame>,
        parent: Node,
        offset: &mut usize,
    ) -> Result<Vec<Mounted<L>>, L::Error> {
        (0..templates.len())
            .map(|index| {
                let template = TemplateRef {
                    templates: Rc::clone(templates),
                    index,
                };
                self.mount(window, template, frame, parent, offset)
            })
            .collect()
    }

    /// Creates the UI for `template` at position `offset` in `parent`, advancing `offset`
    /// past what it placed there.
    fn mount(
        &self,
        window: WindowId,
        template: TemplateRef<L>,
        frame: &Rc<L::Frame>,
        parent: Node,
        offset: &mut usize,
    ) -> Result<Mounted<L>, L::Error> {
        match template.template() {
            Template::Stack { style, children } => {
                let node = self.ui.stack(window, style);
                self.place(parent, offset, node);
                let children = self.mount_list(window, children, frame, node, &mut 0)?;
                Ok(Mounted::Stack { node, children })
            }
            Template::Region { region, .. } => {
                let children = self.mount_region(window, region, frame, parent, offset)?;
                Ok(Mounted::Region {
                    template,
                    frame: Rc::clone(frame),
                    children,
                })
            }
            Template::Widget { widget, deps } => {
                let node = self.create_widget(window, widget, frame)?;
                self.place(parent, offset, node);
                let live = (!deps.is_empty()).then(|| (template, Rc::clone(frame)));
                Ok(Mounted::Widget { node, live })
            }
        }
    }

    fn create_widget(
        &self,
        window: WindowId,
        widget: &Widget<L>,
        frame: &Rc<L::Frame>,
    ) -> Result<Node, L::Error> {
        Ok(match widget {
            Widget::Label { text, style } => {
                self.ui
                    .label(window, &self.logic.text(text, frame)?, *style)
            }
            Widget::Button {
                title,
                style,
                icon,
                action,
            } => {
                let title = self.logic.text(title, frame)?;
                let on_click = self.on_click(action.clone(), Rc::clone(frame));
                self.ui.button(window, &title, *style, *icon, on_click)
            }
            Widget::TextField { value, placeholder } => {
                let text = self.logic.read_text(value, frame)?;
                let on_change = self.on_edit(value.clone(), Rc::clone(frame));
                self.ui.text_field(window, &text, placeholder, on_change)
            }
            Widget::Checkbox { checked, title } => {
                let is_checked = self.logic.read_checked(checked, frame)?;
                let title = match title {
                    Some(title) => self.logic.text(title, frame)?,
                    None => String::new(),
                };
                let on_toggle = self.on_toggle(checked.clone(), Rc::clone(frame));
                self.ui.checkbox(window, &title, is_checked, on_toggle)
            }
            Widget::NavItem {
                title,
                value,
                selection,
                icon,
                badge,
            } => {
                let title = self.logic.text(title, frame)?;
                let badge = self.badge(badge.as_ref(), frame)?;
                let selected = self.is_selected(value, selection, frame)?;
                let on_click = self.on_select(selection.clone(), value.clone(), Rc::clone(frame));
                self.ui
                    .nav_item(window, &title, *icon, &badge, selected, on_click)
            }
            Widget::Spacer => self.ui.spacer(window),
            Widget::Divider => self.ui.divider(window),
        })
    }

    fn place(&self, parent: Node, offset: &mut usize, node: Node) {
        self.ui.insert(parent, *offset, node);
        *offset += 1;
    }

    /// Creates what a region currently shows.
    fn mount_region(
        &self,
        window: WindowId,
        region: &L::Region,
        frame: &Rc<L::Frame>,
        parent: Node,
        offset: &mut usize,
    ) -> Result<Vec<Mounted<L>>, L::Error> {
        let mut children = Vec::new();
        for shown in self.logic.expand(region, frame)? {
            children.extend(self.mount_list(
                window,
                &shown.templates,
                &shown.frame,
                parent,
                offset,
            )?);
        }
        Ok(children)
    }

    /// Brings every window up to date with the state changed by the last event.
    fn refresh(&self) -> Result<(), L::Error> {
        let changed = self.logic.take_changed();
        if changed.is_empty() {
            return Ok(());
        }
        let mut windows = self.windows.borrow_mut();
        for mounted in windows.iter_mut() {
            self.refresh_list(
                mounted.window,
                &mut mounted.children,
                mounted.content,
                &changed,
                &mut 0,
            )?;
        }
        self.ui.flush();
        Ok(())
    }

    fn refresh_list(
        &self,
        window: WindowId,
        items: &mut [Mounted<L>],
        parent: Node,
        changed: &StateSet,
        offset: &mut usize,
    ) -> Result<(), L::Error> {
        for item in items {
            match item {
                Mounted::Widget { node, live } => {
                    if let Some((template, frame)) = live
                        && let Template::Widget { widget, deps } = template.template()
                        && deps.intersects(changed)
                    {
                        self.update_widget(*node, widget, frame)?;
                    }
                    *offset += 1;
                }
                Mounted::Stack { node, children } => {
                    self.refresh_list(window, children, *node, changed, &mut 0)?;
                    *offset += 1;
                }
                Mounted::Region {
                    template,
                    frame,
                    children,
                } => match template.template() {
                    Template::Region { region, deps } if deps.intersects(changed) => {
                        for child in children.drain(..) {
                            self.unmount(child);
                        }
                        *children = self.mount_region(window, region, frame, parent, offset)?;
                    }
                    _ => self.refresh_list(window, children, parent, changed, offset)?,
                },
            }
        }
        Ok(())
    }

    fn unmount(&self, mounted: Mounted<L>) {
        match mounted {
            Mounted::Widget { node, .. } | Mounted::Stack { node, .. } => self.ui.remove(node),
            Mounted::Region { children, .. } => {
                for child in children {
                    self.unmount(child);
                }
            }
        }
    }

    fn update_widget(
        &self,
        node: Node,
        widget: &Widget<L>,
        frame: &L::Frame,
    ) -> Result<(), L::Error> {
        match widget {
            Widget::Label { text, .. } | Widget::Button { title: text, .. } => {
                self.ui.set_text(node, &self.logic.text(text, frame)?);
            }
            Widget::TextField { value, .. } => {
                self.ui.set_text(node, &self.logic.read_text(value, frame)?);
            }
            Widget::Checkbox { checked, title } => {
                self.ui
                    .set_checked(node, self.logic.read_checked(checked, frame)?);
                if let Some(title) = title {
                    self.ui.set_text(node, &self.logic.text(title, frame)?);
                }
            }
            Widget::NavItem {
                title,
                value,
                selection,
                badge,
                ..
            } => {
                self.ui.set_text(node, &self.logic.text(title, frame)?);
                self.ui.set_badge(node, &self.badge(badge.as_ref(), frame)?);
                self.ui
                    .set_checked(node, self.is_selected(value, selection, frame)?);
            }
            Widget::Spacer | Widget::Divider => {}
        }
        Ok(())
    }

    fn badge(&self, badge: Option<&L::Text>, frame: &L::Frame) -> Result<String, L::Error> {
        badge.map_or_else(|| Ok(String::new()), |badge| self.logic.text(badge, frame))
    }

    fn is_selected(
        &self,
        value: &L::Text,
        selection: &L::Binding,
        frame: &L::Frame,
    ) -> Result<bool, L::Error> {
        Ok(self.logic.read_text(selection, frame)? == self.logic.text(value, frame)?)
    }

    fn on_click(&self, action: L::Action, frame: Rc<L::Frame>) -> ClickHandler {
        let engine = self.this.clone();
        Box::new(move || {
            if let Some(engine) = engine.upgrade() {
                engine.handle_event(|logic| logic.run(&action, &frame));
            }
        })
    }

    fn on_edit(&self, binding: L::Binding, frame: Rc<L::Frame>) -> TextHandler {
        let engine = self.this.clone();
        Box::new(move |text| {
            if let Some(engine) = engine.upgrade() {
                engine.handle_event(|logic| logic.write_text(&binding, &frame, text));
            }
        })
    }

    /// Writes the item's value as it is when picked, which may differ from when it was shown.
    fn on_select(
        &self,
        selection: L::Binding,
        value: L::Text,
        frame: Rc<L::Frame>,
    ) -> ClickHandler {
        let engine = self.this.clone();
        Box::new(move || {
            if let Some(engine) = engine.upgrade() {
                engine.handle_event(|logic| {
                    let value = logic.text(&value, &frame)?;
                    logic.write_text(&selection, &frame, &value)
                });
            }
        })
    }

    fn on_toggle(&self, binding: L::Binding, frame: Rc<L::Frame>) -> ToggleHandler {
        let engine = self.this.clone();
        Box::new(move |checked| {
            if let Some(engine) = engine.upgrade() {
                engine.handle_event(|logic| logic.write_checked(&binding, &frame, checked));
            }
        })
    }
}

impl<L: Logic, B: Backend> fmt::Debug for Engine<L, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("windows", &self.windows.borrow().len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
