//! Turns a checked program's windows and views into the runtime's templates, whose dynamic
//! parts point back into the checked code for the interpreter to evaluate.

use std::rc::Rc;

use nifty_check::hir::{
    Binding, Dynamic, Expr, NavItem, Node, Nodes, Program, StateSet, ViewDef, WindowDef,
};
use nifty_runtime::{Template, Templates, ToggleStyle, Widget, WindowTemplate};

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
        backdrop: window.backdrop,
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
        Node::Stack {
            style,
            children,
            scrolls,
        } => Template::Stack {
            style: *style,
            children: nodes(children),
            scrolls: *scrolls,
        },
        Node::If {
            branches,
            otherwise,
            deps,
        } => region(if_region(branches, otherwise), deps),
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
        control => widget_template(control),
    }
}

/// A control's template, with the state what it shows depends on.
fn widget_template(node: &Node) -> Template<Interpreter> {
    match node {
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
            icon,
            selected,
            action,
        } => {
            let mut deps = title.deps.clone();
            if let Some(selected) = selected {
                deps.extend(&selected.deps);
            }
            widget(
                Widget::Button {
                    title: Rc::clone(title),
                    style: *style,
                    icon: *icon,
                    selected: selected.clone(),
                    action: Rc::clone(action),
                },
                deps,
            )
        }
        Node::TextField {
            value,
            placeholder,
            lines,
        } => widget(
            Widget::TextField {
                value: Rc::clone(value),
                placeholder: placeholder.clone(),
                lines: *lines,
            },
            value.deps.clone(),
        ),
        Node::Checkbox {
            checked,
            title,
            style,
        } => checkbox(checked, title.as_ref(), *style),
        Node::Slider { value, min, max } => widget(
            Widget::Slider {
                value: Rc::clone(value),
                min: *min,
                max: *max,
            },
            value.deps.clone(),
        ),
        Node::Picker {
            selection,
            options,
            deps,
        } => widget(
            Widget::Picker {
                selection: Rc::clone(selection),
                options: Rc::clone(options),
            },
            deps.clone(),
        ),
        Node::NavItem(item) => nav_item(item),
        Node::Progress { value } => widget(
            Widget::Progress {
                value: value.clone(),
            },
            value
                .as_ref()
                .map_or_else(StateSet::default, |value| value.deps.clone()),
        ),
        Node::Spacer => widget(Widget::Spacer, StateSet::default()),
        Node::Divider => widget(Widget::Divider, StateSet::default()),
        Node::Stack { .. } | Node::If { .. } | Node::For { .. } | Node::View { .. } => {
            unreachable!("`node` lowers stacks and regions itself")
        }
    }
}

fn if_region(branches: &[(Rc<Expr>, Nodes)], otherwise: &Nodes) -> Region {
    Region::If {
        branches: branches
            .iter()
            .map(|(condition, body)| (Rc::clone(condition), nodes(body)))
            .collect(),
        otherwise: nodes(otherwise),
    }
}

fn checkbox(
    checked: &Rc<Binding>,
    title: Option<&Rc<Dynamic>>,
    style: ToggleStyle,
) -> Template<Interpreter> {
    let mut deps = checked.deps.clone();
    if let Some(title) = title {
        deps.extend(&title.deps);
    }
    widget(
        Widget::Checkbox {
            checked: Rc::clone(checked),
            title: title.cloned(),
            style,
        },
        deps,
    )
}

fn nav_item(item: &NavItem) -> Template<Interpreter> {
    widget(
        Widget::NavItem {
            title: Rc::clone(&item.title),
            value: Rc::clone(&item.value),
            selection: Rc::clone(&item.selection),
            icon: item.icon,
            badge: item.badge.clone(),
        },
        item.deps.clone(),
    )
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
