//! Shows an app's windows from templates and keeps them in step with its state.
//!
//! Each dynamic part of a template records which state it reads. After every event the
//! engine updates only the widgets whose state changed, and rebuilds regions in place when
//! the state deciding their contents changed. The [`Logic`] gives everything its values,
//! so the interpreter and compiled apps share this engine.

mod template;
mod windows;

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

pub use template::{
    AppTemplate, Logic, MenuBar, Shown, Template, Templates, Widget, WindowAction, WindowRequest,
    WindowTemplate,
};

use crate::backend::{
    Backend, ClickHandler, DropHandler, MenuHandler, NumberHandler, TextHandler, ToggleHandler,
};
use crate::menu::{Menu, MenuEntry};
use crate::state::StateSet;
use crate::ui::{Node, Position, Ui};
use windows::{MountedWindow, ReusedWindow};

pub type ErrorHandler<E> = Rc<dyn Fn(E)>;

/// A running app: its logic, its windows, and what each widget shows.
pub struct Engine<L: Logic, B: Backend> {
    logic: L,
    ui: Ui<B>,
    /// The app's windows, which code refers to by their position.
    templates: Vec<WindowTemplate<L>>,
    /// What each window shows, by its template's position; `None` while it is closed.
    windows: RefCell<Vec<Option<MountedWindow<L>>>>,
    on_error: ErrorHandler<L::Error>,
    this: Weak<Self>,
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
    /// Shows the windows of `app` that start open, with `make_logic` giving their dynamic
    /// parts values. It receives the UI and the engine-to-be, for logic that wakes the app
    /// later.
    ///
    /// # Errors
    /// When building the first UI fails.
    pub fn start(
        ui: Ui<B>,
        app: AppTemplate<L>,
        on_error: ErrorHandler<L::Error>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
    ) -> Result<Rc<Self>, L::Error> {
        Self::open(ui, app, on_error, make_logic, &[])
    }

    /// Shows `app`'s windows and menus in place of this app's. A window open now stays
    /// open with its new content, one closed now stays closed, and windows this app
    /// didn't have open if they start open; windows it has beyond `app`'s close.
    ///
    /// # Errors
    /// When building the new UI fails. Nothing has changed then, and this app keeps running.
    pub fn reload(
        &self,
        app: AppTemplate<L>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
    ) -> Result<Rc<Self>, L::Error> {
        let reused: Vec<Option<ReusedWindow>> = self
            .windows
            .borrow()
            .iter()
            .map(|mounted| mounted.as_ref().map(ReusedWindow::of))
            .collect();
        let next = Self::open(
            self.ui.clone(),
            app,
            Rc::clone(&self.on_error),
            make_logic,
            &reused,
        )?;

        // The new app has replaced this one's content, so this one must not touch it again.
        self.windows.borrow_mut().clear();
        let kept = next.templates.len();
        for surplus in reused.iter().skip(kept).flatten() {
            self.ui.close(surplus.window());
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

    /// Builds the content of every window to open, reusing `reused` by position, and
    /// shows it all only once everything is built, so a failure leaves the windows as
    /// they were. The menus change with them.
    fn open(
        ui: Ui<B>,
        AppTemplate { windows, menu_bar }: AppTemplate<L>,
        on_error: ErrorHandler<L::Error>,
        make_logic: impl FnOnce(&Ui<B>, Weak<Self>) -> L,
        reused: &[Option<ReusedWindow>],
    ) -> Result<Rc<Self>, L::Error> {
        let engine = Rc::new_cyclic(|this: &Weak<Self>| Self {
            logic: make_logic(&ui, this.clone()),
            ui,
            windows: RefCell::new(windows.iter().map(|_| None).collect()),
            templates: windows,
            on_error,
            this: this.clone(),
        });

        let mut prepared = Vec::new();
        for (index, opens) in engine.windows_to_open(reused).into_iter().enumerate() {
            if !opens {
                continue;
            }
            let reused = reused.get(index).copied().flatten();
            match engine.prepare_window(index, reused) {
                Ok(window) => prepared.push(window),
                Err(error) => {
                    engine.discard(prepared);
                    return Err(error);
                }
            }
        }

        // Menus come before windows show, since a window's menu bar can change its size.
        engine.show_menus(menu_bar);
        for window in prepared {
            engine.show_prepared(window);
        }
        engine.ui.set_close_handler(engine.on_close());
        engine.ui.flush();
        Ok(engine)
    }

    /// Gives the app `menu_bar`'s menus, whose items run their actions as events.
    fn show_menus(&self, menu_bar: MenuBar<L>) {
        let mut actions = Vec::new();
        let menus: Vec<Menu<usize>> = menu_bar
            .menus
            .into_iter()
            .map(|menu| menu.number(&mut actions))
            .collect();
        let on_select = self.on_menu(actions, Rc::new(menu_bar.frame));
        self.ui.set_menu_bar(&menus, on_select);
    }

    /// Runs the numbered item's action, with `frame` as its locals, as one event.
    fn on_menu(&self, actions: Vec<L::Action>, frame: Rc<L::Frame>) -> MenuHandler {
        let engine = self.this.clone();
        Box::new(move |item| send(&engine, |logic| logic.run(&actions[item], &frame)))
    }

    fn mount_list(
        &self,
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
                self.mount(template, frame, parent, offset)
            })
            .collect()
    }

