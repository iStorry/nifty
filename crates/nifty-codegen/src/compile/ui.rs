//! Windows and views: their templates, and the functions behind each template's dynamic
//! parts, which the runtime calls with the template's frame.
//!
//! A `for` over a place shows each item through an alias, as the interpreter does. Its
//! list positions are worked out when the region is built and kept in extra slots after
//! the frame's own locals, so every function that reads the item finds the same one.

use std::collections::HashMap;
use std::mem::{offset_of, size_of};

use cranelift_codegen::ir::types::{I8, I64};
use cranelift_codegen::ir::{InstBuilder, MemFlags, Value};
use cranelift_module::{FuncId, Module};
use nifty_check::hir::{
    Binding, Block as Stmts, Dynamic, Expr, Iterable, LocalId, Loop, NavItem, Node, Nodes, Place,
    PlaceRoot, PlaceStep, StateSet, Type, ViewId,
};
use nifty_rt::{
    BindingDesc, FrameDesc, RegionDesc, Slice, TemplateDesc, TemplateKind, TypeDesc, ViewDesc,
    WindowDesc,
};
use nifty_runtime::{LabelStyle, StackStyle};

use super::body::{Alias, Body, Index, Operand, Root, Step, element_type};
use super::{List, Result, Shared, put_str};
use crate::CodegenError;
use crate::emit::{Blob, Emitter, Target};

/// The frame a window's or view's templates share: its locals, then the extra slots its
/// `for` regions keep list positions in.
struct Frame<'p> {
    locals: &'p [Type],
    extra: usize,
}

impl Frame<'_> {
    fn add_slot(&mut self) -> usize {
        self.extra += 1;
        self.locals.len() + self.extra - 1
    }

    /// The type of every slot; the extra ones hold list positions.
    fn types(&self) -> Vec<Type> {
        let mut types = self.locals.to_vec();
        types.extend(std::iter::repeat_n(Type::Int, self.extra));
        types
    }
}

/// The aliases templates inside a region can see.
type Aliases = HashMap<LocalId, Alias>;

/// Where a template's fields go: its blob and its offset in it.
struct Slot<'b> {
    blob: &'b mut Blob,
    at: usize,
}

impl Slot<'_> {
    fn function(&mut self, field: usize, function: FuncId) {
        self.blob
            .put_pointer(self.at + field, Target::Function(function));
    }
}

struct Ui<'a, 'p, M: Module> {
    emitter: &'a mut Emitter<M>,
    shared: &'a mut Shared<'p>,
}

pub(super) fn windows<M: Module>(
    emitter: &mut Emitter<M>,
    shared: &mut Shared<'_>,
) -> Result<List> {
    let program = shared.program;
    let mut ui = Ui { emitter, shared };
    let size = size_of::<WindowDesc>();
    let mut blob = Blob::zeroed(program.windows.len() * size);
    for (index, window) in program.windows.iter().enumerate() {
        let mut frame = Frame {
            locals: &window.locals,
            extra: 0,
        };
        let children = ui.templates(&mut frame, &Aliases::new(), &window.children)?;
        let locals = ui.frame_types(&frame)?;
        let at = index * size;
        put_str(
            ui.emitter,
            ui.shared,
            &mut blob,
            at + offset_of!(WindowDesc, title),
            &window.title,
        )?;
        blob.put_f32(at + offset_of!(WindowDesc, width), window.size.width);
        blob.put_f32(at + offset_of!(WindowDesc, height), window.size.height);
        blob.put_u8(
            at + offset_of!(WindowDesc, backdrop),
            window.backdrop.code(),
        );
        put_stack(&mut blob, at + offset_of!(WindowDesc, style), window.style);
        children.put(&mut blob, at + offset_of!(WindowDesc, children));
        locals.put(
            &mut blob,
            at + offset_of!(WindowDesc, frame) + offset_of!(FrameDesc, locals),
        );
    }
    ui.list(blob, program.windows.len())
}

