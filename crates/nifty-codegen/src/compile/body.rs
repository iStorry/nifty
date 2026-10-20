//! Compiling one function body: statements, expressions and places, with the reference
//! counting that keeps values' sharing invisible.
//!
//! An expression gives an [`Operand`] that is either owned, which its user must release
//! or hand on, or borrowed from wherever it lives. A borrowed operand is only kept while
//! nothing that could change state runs, so what it borrows from stays alive.
//!
//! A failing runtime call returns `false` through the function's failure block. A failure
//! ends the app, so values the function holds then aren't released.

use std::collections::HashMap;
use std::mem::{offset_of, size_of};

use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::types::{F64, I8, I32, I64};
use cranelift_codegen::ir::{
    Block, InstBuilder, MemFlags, StackSlot, StackSlotData, StackSlotKind, Value,
};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataId, FuncId, Module};
use nifty_check::hir::{
    BinaryOp, Block as Stmts, Dialog, Expr, ExprKind, Iterable, LocalId, Loop, Place, PlaceRoot,
    PlaceStep, Segment, StateId, Stmt, Type, WindowAction, WindowId,
};
use nifty_rt::{PathStep, PlaceDesc, StepKind};
use nifty_syntax::Span;

mod download;

use super::Shared;
use super::types::is_object;
use crate::CodegenError;

/// Every slot is 8 bytes.
const SLOT: i64 = 8;

pub(super) type Result<T> = std::result::Result<T, CodegenError>;

/// A value an expression gave, and whether the code holding it owns it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Operand {
    pub value: Value,
    pub owned: bool,
}

impl Operand {
    pub fn owned(value: Value) -> Self {
        Self { value, owned: true }
    }

    pub fn borrowed(value: Value) -> Self {
        Self {
            value,
            owned: false,
        }
    }
}

/// Where a `for` variable over a place finds its item: a root, then steps whose list
/// positions were worked out when the loop began.
#[derive(Clone, Debug)]
pub(super) struct Alias {
    pub root: Root,
    pub steps: Vec<Step>,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Root {
    State(StateId),
    /// A local's slot in the frame.
    Local(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Step {
    Field(usize),
    Index(Index),
}

/// Where a list position is kept.
#[derive(Clone, Copy, Debug)]
pub(super) enum Index {
    /// Worked out in this function.
    Value(Value),
    /// A loop counter in this function.
    Var(Variable),
    /// A frame slot, for UI whose parts are separate functions.
    Slot(usize),
}

/// How `return` leaves the function being compiled, if it may.
#[derive(Clone, Copy, Debug)]
pub(super) struct Returns {
    /// Where the result goes.
    pub out: Value,
}

pub(super) struct Body<'a, 'b, 'p, M: Module> {
    pub b: &'a mut FunctionBuilder<'b>,
    pub module: &'a mut M,
    pub shared: &'a mut Shared<'p>,
    pub context: Value,
    /// The locals' slots.
    pub frame: Value,
    /// The type of each of the frame's locals.
    pub locals: &'a [Type],
    pub aliases: HashMap<LocalId, Alias>,
    pub returns: Option<Returns>,
    fail: Option<Block>,
    /// Where calls write their results, read straight after each call.
    scratch: Option<StackSlot>,
    /// Owned values held across statements, such as the list a loop walks, released if
    /// the function returns from inside.
    temporaries: Vec<(Value, Type)>,
}

impl<'a, 'b, 'p, M: Module> Body<'a, 'b, 'p, M> {
    pub fn new(
        b: &'a mut FunctionBuilder<'b>,
        module: &'a mut M,
        shared: &'a mut Shared<'p>,
        context: Value,
        frame: Value,
        locals: &'a [Type],
    ) -> Self {
        Self {
            b,
            module,
            shared,
            context,
            frame,
            locals,
            aliases: HashMap::new(),
            returns: None,
            fail: None,
            scratch: None,
            temporaries: Vec::new(),
        }
    }

    /// Returns `true` from where the body ended, then emits the failure block.
    pub fn finish(mut self) {
        self.succeed();
        if let Some(fail) = self.fail {
            self.b.switch_to_block(fail);
            let failed = self.b.ins().iconst(I8, 0);
            self.b.ins().return_(&[failed]);
        }
    }

    /// Returns `true` here, and carries on in a new block.
    pub fn succeed_here(&mut self) {
        self.succeed();
        self.continue_after_terminator();
    }

    fn succeed(&mut self) {
        let succeeded = self.b.ins().iconst(I8, 1);
        self.b.ins().return_(&[succeeded]);
    }

    /// Continues in a new block, after a terminator.
    fn continue_after_terminator(&mut self) {
        let next = self.b.create_block();
        self.b.switch_to_block(next);
    }

    // Runtime calls.

    pub fn call(&mut self, function: FuncId, args: &[Value]) -> Vec<Value> {
        let callee = self.module.declare_func_in_func(function, self.b.func);
        let call = self.b.ins().call(callee, args);
        self.b.inst_results(call).to_vec()
    }

    pub fn call1(&mut self, function: FuncId, args: &[Value]) -> Value {
        self.call(function, args)[0]
    }

    /// Makes a call that returns whether it succeeded, going to the failure block if not.
    pub fn call_checked(&mut self, function: FuncId, args: &[Value]) {
        let ok = self.call1(function, args);
        self.check(ok);
    }

    fn check(&mut self, ok: Value) {
        let fail = self.fail_block();
        let next = self.b.create_block();
        self.b.ins().brif(ok, next, &[], fail, &[]);
        self.b.switch_to_block(next);
    }

    fn fail_block(&mut self) -> Block {
        *self.fail.get_or_insert_with(|| self.b.create_block())
    }

    /// Fails with `failure` at `span` when `condition` holds.
    fn fail_if(&mut self, condition: Value, failure: FuncId, span: Span) -> Result<()> {
        let failing = self.b.create_block();
        let next = self.b.create_block();
        self.b.ins().brif(condition, failing, &[], next, &[]);
        self.b.switch_to_block(failing);
        let context = self.context;
        let span = self.span(span)?;
        self.call(failure, &[context, span]);
        let fail = self.fail_block();
        self.b.ins().jump(fail, &[]);
        self.b.switch_to_block(next);
        Ok(())
    }

    pub fn span(&mut self, span: Span) -> Result<Value> {
        let id = self.shared.spans.id(span)?;
        Ok(self.b.ins().iconst(I32, i64::from(id)))
    }

    pub fn u32(&mut self, value: usize) -> Result<Value> {
        let value = u32::try_from(value)
            .map_err(|_| CodegenError::new(format!("{value} is too large for an id")))?;
        Ok(self.b.ins().iconst(I32, i64::from(value)))
    }

    fn usize(&mut self, value: usize) -> Result<Value> {
        let value =
            i64::try_from(value).map_err(|_| CodegenError::new(format!("{value} is too large")))?;
        Ok(self.b.ins().iconst(I64, value))
    }

    pub fn address(&mut self, data: DataId) -> Value {
        let global = self.module.declare_data_in_func(data, self.b.func);
        self.b.ins().global_value(I64, global)
    }

    pub fn desc(&mut self, ty: &Type) -> Result<Value> {
        let shared = &mut *self.shared;
        let id = shared.types.desc(self.module, &shared.runtime, ty)?;
        Ok(self.address(id))
    }

    /// A stack slot of `size` bytes, aligned for slots, and its address.
    fn stack(&mut self, size: usize) -> Result<Value> {
        let size = u32::try_from(size)
            .map_err(|_| CodegenError::new("a function needs too much stack"))?;
        let slot = self.b.create_sized_stack_slot(StackSlotData::new(
            StackSlotKind::ExplicitSlot,
            size,
            3,
        ));
        Ok(self.b.ins().stack_addr(I64, slot, 0))
    }

    /// The address of the slot calls write their results to.
    fn scratch(&mut self) -> Value {
        let slot = *self.scratch.get_or_insert_with(|| {
            self.b
                .create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 8, 3))
        });
        self.b.ins().stack_addr(I64, slot, 0)
    }

