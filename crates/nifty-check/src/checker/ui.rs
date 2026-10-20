//! Window and view bodies: elements, conditional and repeated UI, and what each dynamic part
//! of the UI reads.

use std::rc::Rc;

use nifty_runtime::{Axis, ButtonStyle, Icon, LabelStyle, Size, StackStyle};
use nifty_syntax::Span;
use nifty_syntax::ast::{self, Arg, ElseBranch, ExprKind as Ast, IfStmt, StmtKind};

use super::args::Args;
use super::consts::{
    ALIGNMENTS, BACKDROPS, BUTTON_STYLES, ICONS, JUSTIFICATIONS, SURFACES, TEXT_ALIGNMENTS,
    TEXT_COLORS, TEXT_STYLES,
};
use super::env::Context;
use super::{Checker, Global};
use crate::elements::{self, Element, ElementKind};
use crate::hir::{
    Binding, Block, Dynamic, NavItem, Node, Nodes, PlaceRoot, StateSet, Type, WindowDef,
};

const DEFAULT_WINDOW_SIZE: Size = Size::new(480.0, 320.0);

impl Checker {
    pub(super) fn window(&mut self, stmt: &ast::Stmt) -> Option<WindowDef> {
        let StmtKind::Expr(ast::Expr {
            kind:
                Ast::Call {
                    callee,
                    args,
                    trailing_block,
                },
            span,
        }) = &stmt.kind
        else {
            self.error(
                "only declarations and `window(…)` can appear at the top level",
                stmt.span,
            );
            return None;
        };
        let window = match &callee.kind {
            Ast::Name(name) => {
                elements::element(name).filter(|element| element.kind == ElementKind::Window)
            }
            _ => None,
        };
        let Some(window) = window else {
            self.error(
                "only declarations and `window(…)` can appear at the top level",
                callee.span,
            );
            return None;
        };
        let mut args = Args::built_in(self, window.name, window.params, args);
        let title = if let Some(title) = args.positional() {
            self.const_string(title)
        } else {
            self.error(
                "`window` needs a title, such as `window(\"My App\")`",
                *span,
            );
            None
        };
        let width = args
            .named("width")
            .and_then(|width| self.const_points(width));
        let height = args
            .named("height")
            .and_then(|height| self.const_points(height));
        let backdrop = args
            .named("backdrop")
            .and_then(|backdrop| self.const_member(backdrop, BACKDROPS))
            .unwrap_or_default();
        args.finish(self);
        let Some(block) = trailing_block else {
            self.error("`window` needs a `{ … }` block with its content", *span);
            return None;
        };

        let nodes = self.ui_nodes(&block.stmts);
        let (style, children) = match nodes.as_slice() {
            [
                Node::Stack {
                    style,
                    children,
                    scrolls: false,
                },
            ] => (*style, Rc::clone(children)),
            _ => (StackStyle::new(Axis::Vertical), nodes.into()),
        };
        Some(WindowDef {
            title: title?,
            size: Size::new(
                width.unwrap_or(DEFAULT_WINDOW_SIZE.width),
                height.unwrap_or(DEFAULT_WINDOW_SIZE.height),
            ),
            backdrop,
            style,
            children,
            locals: self
                .body
                .locals
                .iter()
                .map(|local| local.ty.clone())
                .collect(),
        })
    }

    pub(super) fn ui_block(&mut self, stmts: &[ast::Stmt]) -> Nodes {
        self.ui_nodes(stmts).into()
    }

    fn ui_nodes(&mut self, stmts: &[ast::Stmt]) -> Vec<Node> {
        self.body.push_scope();
        let nodes = stmts.iter().filter_map(|stmt| self.ui_stmt(stmt)).collect();
        self.body.pop_scope();
        nodes
    }

