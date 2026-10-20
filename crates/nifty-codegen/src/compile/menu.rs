//! The menu bar: its menus as data the runtime reads, and their items' compiled actions,
//! which share one frame of locals.

use std::mem::{offset_of, size_of};

use cranelift_module::Module;
use nifty_check::hir::{Block as Stmts, Menu, MenuEntry};
use nifty_rt::{MenuDesc, MenuEntryDesc, MenuEntryKind, ShortcutDesc};
use nifty_runtime::Shortcut;

use super::ui::{Aliases, Frame, Ui};
use super::{List, Result, Shared, put_str};
use crate::emit::{Blob, Emitter, Target};

/// The menus, and the types of the locals their actions share.
pub(super) fn menu_bar<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
) -> Result<(List, List)> {
    let program = shared.program;
    let mut ui = Ui { emitter, shared };
    let frame = Frame::new(&program.menu_bar.locals);
    let menus = ui.menus(&frame, &program.menu_bar.menus)?;
    let locals = ui.frame_types(&frame)?;
    Ok((menus, locals))
}

impl<M: Module> Ui<'_, '_, M> {
    fn menus(&mut self, frame: &Frame<'_>, menus: &[Menu<Stmts>]) -> Result<List> {
        let size = size_of::<MenuDesc>();
        let mut blob = Blob::zeroed(menus.len() * size);
        for (index, menu) in menus.iter().enumerate() {
            let at = index * size;
            let title = at + offset_of!(MenuDesc, title);
            put_str(self.emitter, self.shared, &mut blob, title, &menu.title)?;
            let entries = self.menu_entries(frame, &menu.entries)?;
            entries.put(&mut blob, at + offset_of!(MenuDesc, entries));
        }
        self.list(blob, menus.len())
    }

    fn menu_entries(&mut self, frame: &Frame<'_>, entries: &[MenuEntry<Stmts>]) -> Result<List> {
        let size = size_of::<MenuEntryDesc>();
        let mut blob = Blob::zeroed(entries.len() * size);
        for (index, entry) in entries.iter().enumerate() {
            let at = index * size;
            let title_at = at + offset_of!(MenuEntryDesc, title);
            let kind = match entry {
                MenuEntry::Item {
                    title,
                    shortcut,
                    action,
                } => {
                    put_str(self.emitter, self.shared, &mut blob, title_at, title)?;
                    if let Some(shortcut) = shortcut {
                        put_shortcut(
                            &mut blob,
                            at + offset_of!(MenuEntryDesc, shortcut),
                            *shortcut,
                        );
                    }
                    let action = self.action(frame, &Aliases::new(), action)?;
                    blob.put_pointer(
                        at + offset_of!(MenuEntryDesc, action),
                        Target::Function(action),
                    );
                    MenuEntryKind::Item
                }
                MenuEntry::Separator => MenuEntryKind::Separator,
                MenuEntry::Submenu(submenu) => {
                    put_str(
                        self.emitter,
                        self.shared,
                        &mut blob,
                        title_at,
                        &submenu.title,
                    )?;
                    let entries = self.menu_entries(frame, &submenu.entries)?;
                    entries.put(&mut blob, at + offset_of!(MenuEntryDesc, entries));
                    MenuEntryKind::Submenu
                }
            };
            blob.put_u8(at + offset_of!(MenuEntryDesc, kind), kind as u8);
        }
        self.list(blob, entries.len())
    }
}

fn put_shortcut(blob: &mut Blob, at: usize, shortcut: Shortcut) {
    blob.put_u8(at + offset_of!(ShortcutDesc, present), 1);
    let at = at + offset_of!(ShortcutDesc, shortcut);
    blob.put_u8(at + offset_of!(Shortcut, key), shortcut.key.code());
    blob.put_u8(at + offset_of!(Shortcut, shift), u8::from(shortcut.shift));
    blob.put_u8(at + offset_of!(Shortcut, alt), u8::from(shortcut.alt));
}