    /// `values`, stored in order in a stack array, and its address.
    fn array(&mut self, values: &[Value]) -> Result<Value> {
        if values.is_empty() {
            return Ok(self.b.ins().iconst(I64, 0));
        }
        let array = self.stack(values.len() * 8)?;
        for (index, value) in values.iter().enumerate() {
            let offset = offset(index * 8)?;
            self.b
                .ins()
                .store(MemFlags::trusted(), *value, array, offset);
        }
        Ok(array)
    }

    fn load(&mut self, address: Value) -> Value {
        self.b.ins().load(I64, MemFlags::trusted(), address, 0)
    }

    // Ownership.

    /// `operand`, owned: retained if it was borrowed.
    pub fn own(&mut self, operand: Operand, ty: &Type) -> Result<Value> {
        if !operand.owned && is_object(ty) {
            let desc = self.desc(ty)?;
            let retain = self.shared.runtime.retain;
            self.call(retain, &[operand.value, desc]);
        }
        Ok(operand.value)
    }

    /// Lets go of `operand`: released if it was owned.
    pub fn discard(&mut self, operand: Operand, ty: &Type) -> Result<()> {
        if operand.owned && is_object(ty) {
            self.release(operand.value, ty)?;
        }
        Ok(())
    }

    pub fn release(&mut self, value: Value, ty: &Type) -> Result<()> {
        let desc = self.desc(ty)?;
        let release = self.shared.runtime.release;
        self.call(release, &[value, desc]);
        Ok(())
    }

    /// Writes an owned `value` of type `ty` to the slot at `place`, releasing what was
    /// there.
    pub fn store(&mut self, place: Value, value: Value, ty: &Type) -> Result<()> {
        if is_object(ty) {
            let desc = self.desc(ty)?;
            let store = self.shared.runtime.store;
            self.call(store, &[place, value, desc]);
        } else {
            self.b.ins().store(MemFlags::trusted(), value, place, 0);
        }
        Ok(())
    }

    pub fn slot(&mut self, local: usize) -> Result<Value> {
        let offset = i64::try_from(local)
            .ok()
            .and_then(|local| local.checked_mul(SLOT))
            .ok_or_else(|| CodegenError::new("a frame has too many locals"))?;
        Ok(self.b.ins().iadd_imm(self.frame, offset))
    }

    /// Releases every local of the frame, as a function does before it returns.
    pub fn release_locals(&mut self) -> Result<()> {
        for (local, ty) in self.locals.iter().enumerate() {
            if is_object(ty) {
                let slot = self.slot(local)?;
                let value = self.load(slot);
                self.release(value, ty)?;
            }
        }
        Ok(())
    }

    // Places.

