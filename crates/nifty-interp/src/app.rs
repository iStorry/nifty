//! Mounts a checked program's windows on the runtime and keeps them in step with state.
//!
//! The checker recorded which state each dynamic part of the UI reads. After every event the
//! app updates only the widgets whose state changed, and rebuilds `if`, `for` and view
//! regions in place when the state deciding their contents changed.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use nifty_check::hir::{
    Binding, Block, Dynamic, Iterable, Node, Nodes, Program, StateSet, WindowDef,
};
use nifty_runtime::{Backend, ClickHandler, Size, TextHandler, ToggleHandler, Ui, WindowId};

use crate::download::Downloads;
use crate::machine::{Frame, Machine, Result, RuntimeError, Slot, list_len, new_frame};
use crate::value::Value;

pub type ErrorHandler = Rc<dyn Fn(RuntimeError)>;

/// A running app: its state, its windows, and what each widget shows.
pub struct App<B: Backend> {
    machine: Machine,
    ui: Ui<B>,
    downloads: Downloads<B::Waker>,
    windows: RefCell<Vec<MountedWindow>>,
    on_error: ErrorHandler,
    this: Weak<Self>,
}

impl<B: Backend> std::fmt::Debug for App<B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("App").finish_non_exhaustive()
    }
}

struct MountedWindow {
    window: WindowId,
    content: nifty_runtime::Node,
    children: Vec<Mounted>,
}

/// A window of the version being replaced, and the size its program asked for.
#[derive(Clone, Copy)]
struct ReusedWindow {
    window: WindowId,
    declared_size: Size,
}

/// A window whose new content is built but not yet showing.
struct PreparedWindow {
    mounted: MountedWindow,
    reused: Option<ReusedWindow>,
}

/// A UI node and the list holding it, so mounted UI can re-read its definition later.
#[derive(Clone)]
struct Template {
    nodes: Nodes,
    index: usize,
}

impl Template {
    fn node(&self) -> &Node {
        &self.nodes[self.index]
    }
}

/// Locals visible to a piece of UI: loop variables and view parameters.
type SharedFrame = Rc<Frame>;

enum Mounted {
    /// A widget; `live` is kept when what it shows can change.
    Widget {
        node: nifty_runtime::Node,
        live: Option<(Template, SharedFrame)>,
    },
    Stack {
        node: nifty_runtime::Node,
        children: Vec<Mounted>,
    },
    /// The output of an `if`, `for` or view, placed directly among its parent's children.
    Region {
        template: Template,
        frame: SharedFrame,
        children: Vec<Mounted>,
    },
}

impl<B: Backend> App<B> {
    /// Sets up state and shows every window.
    ///
    /// # Errors
    /// When evaluating a starting value or building the first UI fails.
    ///
    /// # Panics
    /// When `program` has errors; run only programs the checker accepted.
    pub fn start(program: Program, ui: Ui<B>, on_error: ErrorHandler) -> Result<Rc<Self>> {
        let machine = Machine::new(Rc::new(program), None)?;
        Self::open(machine, ui, on_error, &[])
    }

    /// Starts `program` in this app's windows, in place of this app. Each state it still
    /// declares with the same name and type keeps its value; the others start over.
    ///
    /// # Errors
    /// When evaluating a starting value or building the new UI fails. Nothing has changed
    /// then, and this app keeps running.
    ///
    /// # Panics
    /// When `program` has errors; run only programs the checker accepted.
    pub fn reload(&self, program: Program) -> Result<Rc<Self>> {
        let machine = Machine::new(Rc::new(program), Some(&self.machine))?;
        let reused: Vec<ReusedWindow> = self
            .windows
            .borrow()
            .iter()
            .zip(&self.machine.program().windows)
            .map(|(mounted, definition)| ReusedWindow {
                window: mounted.window,
                declared_size: definition.size,
            })
            .collect();
        let next = Self::open(machine, self.ui.clone(), Rc::clone(&self.on_error), &reused)?;

        // The new app has replaced this one's content, so this one must not touch it again.
        self.windows.borrow_mut().clear();
        let kept = next.windows.borrow().len();
        for surplus in reused.iter().skip(kept) {
            self.ui.close(surplus.window);
        }
        Ok(next)
    }