pub(super) fn views<M: Module>(emitter: &mut Emitter<M>, shared: &mut Shared<'_>) -> Result<List> {
    let program = shared.program;
    let mut ui = Ui { emitter, shared };
    let size = size_of::<ViewDesc>();
    let mut blob = Blob::zeroed(program.views.len() * size);
    for (index, view) in program.views.iter().enumerate() {
        let mut frame = Frame {
            locals: &view.locals,
            extra: 0,
        };
        let body = ui.templates(&mut frame, &Aliases::new(), &view.body)?;
        let locals = ui.frame_types(&frame)?;
        let at = index * size;
        body.put(&mut blob, at + offset_of!(ViewDesc, body));
        locals.put(
            &mut blob,
            at + offset_of!(ViewDesc, frame) + offset_of!(FrameDesc, locals),
        );
    }
    ui.list(blob, program.views.len())
}

impl<M: Module> Ui<'_, '_, M> {
    fn list(&mut self, blob: Blob, len: usize) -> Result<List> {
        if len == 0 {
            return Ok(List::EMPTY);
        }
        Ok(List {
            data: Some(self.emitter.data(blob)?),
            len,
        })
    }

    fn frame_types(&mut self, frame: &Frame<'_>) -> Result<List> {
        let types = frame
            .types()
            .iter()
            .map(|ty| {
                self.shared
                    .types
                    .desc(self.emitter.module_mut(), &self.shared.runtime, ty)
            })
            .collect::<Result<Vec<_>>>()?;
        List::emit(
            self.emitter,
            types.len(),
            size_of::<&TypeDesc>(),
            |blob, index, at| {
                blob.put_pointer(at, Target::Data(types[index]));
                Ok(())
            },
        )
    }

    /// Emits `nodes` as an array of templates.
    fn templates(
        &mut self,
        frame: &mut Frame<'_>,
        aliases: &Aliases,
        nodes: &Nodes,
    ) -> Result<List> {
        let size = size_of::<TemplateDesc>();
        let mut blob = Blob::zeroed(nodes.len() * size);
        for (index, node) in nodes.iter().enumerate() {
            let mut slot = Slot {
                blob: &mut blob,
                at: index * size,
            };
            let kind = self.template(frame, aliases, node, &mut slot)?;
            blob.put_u8(index * size + offset_of!(TemplateDesc, kind), kind as u8);
        }
        self.list(blob, nodes.len())
    }