    /// Where `place` is: its root, then its steps, with its list positions worked out now.
    pub fn path(&mut self, place: &Place) -> Result<Alias> {
        let mut steps = Vec::new();
        for step in &place.path {
            steps.push(match step {
                PlaceStep::Field(index) => Step::Field(*index),
                PlaceStep::Index(index) => Step::Index(Index::Value(self.int(index)?)),
            });
        }
        let mut path = match place.root {
            PlaceRoot::State(state) => Alias {
                root: Root::State(state),
                steps: Vec::new(),
            },
            PlaceRoot::Local(local) => self.aliases.get(&local).cloned().unwrap_or(Alias {
                root: Root::Local(local.0),
                steps: Vec::new(),
            }),
        };
        path.steps.extend(steps);
        Ok(path)
    }

    fn index(&mut self, index: Index) -> Result<Value> {
        Ok(match index {
            Index::Value(value) => value,
            Index::Var(variable) => self.b.use_var(variable),
            Index::Slot(local) => {
                let at = offset(local * 8)?;
                self.b.ins().load(I64, MemFlags::trusted(), self.frame, at)
            }
        })
    }

    /// The slot `path` leads to, made its only owner's at each step so it can be changed.
    /// Writing through it marks a state root changed.
    pub fn pointer(&mut self, path: &Alias, span: Span) -> Result<Value> {
        let mut pointer = match path.root {
            Root::State(state) => {
                let state = self.u32(state.0)?;
                let state_mut = self.shared.runtime.state_mut;
                self.call1(state_mut, &[self.context, state])
            }
            Root::Local(local) => self.slot(local)?,
        };
        for step in &path.steps {
            pointer = match *step {
                Step::Field(field) => {
                    let field = self.usize(field)?;
                    let field_mut = self.shared.runtime.struct_field_mut;
                    self.call1(field_mut, &[pointer, field])
                }
                Step::Index(index) => {
                    let index = self.index(index)?;
                    let out = self.scratch();
                    let span = self.span(span)?;
                    let item_mut = self.shared.runtime.list_item_mut;
                    self.call_checked(item_mut, &[self.context, pointer, index, span, out]);
                    self.load(out)
                }
            };
        }
        Ok(pointer)
    }

    /// The value `path` leads to, borrowed.
    pub fn read(&mut self, path: &Alias, span: Span) -> Result<Value> {
        let mut value = match path.root {
            Root::State(state) => {
                let state = self.u32(state.0)?;
                let read = self.shared.runtime.state;
                self.call1(read, &[self.context, state])
            }
            Root::Local(local) => {
                let slot = self.slot(local)?;
                self.load(slot)
            }
        };
        for step in &path.steps {
            value = match *step {
                Step::Field(field) => {
                    let field = self.usize(field)?;
                    let get = self.shared.runtime.struct_get;
                    self.call1(get, &[value, field])
                }
                Step::Index(index) => {
                    let index = self.index(index)?;
                    self.list_item(value, index, span)?
                }
            };
        }
        Ok(value)
    }

    /// The item at `index` of the borrowed `list`, borrowed from it.
    pub fn list_item(&mut self, list: Value, index: Value, span: Span) -> Result<Value> {
        let out = self.scratch();
        let span = self.span(span)?;
        let get = self.shared.runtime.list_get;
        self.call_checked(get, &[self.context, list, index, span, out]);
        Ok(self.load(out))
    }

    // Statements.

    pub fn block(&mut self, stmts: &Stmts) -> Result<()> {
        stmts.iter().try_for_each(|stmt| self.stmt(stmt))
    }

    fn stmt(&mut self, stmt: &Stmt) -> Result<()> {
        match stmt {
            Stmt::Let(local, value) => {
                let operand = self.expr(value)?;
                let value_owned = self.own(operand, &value.ty)?;
                let slot = self.slot(local.0)?;
                self.store(slot, value_owned, &value.ty)
            }
            Stmt::Assign { place, op, value } => self.assign(place, *op, value),
            Stmt::If {
                branches,
                otherwise,
            } => self.if_stmt(branches, otherwise.as_ref()),
            Stmt::For { head, body } => match &head.iterable {
                Iterable::Value(list) => self.for_value(head, list, body),
                Iterable::Place(place) => self.for_place(head, place, body),
                Iterable::Range { start, end } => self.for_range(head.item, start, end, body),
            },
            Stmt::Return(value) => self.return_stmt(value.as_ref()),
            Stmt::Expr(expr) => {
                let operand = self.expr(expr)?;
                self.discard(operand, &expr.ty)
            }
        }
    }

    /// The value is worked out before the place, as in the interpreter.
    fn assign(&mut self, place: &Place, op: Option<BinaryOp>, value: &Expr) -> Result<()> {
        let operand = self.expr(value)?;
        let value_owned = self.own(operand, &value.ty)?;
        let path = self.path(place)?;
        let pointer = self.pointer(&path, place.span)?;
        let result = match op {
            Some(op) => {
                let current = Operand::borrowed(self.load(pointer));
                let result = self.apply(
                    op,
                    &place.ty,
                    current,
                    Operand::owned(value_owned),
                    place.span,
                )?;
                let result_ty = op_type(op, &place.ty);
                self.own(result, &result_ty)?
            }
            None => value_owned,
        };
        self.store(pointer, result, &place.ty)
    }