    fn ui_stmt(&mut self, stmt: &ast::Stmt) -> Option<Node> {
        match &stmt.kind {
            StmtKind::Expr(ast::Expr {
                kind:
                    Ast::Call {
                        callee,
                        args,
                        trailing_block,
                    },
                span,
            }) if matches!(callee.kind, Ast::Name(_)) => {
                let Ast::Name(name) = &callee.kind else {
                    return None;
                };
                self.element(name, args, trailing_block.as_ref(), *span)
            }
            StmtKind::If(if_stmt) => Some(self.ui_if(if_stmt, stmt.span)),
            StmtKind::For {
                item,
                iterable,
                body,
            } => {
                let ((local, iterable), deps) =
                    self.dynamic(stmt.span, |checker| checker.loop_head(iterable));
                self.body.push_scope();
                self.body.bind(&item.name, local);
                let body = self.ui_block(&body.stmts);
                self.body.pop_scope();
                Some(Node::For {
                    local,
                    iterable: Rc::new(iterable),
                    body,
                    deps,
                })
            }
            _ => {
                self.error(
                    "only elements, `if` and `for` can appear here; put logic in a function or a button's action",
                    stmt.span,
                );
                None
            }
        }
    }

    fn ui_if(&mut self, if_stmt: &IfStmt, span: Span) -> Node {
        let mut conditions = Vec::new();
        let mut bodies = Vec::new();
        let mut otherwise = None;
        let mut current = if_stmt;
        loop {
            conditions.push(&current.condition);
            bodies.push(&current.then_block);
            match &current.else_branch {
                None => break,
                Some(ElseBranch::Block(block)) => {
                    otherwise = Some(block);
                    break;
                }
                Some(ElseBranch::If(next)) => current = next,
            }
        }
        let (conditions, deps) = self.dynamic(span, |checker| {
            conditions
                .into_iter()
                .map(|condition| checker.expect(condition, &Type::Bool))
                .collect::<Vec<_>>()
        });
        let branches = conditions
            .into_iter()
            .zip(bodies)
            .map(|(condition, body)| (Rc::new(condition), self.ui_block(&body.stmts)))
            .collect();
        let otherwise =
            otherwise.map_or_else(|| Vec::new().into(), |block| self.ui_block(&block.stmts));
        Node::If {
            branches,
            otherwise,
            deps,
        }
    }

    fn element(
        &mut self,
        name: &str,
        args: &[Arg],
        block: Option<&ast::Block>,
        span: Span,
    ) -> Option<Node> {
        let Some(element) = elements::element(name) else {
            let args = Args::new(self, format!("`{name}`"), args);
            return self.view_call(name, args, block, span);
        };
        self.built_in_element(element, args, block, span)
    }

    fn built_in_element(
        &mut self,
        element: &'static Element,
        args: &[Arg],
        block: Option<&ast::Block>,
        span: Span,
    ) -> Option<Node> {
        let name = element.name;
        let mut args = Args::built_in(self, name, element.params, args);
        let node = match element.kind {
            ElementKind::Column | ElementKind::Row | ElementKind::Scroll => {
                let axis = if element.kind == ElementKind::Row {
                    Axis::Horizontal
                } else {
                    Axis::Vertical
                };
                let scrolls = element.kind == ElementKind::Scroll;
                let style = self.stack_style(axis, scrolls, &mut args);
                let children =
                    block.map_or_else(|| Vec::new().into(), |block| self.ui_block(&block.stmts));
                args.finish(self);
                return Some(Node::Stack {
                    style,
                    children,
                    scrolls,
                });
            }
            ElementKind::Label => {
                let text = self.required_text(name, args.positional(), span)?;
                let style = self.label_style(&mut args);
                Node::Label {
                    text: Rc::new(text),
                    style,
                }
            }
            ElementKind::Button => {
                let title = self.required_text(name, args.positional(), span)?;
                let style = args
                    .named("style")
                    .and_then(|style| self.const_member(style, BUTTON_STYLES))
                    .unwrap_or(ButtonStyle::Normal);
                let icon = self.icon(&mut args);
                let Some(block) = block else {
                    args.finish(self);
                    self.error("`button` needs a `{ … }` block with what it does", span);
                    return None;
                };
                let action = self.action(block);
                args.finish(self);
                return Some(Node::Button {
                    title: Rc::new(title),
                    style,
                    icon,
                    action,
                });
            }
            ElementKind::TextField => {
                let value = self.binding(name, args.positional(), &Type::String, span)?;
                let placeholder = args
                    .named("placeholder")
                    .and_then(|placeholder| self.const_string(placeholder))
                    .unwrap_or_default();
                Node::TextField {
                    value: Rc::new(value),
                    placeholder,
                }
            }
            ElementKind::Checkbox => {
                let checked = self.binding(name, args.named("checked"), &Type::Bool, span)?;
                let title = args
                    .named("title")
                    .map(|title| Rc::new(self.text(name, title)));
                Node::Checkbox {
                    checked: Rc::new(checked),
                    title,
                }
            }
            ElementKind::NavItem => self.nav_item(name, &mut args, span)?,
            ElementKind::Progress => Node::Progress {
                value: args.named("value").map(|value| Rc::new(self.number(value))),
            },
            ElementKind::Spacer => Node::Spacer,
            ElementKind::Divider => Node::Divider,
            ElementKind::Window => {
                self.error("a window can only be declared at the top level", span);
                return None;
            }
        };
        args.finish(self);
        if let Some(block) = block {
            self.error(
                format!("`{name}` doesn't take a `{{ … }}` block"),
                block.span,
            );
        }
        Some(node)
    }