    /// Creates the UI for `template` at position `offset` in `parent`, advancing `offset`
    /// past what it placed there.
    fn mount(
        &self,
        template: TemplateRef<L>,
        frame: &Rc<L::Frame>,
        parent: Node,
        offset: &mut usize,
    ) -> Result<Mounted<L>, L::Error> {
        match template.template() {
            Template::Stack {
                style,
                children,
                scrolls,
            } => {
                let at = next(parent, offset);
                let node = if *scrolls {
                    self.ui.scroll(at, style)
                } else {
                    self.ui.stack(at, style)
                };
                let children = self.mount_list(children, frame, node, &mut 0)?;
                Ok(Mounted::Stack { node, children })
            }
            Template::Region { region, .. } => {
                let children = self.mount_region(region, frame, parent, offset)?;
                Ok(Mounted::Region {
                    template,
                    frame: Rc::clone(frame),
                    children,
                })
            }
            Template::Widget { widget, deps } => {
                let node = self.create_widget(next(parent, offset), widget, frame)?;
                let live = (!deps.is_empty()).then(|| (template, Rc::clone(frame)));
                Ok(Mounted::Widget { node, live })
            }
        }
    }

    fn create_widget(
        &self,
        at: Position,
        widget: &Widget<L>,
        frame: &Rc<L::Frame>,
    ) -> Result<Node, L::Error> {
        Ok(match widget {
            Widget::Label { text, style } => {
                self.ui.label(at, &self.logic.text(text, frame)?, *style)
            }
            Widget::Button {
                title,
                style,
                icon,
                selected,
                action,
            } => {
                let title = self.logic.text(title, frame)?;
                let selected = selected
                    .as_ref()
                    .map(|selected| self.logic.flag(selected, frame))
                    .transpose()?;
                let on_click = self.on_click(action.clone(), Rc::clone(frame));
                self.ui
                    .button(at, &title, (*style, *icon), selected, on_click)
            }
            Widget::TextField {
                value,
                placeholder,
                lines,
                action,
            } => {
                let text = self.logic.read_text(value, frame)?;
                let on_change = self.on_edit(value.clone(), Rc::clone(frame));
                let on_submit = action
                    .as_ref()
                    .map(|action| self.on_click(action.clone(), Rc::clone(frame)));
                self.ui
                    .text_field(at, &text, (placeholder, *lines), on_change, on_submit)
            }
            Widget::Checkbox {
                checked,
                title,
                style,
            } => {
                let is_checked = self.logic.read_checked(checked, frame)?;
                let title = match title {
                    Some(title) => self.logic.text(title, frame)?,
                    None => String::new(),
                };
                let on_toggle = self.on_toggle(checked.clone(), Rc::clone(frame));
                self.ui.checkbox(at, &title, is_checked, *style, on_toggle)
            }
            Widget::Slider { value, min, max } => {
                let current = self.logic.read_number(value, frame)?;
                let on_change = self.on_number(value.clone(), Rc::clone(frame));
                self.ui.slider(at, current, (*min, *max), on_change)
            }
            Widget::Picker { selection, options } => {
                let options = self.logic.texts(options, frame)?;
                let selected = self.logic.read_text(selection, frame)?;
                let on_pick = self.on_edit(selection.clone(), Rc::clone(frame));
                self.ui.picker(at, &options, &selected, on_pick)
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
                    .nav_item(at, &title, *icon, &badge, selected, on_click)
            }
            Widget::Image { path, size } => {
                self.ui.image(at, &self.logic.text(path, frame)?, *size)
            }
            Widget::ContextMenu { entries } => {
                let mut actions = Vec::new();
                let entries = MenuEntry::number_all(entries.clone(), &mut actions);
                let on_select = self.on_menu(actions, Rc::clone(frame));
                self.ui.context_menu(at, &entries, on_select)
            }
            Widget::DropTarget { dropped, action } => {
                let on_drop = self.on_drop(dropped.clone(), action.clone(), Rc::clone(frame));
                self.ui.drop_target(at, on_drop)
            }
            Widget::Progress { value } => {
                let percent = value
                    .as_ref()
                    .map(|value| self.logic.number(value, frame))
                    .transpose()?;
                self.ui.progress(at, percent)
            }
            Widget::Spacer => self.ui.spacer(at),
            Widget::Divider => self.ui.divider(at),
        })
    }