    /// Builds every window's content, reusing `reused` in order, and shows it all only
    /// once everything is built, so a failure leaves the windows as they were.
    fn open(
        machine: Machine,
        ui: Ui<B>,
        on_error: ErrorHandler,
        reused: &[ReusedWindow],
    ) -> Result<Rc<Self>> {
        let program = Rc::clone(machine.program());
        let app = Rc::new_cyclic(|this: &Weak<Self>| Self {
            machine,
            downloads: Downloads::new(ui.waker(Box::new({
                let this = this.clone();
                move || {
                    if let Some(app) = this.upgrade() {
                        app.receive_downloads();
                    }
                }
            }))),
            ui,
            windows: RefCell::new(Vec::new()),
            on_error,
            this: this.clone(),
        });

        let mut prepared = Vec::with_capacity(program.windows.len());
        for (index, definition) in program.windows.iter().enumerate() {
            let reused = reused.get(index).copied();
            let window = match reused {
                Some(reused) => reused.window,
                None => app.ui.create_window(&definition.title, definition.size),
            };
            match app.mount_window(window, definition) {
                Ok(mounted) => prepared.push(PreparedWindow { mounted, reused }),
                Err(error) => {
                    if reused.is_none() {
                        app.ui.close(window);
                    }
                    app.discard(prepared);
                    return Err(error);
                }
            }
        }

        for (PreparedWindow { mounted, reused }, definition) in
            prepared.into_iter().zip(&program.windows)
        {
            app.ui.set_content(mounted.window, mounted.content);
            match reused {
                Some(reused) => {
                    app.ui.set_title(mounted.window, &definition.title);
                    if reused.declared_size != definition.size {
                        app.ui.set_size(mounted.window, definition.size);
                    }
                }
                None => app.ui.show(mounted.window),
            }
            app.windows.borrow_mut().push(mounted);
        }
        app.ui.flush();
        Ok(app)
    }