    /// A stack's settings; a scroll area always grows, so it isn't asked.
    fn stack_style(&mut self, axis: Axis, scrolls: bool, args: &mut Args) -> StackStyle {
        let defaults = StackStyle::new(axis);
        StackStyle {
            axis,
            spacing: args
                .named("spacing")
                .and_then(|spacing| self.const_points(spacing))
                .unwrap_or(defaults.spacing),
            padding: args
                .named("padding")
                .and_then(|padding| self.const_points(padding))
                .unwrap_or(defaults.padding),
            align: args
                .named("align")
                .and_then(|align| self.const_member(align, ALIGNMENTS))
                .unwrap_or(defaults.align),
            justify: args
                .named("justify")
                .and_then(|justify| self.const_member(justify, JUSTIFICATIONS))
                .unwrap_or(defaults.justify),
            surface: args
                .named("surface")
                .and_then(|surface| self.const_member(surface, SURFACES))
                .unwrap_or(defaults.surface),
            grow: scrolls
                || args
                    .named("grow")
                    .and_then(|grow| self.const_bool(grow))
                    .unwrap_or(defaults.grow),
            width: args
                .named("width")
                .and_then(|width| self.const_points(width))
                .unwrap_or(defaults.width),
            max_width: args
                .named("max_width")
                .and_then(|max_width| self.const_points(max_width))
                .unwrap_or(defaults.max_width),
        }
    }

    fn nav_item(&mut self, name: &str, args: &mut Args, span: Span) -> Option<Node> {
        let title = self.required_text(name, args.positional(), span);
        let value = args.named("value").map(|value| self.text(name, value));
        if value.is_none() {
            self.error(
                format!("`{name}` needs `value:`, what `selection` holds while it's picked"),
                span,
            );
        }
        let selection = self.binding(name, args.named("selection"), &Type::String, span);
        let icon = self.icon(args);
        let badge = args.named("badge").map(|badge| self.text(name, badge));
        let (title, value, selection) = (title?, value?, selection?);
        let mut deps = title.deps.clone();
        deps.extend(&value.deps);
        deps.extend(&selection.deps);
        if let Some(badge) = &badge {
            deps.extend(&badge.deps);
        }
        Some(Node::NavItem(NavItem {
            title: Rc::new(title),
            value: Rc::new(value),
            selection: Rc::new(selection),
            icon,
            badge: badge.map(Rc::new),
            deps,
        }))
    }

    fn icon(&mut self, args: &mut Args) -> Icon {
        args.named("icon")
            .and_then(|icon| self.const_member(icon, ICONS))
            .unwrap_or_default()
    }

    fn label_style(&mut self, args: &mut Args) -> LabelStyle {
        let font = args
            .named("style")
            .and_then(|style| self.const_member(style, TEXT_STYLES))
            .unwrap_or_default();
        let defaults = LabelStyle::new(font);
        LabelStyle {
            font,
            color: args
                .named("color")
                .and_then(|color| self.const_member(color, TEXT_COLORS))
                .unwrap_or(defaults.color),
            align: args
                .named("align")
                .and_then(|align| self.const_member(align, TEXT_ALIGNMENTS))
                .unwrap_or(defaults.align),
            selectable: args
                .named("selectable")
                .and_then(|selectable| self.const_bool(selectable))
                .unwrap_or(defaults.selectable),
        }
    }