    /// Creates what a region currently shows.
    fn mount_region(
        &self,
        region: &L::Region,
        frame: &Rc<L::Frame>,
        parent: Node,
        offset: &mut usize,
    ) -> Result<Vec<Mounted<L>>, L::Error> {
        let mut children = Vec::new();
        for shown in self.logic.expand(region, frame)? {
            children.extend(self.mount_list(&shown.templates, &shown.frame, parent, offset)?);
        }
        Ok(children)
    }

    /// Brings every window up to date with the state changed by the last event, then
    /// opens and closes the windows it asked for.
    fn refresh(&self) -> Result<(), L::Error> {
        let changed = self.logic.take_changed();
        if !changed.is_empty() {
            let mut windows = self.windows.borrow_mut();
            for mounted in windows.iter_mut().flatten() {
                self.refresh_list(&mut mounted.children, mounted.content, &changed, &mut 0)?;
            }
        }
        self.apply_window_requests(&self.logic.take_window_requests())?;
        self.ui.flush();
        Ok(())
    }

    fn refresh_list(
        &self,
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
                    self.refresh_list(children, *node, changed, &mut 0)?;
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
                        *children = self.mount_region(region, frame, parent, offset)?;
                    }
                    _ => self.refresh_list(children, parent, changed, offset)?,
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
            Widget::Label { text, .. } => {
                self.ui.set_text(node, &self.logic.text(text, frame)?);
            }
            Widget::Button {
                title, selected, ..
            } => {
                self.ui.set_text(node, &self.logic.text(title, frame)?);
                if let Some(selected) = selected {
                    self.ui.set_checked(node, self.logic.flag(selected, frame)?);
                }
            }
            Widget::TextField { value, .. } => {
                self.ui.set_text(node, &self.logic.read_text(value, frame)?);
            }
            Widget::Checkbox { checked, title, .. } => {
                self.ui
                    .set_checked(node, self.logic.read_checked(checked, frame)?);
                if let Some(title) = title {
                    self.ui.set_text(node, &self.logic.text(title, frame)?);
                }
            }
            Widget::Slider { value, .. } => {
                self.ui
                    .set_value(node, self.logic.read_number(value, frame)?);
            }
            Widget::Picker { selection, options } => {
                self.ui
                    .set_options(node, &self.logic.texts(options, frame)?);
                self.ui
                    .set_text(node, &self.logic.read_text(selection, frame)?);
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
            Widget::Image { path, .. } => {
                self.ui.set_image(node, &self.logic.text(path, frame)?);
            }
            Widget::Progress { value } => {
                if let Some(value) = value {
                    self.ui.set_progress(node, self.logic.number(value, frame)?);
                }
            }
            Widget::Spacer
            | Widget::Divider
            | Widget::ContextMenu { .. }
            | Widget::DropTarget { .. } => {}
        }
        Ok(())
    }

    fn badge(&self, badge: Option<&L::Value>, frame: &L::Frame) -> Result<String, L::Error> {
        badge.map_or_else(|| Ok(String::new()), |badge| self.logic.text(badge, frame))
    }

    fn is_selected(
        &self,
        value: &L::Value,
        selection: &L::Binding,
        frame: &L::Frame,
    ) -> Result<bool, L::Error> {
        Ok(self.logic.read_text(selection, frame)? == self.logic.text(value, frame)?)
    }

    fn on_click(&self, action: L::Action, frame: Rc<L::Frame>) -> ClickHandler {
        let engine = self.this.clone();
        Box::new(move || send(&engine, |logic| logic.run(&action, &frame)))
    }

    fn on_edit(&self, binding: L::Binding, frame: Rc<L::Frame>) -> TextHandler {
        let engine = self.this.clone();
        Box::new(move |text| send(&engine, |logic| logic.write_text(&binding, &frame, text)))
    }

    /// Writes the item's value as it is when picked, which may differ from when it was shown.
    fn on_select(
        &self,
        selection: L::Binding,
        value: L::Value,
        frame: Rc<L::Frame>,
    ) -> ClickHandler {
        let engine = self.this.clone();
        Box::new(move || {
            send(&engine, |logic| {
                let value = logic.text(&value, &frame)?;
                logic.write_text(&selection, &frame, &value)
            });
        })
    }

    fn on_number(&self, binding: L::Binding, frame: Rc<L::Frame>) -> NumberHandler {
        let engine = self.this.clone();
        Box::new(move |number| {
            send(&engine, |logic| {
                logic.write_number(&binding, &frame, number)
            });
        })
    }

    /// Writes what was dropped to `dropped`, then runs `action`, as one event.
    fn on_drop(
        &self,
        dropped: L::Binding,
        action: Option<L::Action>,
        frame: Rc<L::Frame>,
    ) -> DropHandler {
        let engine = self.this.clone();
        Box::new(move |items| {
            send(&engine, |logic| {
                logic.write_texts(&dropped, &frame, items)?;
                action
                    .as_ref()
                    .map_or(Ok(()), |action| logic.run(action, &frame))
            });
        })
    }

    fn on_toggle(&self, binding: L::Binding, frame: Rc<L::Frame>) -> ToggleHandler {
        let engine = self.this.clone();
        Box::new(move |checked| {
            send(&engine, |logic| {
                logic.write_checked(&binding, &frame, checked)
            });
        })
    }
}

/// Runs `event` as one event of `engine`, if the app is still running. Handlers hold
/// their engine weakly, since the engine owns the widgets that own them.
fn send<L: Logic, B: Backend>(
    engine: &Weak<Engine<L, B>>,
    event: impl FnOnce(&L) -> Result<(), L::Error>,
) {
    if let Some(engine) = engine.upgrade() {
        engine.handle_event(event);
    }
}

/// The next position in `parent`, advancing `offset` past it.
fn next(parent: Node, offset: &mut usize) -> Position {
    let at = Position {
        parent,
        index: *offset,
    };
    *offset += 1;
    at
}

impl<L: Logic, B: Backend> fmt::Debug for Engine<L, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Engine")
            .field("windows", &self.templates.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