    fn if_stmt(&mut self, branches: &[(Expr, Stmts)], otherwise: Option<&Stmts>) -> Result<()> {
        let done = self.b.create_block();
        for (condition, body) in branches {
            let condition = self.expr(condition)?.value;
            let then = self.b.create_block();
            let next = self.b.create_block();
            self.b.ins().brif(condition, then, &[], next, &[]);
            self.b.switch_to_block(then);
            self.block(body)?;
            self.b.ins().jump(done, &[]);
            self.b.switch_to_block(next);
        }
        if let Some(otherwise) = otherwise {
            self.block(otherwise)?;
        }
        self.b.ins().jump(done, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    /// Walks a copy of the list, putting each item in the loop's item.
    fn for_value(&mut self, head: &Loop, list: &Expr, body: &Stmts) -> Result<()> {
        let operand = self.expr(list)?;
        let list_owned = self.own(operand, &list.ty)?;
        self.temporaries.push((list_owned, list.ty.clone()));
        let element = element_type(&list.ty)?;
        let len_fn = self.shared.runtime.list_len;
        let len = self.call1(len_fn, &[list_owned]);
        self.count_up(len, |this, index| {
            let item = this.list_item(list_owned, index, list.span)?;
            let item = this.own(Operand::borrowed(item), &element)?;
            let slot = this.slot(head.item.0)?;
            this.store(slot, item, &element)?;
            this.set_index(head.index, index)?;
            this.block(body)
        })?;
        self.temporaries.pop();
        self.release(list_owned, &list.ty)
    }

    /// Walks the list at `place`, with the loop's item standing for each of its items in
    /// turn. The length is read each time round, so the body may change the list.
    fn for_place(&mut self, head: &Loop, place: &Place, body: &Stmts) -> Result<()> {
        let path = self.path(place)?;
        let counter = self.b.declare_var(I64);
        let mut item = path.clone();
        item.steps.push(Step::Index(Index::Var(counter)));
        self.aliases.insert(head.item, item);
        let zero = self.b.ins().iconst(I64, 0);
        let len_fn = self.shared.runtime.list_len;
        self.count_with(
            counter,
            zero,
            |this| {
                let list = this.read(&path, place.span)?;
                Ok(this.call1(len_fn, &[list]))
            },
            |this, index| {
                this.set_index(head.index, index)?;
                this.block(body)
            },
        )?;
        self.aliases.remove(&head.item);
        Ok(())
    }

    /// Counts `local` from `start` up to, but not including, `end`.
    fn for_range(&mut self, local: LocalId, start: &Expr, end: &Expr, body: &Stmts) -> Result<()> {
        let start = self.int(start)?;
        let end = self.int(end)?;
        self.count(start, end, |this, count| {
            let slot = this.slot(local.0)?;
            this.store(slot, count, &Type::Int)?;
            this.block(body)
        })
    }

    /// Puts a round's position in the loop's index, when the loop names one.
    fn set_index(&mut self, index: Option<LocalId>, position: Value) -> Result<()> {
        let Some(index) = index else {
            return Ok(());
        };
        let slot = self.slot(index.0)?;
        self.store(slot, position, &Type::Int)
    }

    /// Runs `step` with each index from 0 up to `len`.
    pub fn count_up(
        &mut self,
        len: Value,
        step: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<()> {
        let zero = self.b.ins().iconst(I64, 0);
        self.count(zero, len, step)
    }

    /// Runs `step` with each count from `start` up to, but not including, `end`.
    pub fn count(
        &mut self,
        start: Value,
        end: Value,
        step: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<()> {
        let counter = self.b.declare_var(I64);
        self.count_with(counter, start, |_| Ok(end), step)
    }

    /// Runs `step` with each value of `counter` from `start` while it is below what `end`
    /// gives; `end` is worked out again each time round.
    fn count_with(
        &mut self,
        counter: Variable,
        start: Value,
        mut end: impl FnMut(&mut Self) -> Result<Value>,
        mut step: impl FnMut(&mut Self, Value) -> Result<()>,
    ) -> Result<()> {
        self.b.def_var(counter, start);
        let header = self.b.create_block();
        let looping = self.b.create_block();
        let done = self.b.create_block();
        self.b.ins().jump(header, &[]);
        self.b.switch_to_block(header);
        let end = end(self)?;
        let count = self.b.use_var(counter);
        let ended = self
            .b
            .ins()
            .icmp(IntCC::SignedGreaterThanOrEqual, count, end);
        self.b.ins().brif(ended, done, &[], looping, &[]);
        self.b.switch_to_block(looping);
        let count = self.b.use_var(counter);
        step(self, count)?;
        let count = self.b.use_var(counter);
        let next = self.b.ins().iadd_imm(count, 1);
        self.b.def_var(counter, next);
        self.b.ins().jump(header, &[]);
        self.b.switch_to_block(done);
        Ok(())
    }

    fn return_stmt(&mut self, value: Option<&Expr>) -> Result<()> {
        let returns = self
            .returns
            .ok_or_else(|| CodegenError::new("`return` outside a function"))?;
        if let Some(value) = value {
            let operand = self.expr(value)?;
            let result = self.own(operand, &value.ty)?;
            self.b
                .ins()
                .store(MemFlags::trusted(), result, returns.out, 0);
        }
        for (temporary, ty) in self.temporaries.clone().iter().rev() {
            self.release(*temporary, ty)?;
        }
        self.release_locals()?;
        self.succeed();
        self.continue_after_terminator();
        Ok(())
    }

    // Expressions.

    /// An `int` (or `bool`) expression's value.
    fn int(&mut self, expr: &Expr) -> Result<Value> {
        Ok(self.expr(expr)?.value)
    }

    pub fn expr(&mut self, expr: &Expr) -> Result<Operand> {
        let span = expr.span;
        Ok(match &expr.kind {
            ExprKind::Int(value) => Operand::owned(self.b.ins().iconst(I64, *value)),
            ExprKind::Float(value) => {
                let bits = i64::from_ne_bytes(value.to_bits().to_ne_bytes());
                Operand::owned(self.b.ins().iconst(I64, bits))
            }
            ExprKind::Bool(value) => Operand::owned(self.b.ins().iconst(I64, i64::from(*value))),
            ExprKind::String(text) => Operand::owned(self.string(text)?),
            ExprKind::Interpolate(segments) => Operand::owned(self.interpolate(segments)?),
            ExprKind::State(state) => {
                let path = Alias {
                    root: Root::State(*state),
                    steps: Vec::new(),
                };
                Operand::borrowed(self.read(&path, span)?)
            }
            ExprKind::Local(local) => {
                let path = self.aliases.get(local).cloned().unwrap_or(Alias {
                    root: Root::Local(local.0),
                    steps: Vec::new(),
                });
                Operand::borrowed(self.read(&path, span)?)
            }
            ExprKind::List(items) => Operand::owned(self.aggregate(items, &expr.ty)?),
            ExprKind::Construct(_, fields) => Operand::owned(self.aggregate(fields, &expr.ty)?),
            ExprKind::Field(base, field) => {
                let object = self.expr(base)?;
                let field = self.usize(*field)?;
                let get = self.shared.runtime.struct_get;
                let value = self.call1(get, &[object.value, field]);
                self.part_of(object, &base.ty, value, &expr.ty)?
            }
            ExprKind::Index(base, index) => {
                let list = self.expr(base)?;
                let list = self.hold(list, &base.ty, index)?;
                let index = self.int(index)?;
                let item = self.list_item(list.value, index, span)?;
                self.part_of(list, &base.ty, item, &expr.ty)?
            }
            ExprKind::Not(operand) => {
                let value = self.int(operand)?;
                let not = self.b.ins().icmp_imm(IntCC::Equal, value, 0);
                Operand::owned(self.b.ins().uextend(I64, not))
            }
            ExprKind::Negate(operand) => {
                let value = self.int(operand)?;
                Operand::owned(if operand.ty == Type::Float {
                    let float = self.float(value);
                    let negated = self.b.ins().fneg(float);
                    self.bits(negated)
                } else {
                    let overflows = self.b.ins().icmp_imm(IntCC::Equal, value, i64::MIN);
                    let too_large = self.shared.runtime.fail_too_large;
                    self.fail_if(overflows, too_large, span)?;
                    self.b.ins().ineg(value)
                })
            }
            ExprKind::Binary(op, lhs, rhs) => self.binary(*op, lhs, rhs, span)?,
            ExprKind::Call(function, args) => self.call_fn(function.0, args, &expr.ty, span)?,
            ExprKind::Len(value) => Operand::owned(self.len(value)?),
            ExprKind::IsEmpty(value) => {
                let len = self.len(value)?;
                let empty = self.b.ins().icmp_imm(IntCC::Equal, len, 0);
                Operand::owned(self.b.ins().uextend(I64, empty))
            }
            ExprKind::Push(place, value) => {
                self.push(place, value)?;
                self.nothing()
            }
            ExprKind::Remove(place, index) => {
                self.remove(place, index, span)?;
                self.nothing()
            }
            ExprKind::Download(download) => {
                self.download(download, span)?;
                self.nothing()
            }
            ExprKind::FormatBytes(bytes) => Operand::owned(self.format_bytes(bytes)?),
            ExprKind::DownloadCommand(command, id) => {
                self.download_command(*command, id)?;
                self.nothing()
            }
            ExprKind::Dialog(dialog) => {
                self.dialog(dialog, span)?;
                self.nothing()
            }
            ExprKind::Window(action, window) => {
                self.window_request(*action, *window)?;
                self.nothing()
            }
            ExprKind::Error => {
                return Err(CodegenError::new("programs with errors can't be compiled"));
            }
        })
    }

    /// What an expression of type `void` gives.
    fn nothing(&mut self) -> Operand {
        Operand::owned(self.b.ins().iconst(I64, 0))
    }

    /// A list or struct of type `ty` holding `parts`, worked out in order.
    fn aggregate(&mut self, parts: &[Expr], ty: &Type) -> Result<Value> {
        let values = self.owned_all(parts)?;
        let array = self.array(&values)?;
        let desc = self.desc(ty)?;
        Ok(if let Type::List(_) = ty {
            let len = self.usize(values.len())?;
            let new = self.shared.runtime.list_new;
            self.call1(new, &[desc, array, len])
        } else {
            let new = self.shared.runtime.struct_new;
            self.call1(new, &[desc, array])
        })
    }

    /// The value is worked out before the place, as in the interpreter.
    fn push(&mut self, place: &Place, value: &Expr) -> Result<()> {
        let operand = self.expr(value)?;
        let item = self.own(operand, &value.ty)?;
        let path = self.path(place)?;
        let list = self.pointer(&path, place.span)?;
        let push = self.shared.runtime.list_push;
        self.call(push, &[list, item]);
        Ok(())
    }

    fn remove(&mut self, place: &Place, index: &Expr, span: Span) -> Result<()> {
        let index = self.int(index)?;
        let path = self.path(place)?;
        let list = self.pointer(&path, place.span)?;
        let span = self.span(span)?;
        let remove = self.shared.runtime.list_remove;
        self.call_checked(remove, &[self.context, list, index, span]);
        Ok(())
    }

    /// Each expression's value, owned, in order.
    fn owned_all(&mut self, exprs: &[Expr]) -> Result<Vec<Value>> {
        exprs
            .iter()
            .map(|expr| {
                let operand = self.expr(expr)?;
                self.own(operand, &expr.ty)
            })
            .collect()
    }

    /// `operand`, kept while `next` is worked out: owned unless `next` can't change state.
    fn hold(&mut self, operand: Operand, ty: &Type, next: &Expr) -> Result<Operand> {
        if operand.owned || is_pure(next) {
            Ok(operand)
        } else {
            Ok(Operand::owned(self.own(operand, ty)?))
        }
    }

    /// A part borrowed from `container`: still borrowed if the container was, and
    /// otherwise owned, since the container is released.
    fn part_of(
        &mut self,
        container: Operand,
        container_ty: &Type,
        part: Value,
        part_ty: &Type,
    ) -> Result<Operand> {
        if !container.owned {
            return Ok(Operand::borrowed(part));
        }
        let part = self.own(Operand::borrowed(part), part_ty)?;
        self.release(container.value, container_ty)?;
        Ok(Operand::owned(part))
    }

    fn string(&mut self, text: &str) -> Result<Value> {
        let data = self.shared.text(self.module, text)?;
        let bytes = self.address(data);
        let len = self.usize(text.len())?;
        let new = self.shared.runtime.string_new;
        Ok(self.call1(new, &[bytes, len]))
    }

    fn interpolate(&mut self, segments: &[Segment]) -> Result<Value> {
        let new = self.shared.runtime.text_new;
        let text = self.call1(new, &[]);
        for segment in segments {
            match segment {
                Segment::Text(part) => {
                    let data = self.shared.text(self.module, part)?;
                    let bytes = self.address(data);
                    let len = self.usize(part.len())?;
                    let push = self.shared.runtime.text_push_str;
                    self.call(push, &[text, bytes, len]);
                }
                Segment::Value(value) => {
                    let operand = self.expr(value)?;
                    let desc = self.desc(&value.ty)?;
                    let push = self.shared.runtime.text_push;
                    self.call(push, &[text, operand.value, desc]);
                    self.discard(operand, &value.ty)?;
                }
            }
        }
        let finish = self.shared.runtime.text_finish;
        Ok(self.call1(finish, &[text]))
    }

    fn len(&mut self, value: &Expr) -> Result<Value> {
        let operand = self.expr(value)?;
        let len_fn = if value.ty == Type::String {
            self.shared.runtime.string_len
        } else {
            self.shared.runtime.list_len
        };
        let len = self.call1(len_fn, &[operand.value]);
        self.discard(operand, &value.ty)?;
        Ok(len)
    }

    fn float(&mut self, bits: Value) -> Value {
        self.b.ins().bitcast(F64, MemFlags::new(), bits)
    }

    fn bits(&mut self, float: Value) -> Value {
        self.b.ins().bitcast(I64, MemFlags::new(), float)
    }

    fn binary(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr, span: Span) -> Result<Operand> {
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            return self.short_circuit(op, lhs, rhs);
        }
        let left = self.expr(lhs)?;
        let left = self.hold(left, &lhs.ty, rhs)?;
        let right = self.expr(rhs)?;
        self.apply(op, &lhs.ty, left, right, span)
    }

    fn short_circuit(&mut self, op: BinaryOp, lhs: &Expr, rhs: &Expr) -> Result<Operand> {
        let left = self.int(lhs)?;
        let right_block = self.b.create_block();
        let done = self.b.create_block();
        self.b.append_block_param(done, I64);
        if op == BinaryOp::And {
            self.b
                .ins()
                .brif(left, right_block, &[], done, &[left.into()]);
        } else {
            self.b
                .ins()
                .brif(left, done, &[left.into()], right_block, &[]);
        }
        self.b.switch_to_block(right_block);
        let right = self.int(rhs)?;
        self.b.ins().jump(done, &[right.into()]);
        self.b.switch_to_block(done);
        Ok(Operand::owned(self.b.block_params(done)[0]))
    }

    /// Applies `op` to operands of type `ty`, releasing whichever are owned.
    pub fn apply(
        &mut self,
        op: BinaryOp,
        ty: &Type,
        left: Operand,
        right: Operand,
        span: Span,
    ) -> Result<Operand> {
        let (a, b) = (left.value, right.value);
        let result = match (op, ty) {
            (BinaryOp::Eq | BinaryOp::NotEq, ty) if is_object(ty) => {
                let desc = self.desc(ty)?;
                let equal_fn = self.shared.runtime.equal;
                let equal = self.call1(equal_fn, &[a, b, desc]);
                let equal = self.b.ins().uextend(I64, equal);
                if op == BinaryOp::Eq {
                    equal
                } else {
                    self.b.ins().bxor_imm(equal, 1)
                }
            }
            (BinaryOp::Add, Type::String) => {
                let concat = self.shared.runtime.string_concat;
                self.call1(concat, &[a, b])
            }
            (_, Type::String) => {
                let compare_fn = self.shared.runtime.string_compare;
                let order = self.call1(compare_fn, &[a, b]);
                let holds = self.b.ins().icmp_imm(int_condition(op)?, order, 0);
                self.b.ins().uextend(I64, holds)
            }
            (_, Type::Float) => self.float_op(op, a, b)?,
            (_, Type::Int | Type::Bool) => self.int_op(op, a, b, span)?,
            (op, ty) => {
                return Err(CodegenError::new(format!("can't apply {op:?} to {ty:?}")));
            }
        };
        self.discard(left, ty)?;
        self.discard(right, ty)?;
        Ok(Operand::owned(result))
    }

    fn int_op(&mut self, op: BinaryOp, a: Value, b: Value, span: Span) -> Result<Value> {
        let too_large = self.shared.runtime.fail_too_large;
        Ok(match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                let (result, overflowed) = match op {
                    BinaryOp::Add => self.b.ins().sadd_overflow(a, b),
                    BinaryOp::Sub => self.b.ins().ssub_overflow(a, b),
                    _ => self.b.ins().smul_overflow(a, b),
                };
                self.fail_if(overflowed, too_large, span)?;
                result
            }
            BinaryOp::Div | BinaryOp::Rem => {
                let by_zero = self.b.ins().icmp_imm(IntCC::Equal, b, 0);
                let divide_by_zero = self.shared.runtime.fail_divide_by_zero;
                self.fail_if(by_zero, divide_by_zero, span)?;
                let minimum = self.b.ins().icmp_imm(IntCC::Equal, a, i64::MIN);
                let minus_one = self.b.ins().icmp_imm(IntCC::Equal, b, -1);
                let overflows = self.b.ins().band(minimum, minus_one);
                self.fail_if(overflows, too_large, span)?;
                if op == BinaryOp::Div {
                    self.b.ins().sdiv(a, b)
                } else {
                    self.b.ins().srem(a, b)
                }
            }
            op => {
                let holds = self.b.ins().icmp(int_condition(op)?, a, b);
                self.b.ins().uextend(I64, holds)
            }
        })
    }

    fn float_op(&mut self, op: BinaryOp, a: Value, b: Value) -> Result<Value> {
        let (x, y) = (self.float(a), self.float(b));
        let result = match op {
            BinaryOp::Add => self.b.ins().fadd(x, y),
            BinaryOp::Sub => self.b.ins().fsub(x, y),
            BinaryOp::Mul => self.b.ins().fmul(x, y),
            BinaryOp::Div => self.b.ins().fdiv(x, y),
            BinaryOp::Rem => {
                let rem = self.shared.runtime.float_rem;
                self.call1(rem, &[x, y])
            }
            op => {
                let condition = match op {
                    BinaryOp::Eq => FloatCC::Equal,
                    BinaryOp::NotEq => FloatCC::NotEqual,
                    BinaryOp::Lt => FloatCC::LessThan,
                    BinaryOp::LtEq => FloatCC::LessThanOrEqual,
                    BinaryOp::Gt => FloatCC::GreaterThan,
                    BinaryOp::GtEq => FloatCC::GreaterThanOrEqual,
                    op => return Err(CodegenError::new(format!("can't apply {op:?} to floats"))),
                };
                let holds = self.b.ins().fcmp(condition, x, y);
                return Ok(self.b.ins().uextend(I64, holds));
            }
        };
        Ok(self.bits(result))
    }

    /// Calls Nifty function `function`. Its arguments are worked out before the call depth
    /// is checked, as in the interpreter.
    fn call_fn(
        &mut self,
        function: usize,
        args: &[Expr],
        ty: &Type,
        span: Span,
    ) -> Result<Operand> {
        let mut values = vec![self.context];
        values.extend(self.owned_all(args)?);
        let id = self.u32(function)?;
        let span = self.span(span)?;
        let enter = self.shared.runtime.enter;
        self.call_checked(enter, &[self.context, id, span]);
        let out = self.stack(8)?;
        let zero = self.b.ins().iconst(I64, 0);
        self.b.ins().store(MemFlags::trusted(), zero, out, 0);
        values.push(out);
        let compiled = self.shared.functions[function];
        self.call_checked(compiled, &values);
        let leave = self.shared.runtime.leave;
        self.call(leave, &[self.context]);
        let result = self.load(out);
        if *ty == Type::Void {
            Ok(Operand::owned(zero))
        } else {
            Ok(Operand::owned(result))
        }
    }

    /// `pick_file($path, …)` and the other file dialogs: asks for one, shown once the event
    /// finishes, whose answer goes to the bound state.
    fn dialog(&mut self, dialog: &Dialog, span: Span) -> Result<()> {
        let title = self.expr(&dialog.title)?;
        let title = self.hold(title, &Type::String, &dialog.types)?;
        let types = self.expr(&dialog.types)?;
        let types = self.hold(types, &dialog.types.ty, &dialog.folder)?;
        let folder = self.expr(&dialog.folder)?;
        let folder = self.hold(folder, &Type::String, &dialog.name)?;
        let name = self.expr(&dialog.name)?;
        let answer = self.path(&dialog.answer)?;
        let Some(answer) = self.place_desc(&answer)? else {
            return self.fail_now(self.shared.runtime.fail_dialog_into_variable, span);
        };
        let kind = self.b.ins().iconst(I32, i64::from(dialog.kind as u32));
        let span = self.span(span)?;
        let pick = self.shared.runtime.pick;
        self.call_checked(
            pick,
            &[
                self.context,
                kind,
                answer,
                title.value,
                types.value,
                folder.value,
                name.value,
                span,
            ],
        );
        self.discard(title, &Type::String)?;
        self.discard(types, &dialog.types.ty)?;
        self.discard(folder, &Type::String)?;
        self.discard(name, &Type::String)
    }

    /// `open_window(…)` or `close_window(…)`: asks for it, done once the event finishes.
    fn window_request(&mut self, action: WindowAction, window: WindowId) -> Result<()> {
        let action = self.b.ins().iconst(I32, i64::from(action as u32));
        let window = u32::try_from(window.0)
            .map_err(|_| CodegenError::new("the program has too many windows"))?;
        let window = self.b.ins().iconst(I32, i64::from(window));
        let request = self.shared.runtime.window;
        self.call(request, &[self.context, action, window]);
        Ok(())
    }

    /// Calls the runtime's `fail(context, span)` and leaves through the failure block.
    fn fail_now(&mut self, fail: FuncId, span: Span) -> Result<()> {
        let span = self.span(span)?;
        self.call(fail, &[self.context, span]);
        let fail = self.fail_block();
        self.b.ins().jump(fail, &[]);
        self.continue_after_terminator();
        Ok(())
    }

    /// `path` as a `nifty_rt::PlaceDesc` on the stack, or `None` if it isn't in state.
    fn place_desc(&mut self, path: &Alias) -> Result<Option<Value>> {
        let Root::State(state) = path.root else {
            return Ok(None);
        };
        let step_size = size_of::<PathStep>();
        let steps = self.stack(path.steps.len().max(1) * step_size)?;
        for (index, step) in path.steps.iter().enumerate() {
            let (kind, value) = match *step {
                Step::Field(field) => (StepKind::Field, self.usize(field)?),
                Step::Index(position) => (StepKind::Index, self.index(position)?),
            };
            let kind = self.b.ins().iconst(I8, i64::from(kind as u8));
            let kind_at = offset(index * step_size + offset_of!(PathStep, kind))?;
            let index_at = offset(index * step_size + offset_of!(PathStep, index))?;
            self.b
                .ins()
                .store(MemFlags::trusted(), kind, steps, kind_at);
            self.b
                .ins()
                .store(MemFlags::trusted(), value, steps, index_at);
        }
        let desc = self.stack(size_of::<PlaceDesc>())?;
        let state = self.u32(state.0)?;
        let len = self.usize(path.steps.len())?;
        let fields = [
            (state, offset_of!(PlaceDesc, state)),
            (steps, offset_of!(PlaceDesc, steps)),
            (len, offset_of!(PlaceDesc, len)),
        ];
        for (value, at) in fields {
            let at = offset(at)?;
            self.b.ins().store(MemFlags::trusted(), value, desc, at);
        }
        Ok(Some(desc))
    }
}

/// A byte offset as Cranelift takes it.
fn offset(bytes: usize) -> Result<i32> {
    i32::try_from(bytes).map_err(|_| CodegenError::new("a frame or place is too large"))
}

/// Whether working out `expr` can't change state or locals, so values borrowed before it
/// stay alive.
pub(super) fn is_pure(expr: &Expr) -> bool {
    match &expr.kind {
        ExprKind::Int(_)
        | ExprKind::Float(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::State(_)
        | ExprKind::Local(_) => true,
        ExprKind::Interpolate(segments) => segments.iter().all(|segment| match segment {
            Segment::Text(_) => true,
            Segment::Value(value) => is_pure(value),
        }),
        ExprKind::List(items) | ExprKind::Construct(_, items) => items.iter().all(is_pure),
        ExprKind::Field(base, _)
        | ExprKind::Not(base)
        | ExprKind::Negate(base)
        | ExprKind::FormatBytes(base)
        | ExprKind::Len(base)
        | ExprKind::IsEmpty(base) => is_pure(base),
        ExprKind::Index(base, index) | ExprKind::Binary(_, base, index) => {
            is_pure(base) && is_pure(index)
        }
        ExprKind::Call(..)
        | ExprKind::Push(..)
        | ExprKind::Remove(..)
        | ExprKind::Download(_)
        | ExprKind::DownloadCommand(..)
        | ExprKind::Dialog(_)
        | ExprKind::Window(..)
        | ExprKind::Error => false,
    }
}

fn int_condition(op: BinaryOp) -> Result<IntCC> {
    Ok(match op {
        BinaryOp::Eq => IntCC::Equal,
        BinaryOp::NotEq => IntCC::NotEqual,
        BinaryOp::Lt => IntCC::SignedLessThan,
        BinaryOp::LtEq => IntCC::SignedLessThanOrEqual,
        BinaryOp::Gt => IntCC::SignedGreaterThan,
        BinaryOp::GtEq => IntCC::SignedGreaterThanOrEqual,
        op => return Err(CodegenError::new(format!("{op:?} isn't a comparison"))),
    })
}

/// The type `op` gives when applied to `operands`.
fn op_type(op: BinaryOp, operands: &Type) -> Type {
    match op {
        BinaryOp::Eq
        | BinaryOp::NotEq
        | BinaryOp::Lt
        | BinaryOp::LtEq
        | BinaryOp::Gt
        | BinaryOp::GtEq
        | BinaryOp::And
        | BinaryOp::Or => Type::Bool,
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
            operands.clone()
        }
    }
}

pub(super) fn element_type(list: &Type) -> Result<Type> {
    match list {
        Type::List(element) => Ok(Type::clone(element)),
        other => Err(CodegenError::new(format!("{other:?} isn't a list"))),
    }
}