    fn template(
        &mut self,
        frame: &mut Frame<'_>,
        aliases: &Aliases,
        node: &Node,
        slot: &mut Slot<'_>,
    ) -> Result<TemplateKind> {
        Ok(match node {
            Node::Stack { style, children } => {
                put_stack(slot.blob, slot.at + offset_of!(TemplateDesc, stack), *style);
                let children = self.templates(frame, aliases, children)?;
                children.put(slot.blob, slot.at + offset_of!(TemplateDesc, children));
                TemplateKind::Stack
            }
            Node::Label { text, style } => {
                put_label(slot.blob, slot.at + offset_of!(TemplateDesc, label), *style);
                let text_fn = self.text(frame, aliases, text)?;
                slot.function(offset_of!(TemplateDesc, text), text_fn);
                self.deps(slot, &text.deps)?;
                TemplateKind::Label
            }
            Node::Button {
                title,
                style,
                icon,
                action,
            } => {
                slot.blob
                    .put_u8(slot.at + offset_of!(TemplateDesc, button), style.code());
                slot.blob
                    .put_u8(slot.at + offset_of!(TemplateDesc, icon), icon.code());
                let title_fn = self.text(frame, aliases, title)?;
                slot.function(offset_of!(TemplateDesc, text), title_fn);
                let action_fn = self.action(frame, aliases, action)?;
                slot.function(offset_of!(TemplateDesc, action), action_fn);
                self.deps(slot, &title.deps)?;
                TemplateKind::Button
            }
            Node::TextField { value, placeholder } => {
                self.binding(frame, aliases, value, slot)?;
                let at = slot.at + offset_of!(TemplateDesc, placeholder);
                put_str(self.emitter, self.shared, slot.blob, at, placeholder)?;
                self.deps(slot, &value.deps)?;
                TemplateKind::TextField
            }
            Node::Checkbox { checked, title } => {
                self.binding(frame, aliases, checked, slot)?;
                let mut deps = checked.deps.clone();
                if let Some(title) = title {
                    let title_fn = self.text(frame, aliases, title)?;
                    slot.function(offset_of!(TemplateDesc, text), title_fn);
                    deps.extend(&title.deps);
                }
                self.deps(slot, &deps)?;
                TemplateKind::Checkbox
            }
            Node::NavItem(item) => {
                self.nav_item(frame, aliases, item, slot)?;
                TemplateKind::NavItem
            }
            Node::Spacer => TemplateKind::Spacer,
            Node::Divider => TemplateKind::Divider,
            Node::If {
                branches,
                otherwise,
                deps,
            } => {
                self.if_region(frame, aliases, branches, otherwise, slot)?;
                self.deps(slot, deps)?;
                TemplateKind::Region
            }
            Node::For { head, body, deps } => {
                let (expand, list) = match &head.iterable {
                    Iterable::Value(list) => {
                        let body = self.templates(frame, aliases, body)?;
                        (self.for_value(frame, aliases, head, list)?, body)
                    }
                    Iterable::Place(place) => self.for_place(frame, aliases, head, place, body)?,
                    Iterable::Range { start, end } => {
                        let body = self.templates(frame, aliases, body)?;
                        (self.for_range(frame, aliases, head.item, start, end)?, body)
                    }
                };
                self.region(slot, expand, &[list])?;
                self.deps(slot, deps)?;
                TemplateKind::Region
            }
            Node::View { view, args, deps } => {
                let expand = self.view(frame, aliases, *view, args)?;
                self.region(slot, expand, &[])?;
                self.deps(slot, deps)?;
                TemplateKind::Region
            }
        })
    }

    fn nav_item(
        &mut self,
        frame: &mut Frame<'_>,
        aliases: &Aliases,
        item: &NavItem,
        slot: &mut Slot<'_>,
    ) -> Result<()> {
        slot.blob
            .put_u8(slot.at + offset_of!(TemplateDesc, icon), item.icon.code());
        self.binding(frame, aliases, &item.selection, slot)?;
        let title_fn = self.text(frame, aliases, &item.title)?;
        slot.function(offset_of!(TemplateDesc, text), title_fn);
        let value_fn = self.text(frame, aliases, &item.value)?;
        slot.function(offset_of!(TemplateDesc, value), value_fn);
        if let Some(badge) = &item.badge {
            let badge_fn = self.text(frame, aliases, badge)?;
            slot.function(offset_of!(TemplateDesc, badge), badge_fn);
        }
        self.deps(slot, &item.deps)
    }

    /// Shows the first branch whose condition holds, or else `otherwise`, which is the
    /// last list.
    fn if_region(
        &mut self,
        frame: &mut Frame<'_>,
        aliases: &Aliases,
        branches: &[(std::rc::Rc<Expr>, Nodes)],
        otherwise: &Nodes,
        slot: &mut Slot<'_>,
    ) -> Result<()> {
        let mut lists = Vec::new();
        for (_, body) in branches {
            lists.push(self.templates(frame, aliases, body)?);
        }
        lists.push(self.templates(frame, aliases, otherwise)?);
        let expand = self.expansion(frame, aliases, |body, sink| {
            let show = body.shared.runtime.show_same;
            for (index, (condition, _)) in branches.iter().enumerate() {
                let holds = body.expr(condition)?.value;
                let showing = body.b.create_block();
                let next = body.b.create_block();
                body.b.ins().brif(holds, showing, &[], next, &[]);
                body.b.switch_to_block(showing);
                let list = body.u32(index)?;
                body.call(show, &[sink, list]);
                body.succeed_here();
                body.b.switch_to_block(next);
            }
            let otherwise = body.u32(branches.len())?;
            body.call(show, &[sink, otherwise]);
            Ok(())
        })?;
        self.region(slot, expand, &lists)
    }

