//! Menus: the menu bar's menus, their items and submenus, and the items' shortcuts.

use std::collections::HashMap;
use std::mem;

use nifty_runtime::{Menu, MenuEntry, Shortcut};
use nifty_syntax::ast;
use nifty_syntax::{Diagnostic, Span};

use super::Checker;
use super::args::Args;
use super::local_types;
use super::ui::{ElementCall, element_call};
use crate::elements::{ElementKind, MENU, Place};
use crate::hir::{Block, MenuBar, Node};

/// The shortcuts given so far, and where, so each one runs one item.
type Shortcuts = HashMap<Shortcut, Span>;

impl Checker {
    /// The menu bar's menus, written as top-level `menu(…)` calls, in order.
    pub(super) fn menu_bar(&mut self, menus: &[ElementCall]) -> MenuBar {
        let mut shortcuts = Shortcuts::new();
        let menus = menus
            .iter()
            .filter_map(|call| self.menu(call, Some(&mut shortcuts)))
            .collect();
        MenuBar {
            menus,
            locals: local_types(mem::take(&mut self.body.locals)),
        }
    }

    /// A `context_menu`, whose items take no shortcuts: they act on what was clicked, and
    /// the stack a menu is in can show many times.
    pub(super) fn context_menu(
        &mut self,
        name: &str,
        block: Option<&ast::Block>,
        span: Span,
    ) -> Option<Node> {
        let block = self.menu_block(name, block, span)?;
        Some(Node::ContextMenu {
            entries: self.menu_entries(&block.stmts, None),
        })
    }

    /// The entries of a menu's block: items, separators and submenus. Items may take
    /// shortcuts when there are `shortcuts` to add them to.
    fn menu_entries(
        &mut self,
        stmts: &[ast::Stmt],
        mut shortcuts: Option<&mut Shortcuts>,
    ) -> Vec<MenuEntry<Block>> {
        stmts
            .iter()
            .filter_map(|stmt| self.menu_entry(stmt, shortcuts.as_deref_mut()))
            .collect()
    }

    fn menu_entry(
        &mut self,
        stmt: &ast::Stmt,
        shortcuts: Option<&mut Shortcuts>,
    ) -> Option<MenuEntry<Block>> {
        let call = element_call(stmt).filter(|call| call.element.places.contains(&Place::Menu));
        let Some(call) = call else {
            self.error(
                format!("only `menu_item`, `{MENU}` and `divider` can go in a menu"),
                stmt.span,
            );
            return None;
        };
        match call.element.kind {
            ElementKind::Menu => self.menu(&call, shortcuts).map(MenuEntry::Submenu),
            ElementKind::MenuItem => self.menu_item(&call, shortcuts),
            _ => {
                let args = Args::built_in(self, call.element.name, call.element.params, call.args);
                args.finish(self);
                self.no_block(call.element.name, call.block);
                Some(MenuEntry::Separator)
            }
        }
    }

    fn menu(
        &mut self,
        call: &ElementCall,
        shortcuts: Option<&mut Shortcuts>,
    ) -> Option<Menu<Block>> {
        let name = call.element.name;
        let mut args = Args::built_in(self, name, call.element.params, call.args);
        let title = self.fixed_title(name, args.positional(), call.span);
        args.finish(self);
        let block = self.menu_block(name, call.block, call.span)?;
        let entries = self.menu_entries(&block.stmts, shortcuts);
        Some(Menu {
            title: title?,
            entries,
        })
    }

    /// The block holding a menu's items, which every menu needs.
    fn menu_block<'a>(
        &mut self,
        name: &str,
        block: Option<&'a ast::Block>,
        span: Span,
    ) -> Option<&'a ast::Block> {
        if block.is_none() {
            self.error(
                format!("`{name}` needs a `{{ … }}` block with its items"),
                span,
            );
        }
        block
    }

    fn menu_item(
        &mut self,
        call: &ElementCall,
        shortcuts: Option<&mut Shortcuts>,
    ) -> Option<MenuEntry<Block>> {
        let name = call.element.name;
        let mut args = Args::built_in(self, name, call.element.params, call.args);
        let title = self.fixed_title(name, args.positional(), call.span);
        let shortcut = args
            .named("shortcut")
            .and_then(|shortcut| self.shortcut(shortcut, shortcuts));
        args.finish(self);
        let Some(block) = call.block else {
            self.error(
                format!("`{name}` needs a `{{ … }}` block with what it does"),
                call.span,
            );
            return None;
        };
        let action = self.action(block);
        Some(MenuEntry::Item {
            title: title?,
            shortcut,
            action,
        })
    }

    /// A menu's or item's title, which is written literally.
    fn fixed_title(&mut self, name: &str, title: Option<&ast::Expr>, span: Span) -> Option<String> {
        if let Some(title) = title {
            return self.const_string(title);
        }
        self.error(
            format!("`{name}` needs a title, such as `{name}(\"File\")`"),
            span,
        );
        None
    }

    /// A shortcut written literally, which no other item and no system command uses, in
    /// a menu whose items take `shortcuts`.
    fn shortcut(
        &mut self,
        expr: &ast::Expr,
        shortcuts: Option<&mut Shortcuts>,
    ) -> Option<Shortcut> {
        let Some(shortcuts) = shortcuts else {
            self.error(
                format!(
                    "a context menu's items take no shortcut; give it to an item of a top-level `{MENU}(…)`"
                ),
                expr.span,
            );
            return None;
        };
        let text = self.const_string(expr)?;
        let shortcut = match Shortcut::parse(&text) {
            Ok(shortcut) => shortcut,
            Err(error) => {
                self.error(error.to_string(), expr.span);
                return None;
            }
        };
        if let Some(command) = shortcut.taken_by() {
            self.error(
                format!(
                    "`{shortcut}` is the system's shortcut for {}; choose another",
                    command.title()
                ),
                expr.span,
            );
            return None;
        }
        if let Some(&first) = shortcuts.get(&shortcut) {
            self.diagnostics.push(
                Diagnostic::new(
                    format!("another menu item already uses `{shortcut}`"),
                    expr.span,
                )
                .with_note("it's used here", first),
            );
            return None;
        }
        shortcuts.insert(shortcut, expr.span);
        Some(shortcut)
    }
}