    /// Builds a window's content without placing it in the window yet.
    fn mount_window(&self, window: WindowId, definition: &WindowDef) -> Result<MountedWindow> {
        let Node::Stack { style, children } = &definition.content else {
            panic!("the checker always gives a window a stack as its content");
        };
        let content = self.ui.stack(window, style);
        let frame = Rc::new(new_frame(definition.locals));
        match self.mount_list(window, children, &frame, content, &mut 0) {
            Ok(children) => Ok(MountedWindow {
                window,
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
    fn discard(&self, prepared: Vec<PreparedWindow>) {
        for PreparedWindow { mounted, reused } in prepared {
            self.ui.remove(mounted.content);
            if reused.is_none() {
                self.ui.close(mounted.window);
            }
        }
    }

    /// Runs the event loop until the app quits.
    pub fn run(&self) {
        self.ui.run();
    }

    #[cfg(test)]
    pub(crate) fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Waits until every running download has sent its last report, without applying them.
    #[cfg(test)]
    pub(crate) fn wait_for_downloads(&self) {
        self.downloads.wait();
    }

    fn mount_list(
        &self,
        window: WindowId,
        nodes: &Nodes,
        frame: &SharedFrame,
        parent: nifty_runtime::Node,
        offset: &mut usize,
    ) -> Result<Vec<Mounted>> {
        (0..nodes.len())
            .map(|index| {
                let template = Template {
                    nodes: Rc::clone(nodes),
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
        template: Template,
        frame: &SharedFrame,
        parent: nifty_runtime::Node,
        offset: &mut usize,
    ) -> Result<Mounted> {
        let node = match template.node() {
            Node::Stack { style, children } => {
                let node = self.ui.stack(window, style);
                self.place(parent, offset, node);
                let children = self.mount_list(window, children, frame, node, &mut 0)?;
                return Ok(Mounted::Stack { node, children });
            }
            Node::If { .. } | Node::For { .. } | Node::View { .. } => {
                let children = self.mount_region(window, &template, frame, parent, offset)?;
                return Ok(Mounted::Region {
                    template,
                    frame: Rc::clone(frame),
                    children,
                });
            }
            Node::Label { text, style } => self.ui.label(window, &self.text(text, frame)?, *style),
            Node::Button {
                title,
                style,
                action,
            } => {
                let on_click = self.on_click(Rc::clone(action), Rc::clone(frame));
                self.ui
                    .button(window, &self.text(title, frame)?, *style, on_click)
            }
            Node::TextField { value, placeholder } => {
                let text = self.binding_value(value, frame)?;
                let on_change = self.on_edit(template.clone(), Rc::clone(frame));
                self.ui
                    .text_field(window, text.as_str(), placeholder, on_change)
            }
            Node::Checkbox { checked, title } => {
                let is_checked = self.binding_value(checked, frame)?.as_bool();
                let title = match title {
                    Some(title) => self.text(title, frame)?,
                    None => String::new(),
                };
                let on_toggle = self.on_toggle(template.clone(), Rc::clone(frame));
                self.ui.checkbox(window, &title, is_checked, on_toggle)
            }
            Node::Spacer => self.ui.spacer(window),
            Node::Divider => self.ui.divider(window),
        };
        self.place(parent, offset, node);
        let live = (!widget_deps(template.node()).is_empty()).then(|| (template, Rc::clone(frame)));
        Ok(Mounted::Widget { node, live })
    }

    fn place(&self, parent: nifty_runtime::Node, offset: &mut usize, node: nifty_runtime::Node) {
        self.ui.insert(parent, *offset, node);
        *offset += 1;
    }

    /// Creates what an `if`, `for` or view currently shows.
    fn mount_region(
        &self,
        window: WindowId,
        template: &Template,
        frame: &SharedFrame,
        parent: nifty_runtime::Node,
        offset: &mut usize,
    ) -> Result<Vec<Mounted>> {
        let mut scratch = Frame::clone(frame);
        match template.node() {
            Node::If {
                branches,
                otherwise,
                ..
            } => {
                let mut shown = otherwise;
                for (condition, body) in branches {
                    if self.machine.eval(condition, &mut scratch)?.as_bool() {
                        shown = body;
                        break;
                    }
                }
                self.mount_list(window, shown, frame, parent, offset)
            }
            Node::For {
                local,
                iterable,
                body,
                ..
            } => {
                let items: Vec<Slot> = match iterable {
                    Iterable::Place(place) => {
                        let location = self.machine.locate(place, &mut scratch)?;
                        let len = list_len(&self.machine.read(&location, &scratch, place.span)?);
                        (0..len)
                            .map(|index| Slot::Alias(location.item(index)))
                            .collect()
                    }
                    Iterable::Value(list) => match self.machine.eval(list, &mut scratch)? {
                        Value::List(items) => items.iter().cloned().map(Slot::Value).collect(),
                        other => {
                            panic!("the checker only allows looping over lists, not {other:?}")
                        }
                    },
                };
                let mut children = Vec::new();
                for item in items {
                    let mut item_frame = Frame::clone(frame);
                    item_frame[local.0] = item;
                    let item_frame = Rc::new(item_frame);
                    children.extend(self.mount_list(window, body, &item_frame, parent, offset)?);
                }
                Ok(children)
            }
            Node::View { view, args, .. } => {
                let program = Rc::clone(self.machine.program());
                let definition = &program.views[view.0];
                let mut view_frame = new_frame(definition.locals);
                for (slot, arg) in view_frame.iter_mut().zip(args) {
                    *slot = Slot::Value(self.machine.eval(arg, &mut scratch)?);
                }
                self.mount_list(
                    window,
                    &definition.body,
                    &Rc::new(view_frame),
                    parent,
                    offset,
                )
            }
            other => panic!("{other:?} is not a region"),
        }
    }

    /// Brings every window up to date with the state changed by the last event.
    fn refresh(&self) -> Result<()> {
        let changed = self.machine.take_changed();
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
        items: &mut [Mounted],
        parent: nifty_runtime::Node,
        changed: &StateSet,
        offset: &mut usize,
    ) -> Result<()> {
        for item in items {
            match item {
                Mounted::Widget { node, live } => {
                    if let Some((template, frame)) = live
                        && widget_deps(template.node()).intersects(changed)
                    {
                        self.update_widget(*node, template.node(), frame)?;
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
                } => {
                    if region_deps(template.node()).intersects(changed) {
                        for child in children.drain(..) {
                            self.unmount(child);
                        }
                        *children = self.mount_region(window, template, frame, parent, offset)?;
                    } else {
                        self.refresh_list(window, children, parent, changed, offset)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn unmount(&self, mounted: Mounted) {
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
        node: nifty_runtime::Node,
        template: &Node,
        frame: &Frame,
    ) -> Result<()> {
        match template {
            Node::Label { text, .. } => self.ui.set_text(node, &self.text(text, frame)?),
            Node::Button { title, .. } => self.ui.set_text(node, &self.text(title, frame)?),
            Node::TextField { value, .. } => {
                self.ui
                    .set_text(node, self.binding_value(value, frame)?.as_str());
            }
            Node::Checkbox { checked, title } => {
                self.ui
                    .set_checked(node, self.binding_value(checked, frame)?.as_bool());
                if let Some(title) = title {
                    self.ui.set_text(node, &self.text(title, frame)?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn text(&self, text: &Dynamic, frame: &Frame) -> Result<String> {
        let value = self.machine.eval(&text.expr, &mut frame.clone())?;
        Ok(value.as_str().to_owned())
    }

    fn binding_value(&self, binding: &Binding, frame: &Frame) -> Result<Value> {
        let mut scratch = frame.clone();
        let location = self.machine.locate(&binding.place, &mut scratch)?;
        self.machine.read(&location, &scratch, binding.place.span)
    }

    fn on_click(&self, action: Block, frame: SharedFrame) -> ClickHandler {
        let app = self.this.clone();
        Box::new(move || {
            if let Some(app) = app.upgrade() {
                app.handle_event(|machine| {
                    machine.exec_block(&action, &mut Frame::clone(&frame))?;
                    Ok(())
                });
            }
        })
    }

    fn on_edit(&self, template: Template, frame: SharedFrame) -> TextHandler {
        let app = self.this.clone();
        Box::new(move |text| {
            let Node::TextField { value, .. } = template.node() else {
                panic!("text edits only come from text fields");
            };
            if let Some(app) = app.upgrade() {
                app.handle_event(|machine| {
                    write_binding(machine, value, &frame, Value::String(Rc::from(text)))
                });
            }
        })
    }

    fn on_toggle(&self, template: Template, frame: SharedFrame) -> ToggleHandler {
        let app = self.this.clone();
        Box::new(move |checked| {
            let Node::Checkbox {
                checked: binding, ..
            } = template.node()
            else {
                panic!("toggles only come from checkboxes");
            };
            if let Some(app) = app.upgrade() {
                app.handle_event(|machine| {
                    write_binding(machine, binding, &frame, Value::Bool(checked))
                });
            }
        })
    }

    /// Runs one event, starts the downloads it asked for, then updates the UI; errors go to
    /// the app's error handler.
    fn handle_event(&self, event: impl FnOnce(&Machine) -> Result<()>) {
        let result = event(&self.machine);
        for request in self.machine.take_downloads() {
            self.downloads.start(request);
        }
        if let Err(error) = result.and_then(|()| self.refresh()) {
            (self.on_error)(error);
        }
    }

    fn receive_downloads(&self) {
        self.handle_event(|machine| {
            self.downloads
                .receive(|targets, report| machine.report_download(targets, report))
        });
    }
}

fn write_binding(machine: &Machine, binding: &Binding, frame: &Frame, value: Value) -> Result<()> {
    let mut scratch = frame.clone();
    let location = machine.locate(&binding.place, &mut scratch)?;
    machine.write(&location, &mut scratch, value, binding.place.span)
}

/// The state a widget shows.
fn widget_deps(node: &Node) -> StateSet {
    match node {
        Node::Label { text, .. } => text.deps.clone(),
        Node::Button { title, .. } => title.deps.clone(),
        Node::TextField { value, .. } => value.deps.clone(),
        Node::Checkbox { checked, title } => {
            let mut deps = checked.deps.clone();
            if let Some(title) = title {
                deps.extend(&title.deps);
            }
            deps
        }
        _ => StateSet::default(),
    }
}

/// The state deciding what a region shows.
fn region_deps(node: &Node) -> &StateSet {
    match node {
        Node::If { deps, .. } | Node::For { deps, .. } | Node::View { deps, .. } => deps,
        other => panic!("{other:?} is not a region"),
    }
}