    /// Shows the body once per item of a copy of the list, with the item in the loop's
    /// item.
    fn for_value(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        head: &Loop,
        list: &Expr,
    ) -> Result<FuncId> {
        let element = element_type(&list.ty)?;
        self.expansion(frame, aliases, |body, sink| {
            let operand = body.expr(list)?;
            let list_owned = body.own(operand, &list.ty)?;
            let len_fn = body.shared.runtime.list_len;
            let len = body.call1(len_fn, &[list_owned]);
            body.count_up(len, |body, index| {
                let item = body.list_item(list_owned, index, list.span)?;
                let item = body.own(Operand::borrowed(item), &element)?;
                let locals = show_round(body, sink)?;
                store_index(body, locals, head.index, index)?;
                let slot = body.b.ins().iadd_imm(locals, slot_offset(head.item.0)?);
                body.store(slot, item, &element)
            })?;
            body.release(list_owned, &list.ty)
        })
    }

    /// Shows the body once per count from `start` up to, but not including, `end`, with the
    /// count in `local`.
    fn for_range(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        local: LocalId,
        start: &Expr,
        end: &Expr,
    ) -> Result<FuncId> {
        self.expansion(frame, aliases, |body, sink| {
            let start = body.expr(start)?.value;
            let end = body.expr(end)?.value;
            body.count(start, end, |body, count| {
                let locals = show_round(body, sink)?;
                store_int(body, locals, local.0, count)
            })
        })
    }

    /// Shows the body once per item of the list at `place`, through an alias whose list
    /// positions are kept in extra frame slots.
    fn for_place(
        &mut self,
        frame: &mut Frame<'_>,
        aliases: &Aliases,
        head: &Loop,
        place: &Place,
        nodes: &Nodes,
    ) -> Result<(FuncId, List)> {
        let mut item = match place.root {
            PlaceRoot::State(state) => Alias {
                root: Root::State(state),
                steps: Vec::new(),
            },
            PlaceRoot::Local(root) => aliases.get(&root).cloned().unwrap_or(Alias {
                root: Root::Local(root.0),
                steps: Vec::new(),
            }),
        };
        let mut positions = Vec::new();
        for step in &place.path {
            item.steps.push(match step {
                PlaceStep::Field(field) => Step::Field(*field),
                PlaceStep::Index(_) => {
                    let slot = frame.add_slot();
                    positions.push(slot);
                    Step::Index(Index::Slot(slot))
                }
            });
        }
        let counter = frame.add_slot();
        item.steps.push(Step::Index(Index::Slot(counter)));
        let mut inner = aliases.clone();
        inner.insert(head.item, item);
        let list = self.templates(frame, &inner, nodes)?;
        let expand = self.expansion(frame, aliases, |body, sink| {
            let path = body.path(place)?;
            let worked_out: Vec<Value> = path
                .steps
                .iter()
                .filter_map(|step| match step {
                    Step::Index(Index::Value(value)) => Some(*value),
                    _ => None,
                })
                .collect();
            let list_value = body.read(&path, place.span)?;
            let len_fn = body.shared.runtime.list_len;
            let len = body.call1(len_fn, &[list_value]);
            body.count_up(len, |body, index| {
                let locals = show_round(body, sink)?;
                for (slot, value) in positions.iter().zip(&worked_out) {
                    store_int(body, locals, *slot, *value)?;
                }
                store_index(body, locals, head.index, index)?;
                store_int(body, locals, counter, index)
            })
        })?;
        Ok((expand, list))
    }