    fn view_call(
        &mut self,
        name: &str,
        mut args: Args,
        block: Option<&ast::Block>,
        span: Span,
    ) -> Option<Node> {
        let Some(Global::View(view)) = self.global(name) else {
            args.finish(self);
            let message = match self.global(name) {
                Some(Global::Fn(_)) => format!(
                    "`{name}` is a function; call it from a button's action, or show its result with `label`"
                ),
                _ => format!("unknown element or view `{name}`"),
            };
            self.error(message, span);
            return None;
        };
        let params = self.view_params[view.0].clone();
        let (checked, deps) = self.dynamic(span, |checker| {
            let mut checked = Vec::new();
            for (param, ty) in &params {
                let arg = args.param(param);
                checked.push(checker.required_arg(arg, ty, span, || {
                    format!("`{name}` needs an argument for `{param}`")
                }));
            }
            checked
        });
        args.finish(self);
        if let Some(block) = block {
            self.error(
                format!("`{name}` doesn't take a `{{ … }}` block"),
                block.span,
            );
        }
        Some(Node::View {
            view,
            args: checked.into(),
            deps,
        })
    }

    fn required_text(
        &mut self,
        element: &str,
        expr: Option<&ast::Expr>,
        span: Span,
    ) -> Option<Dynamic> {
        if let Some(expr) = expr {
            return Some(self.text(element, expr));
        }
        self.error(
            format!("`{element}` needs its text, such as `{element}(\"Hello\")`"),
            span,
        );
        None
    }

    /// Text the UI shows; it updates whenever the state it reads changes.
    fn text(&mut self, element: &str, expr: &ast::Expr) -> Dynamic {
        let (expr, deps) =
            self.dynamic(expr.span, |checker| checker.expr(expr, Some(&Type::String)));
        match &expr.ty {
            Type::String | Type::Error => {}
            Type::Int | Type::Float | Type::Bool => self.error(
                format!(
                    "`{element}` shows text; turn this `{}` into text with \"{{…}}\"",
                    self.type_name(&expr.ty)
                ),
                expr.span,
            ),
            other => self.require(&other.clone(), &Type::String, expr.span),
        }
        Dynamic { expr, deps }
    }

    /// An `int` the UI shows; it updates whenever the state it reads changes.
    fn number(&mut self, expr: &ast::Expr) -> Dynamic {
        let (expr, deps) = self.dynamic(expr.span, |checker| checker.expect(expr, &Type::Int));
        Dynamic { expr, deps }
    }

    fn binding(
        &mut self,
        element: &str,
        expr: Option<&ast::Expr>,
        ty: &Type,
        span: Span,
    ) -> Option<Binding> {
        let (place, deps) = self.dynamic(expr.map_or(span, |expr| expr.span), |checker| {
            let place = checker.bound_place(element, expr, ty, span)?;
            if let PlaceRoot::State(state) = place.root {
                checker.effects.reads.insert(state);
            }
            Some(place)
        });
        Some(Binding {
            place: place?,
            deps,
        })
    }

    /// A button's action: statements that may change state.
    fn action(&mut self, block: &ast::Block) -> Block {
        let outer = std::mem::replace(&mut self.body.context, Context::Action);
        let action = self.block(block);
        self.body.context = outer;
        action
    }

    /// Checks UI code and returns the state it reads. Building the UI must not change state,
    /// since it runs again whenever that state changes.
    fn dynamic<T>(&mut self, span: Span, check: impl FnOnce(&mut Self) -> T) -> (T, StateSet) {
        let (result, effects) = self.measure(check);
        let (reads, writes) = self.fn_effects.total(&effects);
        if writes {
            self.error(
                "showing the UI can't change state; do that in a button's action",
                span,
            );
        }
        (result, reads)
    }
}
