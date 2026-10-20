//! Turns a checked program's windows and views into the runtime's templates, whose dynamic
//! parts point back into the checked code for the interpreter to evaluate.

use std::rc::Rc;

use nifty_check::hir::{Node, Nodes, Program, StateSet, ViewDef, WindowDef};
use nifty_runtime::{Template, Templates, Widget, WindowTemplate};

use crate::logic::{Interpreter, Region, ViewTemplate};
use crate::machine::new_frame;

pub(crate) fn windows(program: &Program) -> Vec<WindowTemplate<Interpreter>> {
    program.windows.iter().map(window).collect()
}

/// Indexed by `ViewId`, so a view call finds its view's templates.
pub(crate) fn views(program: &Program) -> Vec<ViewTemplate> {
    program.views.iter().map(view).collect()
}

fn window(window: &WindowDef) -> WindowTemplate<Interpreter> {
    WindowTemplate {
        title: window.title.clone(),
        size: window.size,
        style: window.style,
        children: nodes(&window.children),
        frame: new_frame(window.locals.len()),
    }
}

fn view(view: &ViewDef) -> ViewTemplate {
    ViewTemplate {
        body: nodes(&view.body),
        locals: view.locals.len(),
    }
}

fn nodes(nodes: &Nodes) -> Templates<Interpreter> {
    nodes.iter().map(node).collect()
}

fn node(node: &Node) -> Template<Interpreter> {
    match node {
        Node::Stack { style, children } => Template::Stack {
            style: *style,
            children: nodes(children),
        },
        Node::If {
            branches,
            otherwise,
            deps,
        } => region(
            Region::If {
                branches: branches
                    .iter()
                    .map(|(condition, body)| (Rc::clone(condition), nodes(body)))
                    .collect(),
                otherwise: nodes(otherwise),
            },
            deps,
        ),
        Node::For {
            local,
            iterable,
            body,
            deps,
        } => region(
            Region::For {
                local: *local,
                iterable: Rc::clone(iterable),
                body: nodes(body),
            },
            deps,
        ),
        Node::View { view, args, deps } => region(
            Region::View {
                view: *view,
                args: Rc::clone(args),
            },
            deps,
        ),
        Node::Label { text, style } => widget(
            Widget::Label {
                text: Rc::clone(text),
                style: *style,
            },
            text.deps.clone(),
        ),
        Node::Button {
            title,
            style,
            action,
        } => widget(
            Widget::Button {
                title: Rc::clone(title),
                style: *style,
                action: Rc::clone(action),
            },
            title.deps.clone(),
        ),
        Node::TextField { value, placeholder } => widget(
            Widget::TextField {
                value: Rc::clone(value),
                placeholder: placeholder.clone(),
            },
            value.deps.clone(),
        ),
        Node::Checkbox { checked, title } => {
            let mut deps = checked.deps.clone();
            if let Some(title) = title {
                deps.extend(&title.deps);
            }
            widget(
                Widget::Checkbox {
                    checked: Rc::clone(checked),
                    title: title.clone(),
                },
                deps,
            )
        }
        Node::Spacer => widget(Widget::Spacer, StateSet::default()),
        Node::Divider => widget(Widget::Divider, StateSet::default()),
    }
}

fn widget(widget: Widget<Interpreter>, deps: StateSet) -> Template<Interpreter> {
    Template::Widget { widget, deps }
}

fn region(region: Region, deps: &StateSet) -> Template<Interpreter> {
    Template::Region {
        region,
        deps: deps.clone(),
    }
}