    /// Shows the view's body in a new frame holding its arguments.
    fn view(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        view: ViewId,
        args: &[Expr],
    ) -> Result<FuncId> {
        self.expansion(frame, aliases, |body, sink| {
            let id = body.u32(view.0)?;
            let show = body.shared.runtime.show_view;
            let locals = body.call1(show, &[sink, id]);
            for (index, arg) in args.iter().enumerate() {
                let operand = body.expr(arg)?;
                let value = body.own(operand, &arg.ty)?;
                let slot = body.b.ins().iadd_imm(locals, slot_offset(index)?);
                body.store(slot, value, &arg.ty)?;
            }
            Ok(())
        })
    }

    /// A function the runtime calls with the context, the template's frame, and `extra`
    /// if there is one. It returns `true` where `build` leaves it.
    fn function(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        extra: bool,
        build: impl FnOnce(&mut Body<'_, '_, '_, M>, Value) -> Result<()>,
    ) -> Result<FuncId> {
        let params: &[_] = if extra { &[I64, I64, I64] } else { &[I64, I64] };
        let signature = self.emitter.signature(params, &[I8]);
        let (shared, locals) = (&mut *self.shared, frame.locals);
        self.emitter.function(signature, |b, module| {
            let entry = b
                .current_block()
                .ok_or_else(|| CodegenError::new("a function has no entry block"))?;
            let params = b.block_params(entry).to_vec();
            let mut body = Body::new(b, module, shared, params[0], params[1], locals);
            body.aliases.clone_from(aliases);
            build(&mut body, params[params.len() - 1])?;
            body.finish();
            Ok(())
        })
    }

    /// A region's expansion, which tells the sink it's given what the region shows.
    fn expansion(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        build: impl FnOnce(&mut Body<'_, '_, '_, M>, Value) -> Result<()>,
    ) -> Result<FuncId> {
        self.function(frame, aliases, true, build)
    }

    /// Writes the text's value, owned, to the pointer it's given.
    fn text(&mut self, frame: &Frame<'_>, aliases: &Aliases, text: &Dynamic) -> Result<FuncId> {
        self.function(frame, aliases, true, |body, out| {
            let operand = body.expr(&text.expr)?;
            let value = body.own(operand, &text.expr.ty)?;
            body.b.ins().store(MemFlags::trusted(), value, out, 0);
            Ok(())
        })
    }

    fn action(&mut self, frame: &Frame<'_>, aliases: &Aliases, action: &Stmts) -> Result<FuncId> {
        self.function(frame, aliases, false, |body, _| body.block(action))
    }

    /// The functions that read a binding's place, and write the user's edits to it.
    fn binding(
        &mut self,
        frame: &Frame<'_>,
        aliases: &Aliases,
        binding: &Binding,
        slot: &mut Slot<'_>,
    ) -> Result<()> {
        let place = &binding.place;
        let read = self.function(frame, aliases, true, |body, out| {
            let path = body.path(place)?;
            let value = body.read(&path, place.span)?;
            let value = body.own(Operand::borrowed(value), &place.ty)?;
            body.b.ins().store(MemFlags::trusted(), value, out, 0);
            Ok(())
        })?;
        let write = self.function(frame, aliases, true, |body, value| {
            let path = body.path(place)?;
            // The interpreter writes edits of a place outside state into a copy of the
            // frame, where nothing sees them, so they're dropped here too.
            if !matches!(path.root, Root::State(_)) {
                return body.discard(Operand::owned(value), &place.ty);
            }
            let pointer = body.pointer(&path, place.span)?;
            body.store(pointer, value, &place.ty)
        })?;
        let binding_at = offset_of!(TemplateDesc, binding);
        slot.function(binding_at + offset_of!(BindingDesc, read), read);
        slot.function(binding_at + offset_of!(BindingDesc, write), write);
        Ok(())
    }

    fn region(&mut self, slot: &mut Slot<'_>, expand: FuncId, lists: &[List]) -> Result<()> {
        let region_at = offset_of!(TemplateDesc, region);
        slot.function(region_at + offset_of!(RegionDesc, expand), expand);
        let size = size_of::<Slice<TemplateDesc>>();
        let lists = List::emit(self.emitter, lists.len(), size, |blob, index, at| {
            lists[index].put(blob, at);
            Ok(())
        })?;
        lists.put(
            slot.blob,
            slot.at + region_at + offset_of!(RegionDesc, lists),
        );
        Ok(())
    }

    fn deps(&mut self, slot: &mut Slot<'_>, states: &StateSet) -> Result<()> {
        let ids = states
            .iter()
            .map(|state| {
                u32::try_from(state.0)
                    .map_err(|_| CodegenError::new("the program has too many states"))
            })
            .collect::<Result<Vec<_>>>()?;
        let list = List::emit(
            self.emitter,
            ids.len(),
            size_of::<u32>(),
            |blob, index, at| {
                blob.put_u32(at, ids[index]);
                Ok(())
            },
        )?;
        list.put(slot.blob, slot.at + offset_of!(TemplateDesc, deps));
        Ok(())
    }
}

/// Tells the sink to show a `for` region's body once more, and gives that round's frame.
fn show_round<M: Module>(body: &mut Body<'_, '_, '_, M>, sink: Value) -> Result<Value> {
    let show = body.shared.runtime.show_item;
    let first = body.u32(0)?;
    Ok(body.call1(show, &[sink, first]))
}

/// Puts a round's position in the loop's index, when the loop names one.
fn store_index<M: Module>(
    body: &mut Body<'_, '_, '_, M>,
    locals: Value,
    index: Option<LocalId>,
    position: Value,
) -> Result<()> {
    match index {
        Some(index) => store_int(body, locals, index.0, position),
        None => Ok(()),
    }
}

/// Stores an `int` in a frame's slot.
fn store_int<M: Module>(
    body: &mut Body<'_, '_, '_, M>,
    locals: Value,
    slot: usize,
    value: Value,
) -> Result<()> {
    let at =
        i32::try_from(slot_offset(slot)?).map_err(|_| CodegenError::new("a frame is too large"))?;
    body.b.ins().store(MemFlags::trusted(), value, locals, at);
    Ok(())
}

fn put_stack(blob: &mut Blob, at: usize, style: StackStyle) {
    blob.put_u8(at + offset_of!(StackStyle, axis), style.axis.code());
    blob.put_f32(at + offset_of!(StackStyle, spacing), style.spacing);
    blob.put_f32(at + offset_of!(StackStyle, padding), style.padding);
    blob.put_u8(at + offset_of!(StackStyle, align), style.align.code());
    blob.put_u8(at + offset_of!(StackStyle, justify), style.justify.code());
    blob.put_u8(at + offset_of!(StackStyle, surface), style.surface.code());
    blob.put_u8(at + offset_of!(StackStyle, grow), u8::from(style.grow));
    blob.put_f32(at + offset_of!(StackStyle, width), style.width);
    blob.put_f32(at + offset_of!(StackStyle, max_width), style.max_width);
}

fn put_label(blob: &mut Blob, at: usize, style: LabelStyle) {
    blob.put_u8(at + offset_of!(LabelStyle, font), style.font.code());
    blob.put_u8(at + offset_of!(LabelStyle, color), style.color.code());
    blob.put_u8(at + offset_of!(LabelStyle, align), style.align.code());
    blob.put_u8(
        at + offset_of!(LabelStyle, selectable),
        u8::from(style.selectable),
    );
}

fn slot_offset(local: usize) -> Result<i64> {
    i64::try_from(local)
        .ok()
        .and_then(|local| local.checked_mul(8))
        .ok_or_else(|| CodegenError::new("a frame is too large"))
}
