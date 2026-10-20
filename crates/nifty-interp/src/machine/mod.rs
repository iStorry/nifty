//! Evaluates checked expressions and statements against the app's state.

mod carry;
mod place;
mod stmt;
mod walk;

use std::cell::{Cell, RefCell};
use std::fmt::{self, Write as _};
use std::mem;
use std::rc::Rc;

use nifty_check::hir::{
    BinaryOp, Dialog, DialogKind, Download, DownloadCommand, DownloadField, Expr, ExprKind, FnId,
    LocalId, Program, StateSet,
};
use nifty_download::{DownloadId, Report, Written, follow_id};
use nifty_runtime::{Failure, FileDialog, MAX_CALL_DEPTH, Picked, WindowRequest};
use nifty_syntax::Span;

use crate::value::Value;
pub(crate) use place::Location;
pub(crate) use stmt::Flow;
pub(crate) use walk::rounds;

/// A failure while the app runs, such as dividing by zero, pointing at the code responsible.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeError {
    pub message: String,
    pub span: Span,
}

impl RuntimeError {
    fn new(failure: &Failure, span: Span) -> Self {
        Self {
            message: failure.to_string(),
            span,
        }
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

pub(crate) type Result<T> = std::result::Result<T, RuntimeError>;

/// Where a download's reports go: the state each of its bindings bound.
#[derive(Debug)]
pub(crate) struct Targets {
    pub id: DownloadId,
    pub bindings: Vec<(DownloadField, Location)>,
    /// The `download(…)` call, which errors writing the reports point at.
    pub span: Span,
}

pub(crate) type Request = nifty_download::Request<Targets>;

/// Where a file dialog's answer goes: the state its call bound.
#[derive(Debug)]
pub(crate) struct Answer {
    pub place: Location,
    pub kind: DialogKind,
    /// The `pick_…(…)` call, which errors writing the answer point at.
    pub span: Span,
}

/// A local variable's storage. A `for` variable over a place holds where its element lives,
/// so writes through it reach the original list.
#[derive(Clone, Debug)]
pub(crate) enum Slot {
    Empty,
    Value(Value),
    Alias(Location),
}

pub(crate) type Frame = Vec<Slot>;

pub(crate) fn new_frame(locals: usize) -> Frame {
    vec![Slot::Empty; locals]
}

pub(crate) struct Machine {
    program: Rc<Program>,
    states: RefCell<Vec<Value>>,
    /// States changed since the UI last caught up.
    changed: RefCell<StateSet>,
    /// Downloads asked for since the app last started them.
    downloads: RefCell<Vec<Request>>,
    /// What code asked of running downloads since the app last passed it on, by their ids.
    download_commands: RefCell<Vec<(DownloadCommand, String)>>,
    /// File dialogs asked for since the app last passed them on.
    dialogs: RefCell<Vec<(FileDialog, Answer)>>,
    /// Windows asked to open or close since the app last passed them on.
    window_requests: RefCell<Vec<WindowRequest>>,
    call_depth: Cell<usize>,
}

impl Machine {
    /// Sets every state to its starting value, or to its value in `previous` when it still
    /// fits there.
    pub(crate) fn new(program: Rc<Program>, previous: Option<&Machine>) -> Result<Self> {
        let machine = Self {
            program,
            states: RefCell::new(Vec::new()),
            changed: RefCell::new(StateSet::default()),
            downloads: RefCell::new(Vec::new()),
            download_commands: RefCell::new(Vec::new()),
            dialogs: RefCell::new(Vec::new()),
            window_requests: RefCell::new(Vec::new()),
            call_depth: Cell::new(0),
        };
        let program = Rc::clone(&machine.program);
        for state in &program.states {
            let kept = previous.and_then(|previous| carry::kept_value(previous, state, &program));
            let value = match kept {
                Some(value) => value,
                None => machine.eval(&state.init, &mut Frame::new())?,
            };
            machine.states.borrow_mut().push(value);
        }
        Ok(machine)
    }

    pub(crate) fn program(&self) -> &Rc<Program> {
        &self.program
    }

    #[cfg(test)]
    pub(crate) fn state(&self, state: nifty_check::hir::StateId) -> Value {
        self.states.borrow()[state.0].clone()
    }

    /// The states changed since the last call.
    pub(crate) fn take_changed(&self) -> StateSet {
        mem::take(&mut self.changed.borrow_mut())
    }

    /// The downloads asked for since the last call.
    pub(crate) fn take_downloads(&self) -> Vec<Request> {
        mem::take(&mut self.downloads.borrow_mut())
    }

    /// What code asked of running downloads since the last call, in order.
    pub(crate) fn take_download_commands(&self) -> Vec<(DownloadCommand, String)> {
        mem::take(&mut self.download_commands.borrow_mut())
    }

    /// The file dialogs asked for since the last call.
    pub(crate) fn take_dialogs(&self) -> Vec<(FileDialog, Answer)> {
        mem::take(&mut self.dialogs.borrow_mut())
    }

    /// The windows asked to open or close since the last call, in order.
    pub(crate) fn take_window_requests(&self) -> Vec<WindowRequest> {
        mem::take(&mut self.window_requests.borrow_mut())
    }

    /// Writes the path or paths the user picked into the state the dialog's call bound;
    /// cancelling changes nothing.
    pub(crate) fn answer_dialog(&self, answer: &Answer, picked: Picked) -> Result<()> {
        let paths = match picked {
            Picked::Chosen(paths) => paths,
            Picked::Cancelled => return Ok(()),
            Picked::Failed(reason) => {
                return Err(RuntimeError::new(&Failure::Dialog(reason), answer.span));
            }
        };
        let value = if answer.kind.picks_many() {
            Value::strings(paths)
        } else {
            let path = paths
                .into_iter()
                .next()
                .expect("backends answer a one-path dialog with exactly one path");
            Value::String(Rc::from(path))
        };
        self.write_state(&answer.place, value, answer.span)
    }

    /// Asks for a file dialog, shown once the current event finishes.
    fn ask_dialog(&self, dialog: &Dialog, frame: &mut Frame, span: Span) -> Result<()> {
        let title = self.eval(&dialog.title, frame)?.as_str().to_owned();
        let types = self.eval(&dialog.types, frame)?;
        let folder = self.eval(&dialog.folder, frame)?;
        let name = self.eval(&dialog.name, frame)?.as_str().to_owned();
        let place = self.locate(&dialog.answer, frame)?;
        if !place.is_in_state() {
            return Err(RuntimeError::new(&Failure::DialogIntoVariable, span));
        }
        let Value::List(types) = types else {
            panic!("checked code gave file types as {types:?}");
        };
        let types: Vec<&str> = types.iter().map(Value::as_str).collect();
        let file_dialog = FileDialog::new(dialog.kind, title, &types, folder.as_str(), name)
            .map_err(|error| RuntimeError::new(&Failure::Dialog(error.to_string()), span))?;
        self.dialogs.borrow_mut().push((
            file_dialog,
            Answer {
                place,
                kind: dialog.kind,
                span,
            },
        ));
        Ok(())
    }

    /// Writes a download's report into the state its call bound.
    pub(crate) fn report_download(&self, targets: &Targets, report: &Report) -> Result<()> {
        for (field, written) in report.writes() {
            let bound = targets.bindings.iter().find(|(bound, _)| *bound == field);
            if let Some((_, location)) = bound {
                self.write_state(location, written_value(written), targets.span)?;
            }
        }
        Ok(())
    }

    /// Moves a download's targets to where the item holding its id is now; `false` when
    /// nothing holds it any more.
    pub(crate) fn follow_download(&self, targets: &mut Targets) -> bool {
        follow_id(self, &mut targets.bindings, &targets.id.to_string())
    }

    /// Queues a download for the app to start once the current event finishes, and shows
    /// that it has begun.
    fn request_download(&self, download: &Download, frame: &mut Frame, span: Span) -> Result<()> {
        let url = self.eval(&download.url, frame)?.as_str().to_owned();
        let folder = self.eval(&download.folder, frame)?.as_str().to_owned();
        let mut bindings = Vec::with_capacity(download.bindings.len());
        for (field, place) in &download.bindings {
            let location = self.locate(place, frame)?;
            if !location.is_in_state() {
                return Err(RuntimeError::new(&Failure::DownloadIntoVariable, span));
            }
            bindings.push((*field, location));
        }
        let id = DownloadId::next();
        let targets = Targets { id, bindings, span };
        self.report_download(&targets, &Report::Started(id))?;
        self.downloads.borrow_mut().push(Request {
            id,
            url,
            folder,
            targets,
        });
        Ok(())
    }

    pub(crate) fn eval(&self, expr: &Expr, frame: &mut Frame) -> Result<Value> {
        let span = expr.span;
        Ok(match &expr.kind {
            ExprKind::Int(value) => Value::Int(*value),
            ExprKind::Float(value) => Value::Float(*value),
            ExprKind::Bool(value) => Value::Bool(*value),
            ExprKind::String(text) => Value::String(Rc::clone(text)),
            ExprKind::Interpolate(segments) => {
                let mut text = String::new();
                for segment in segments {
                    match segment {
                        nifty_check::hir::Segment::Text(part) => text.push_str(part),
                        nifty_check::hir::Segment::Value(value) => {
                            let value = self.eval(value, frame)?;
                            write!(text, "{value}").expect("writing to a String cannot fail");
                        }
                    }
                }
                Value::String(Rc::from(text))
            }
            ExprKind::State(state) => self.states.borrow()[state.0].clone(),
            ExprKind::Local(local) => self.read_local(*local, frame, span)?,
            ExprKind::List(items) => Value::List(Rc::new(self.eval_all(items, frame)?)),
            ExprKind::Construct(_, fields) => Value::Struct(Rc::new(self.eval_all(fields, frame)?)),
            ExprKind::Field(base, index) => match self.eval(base, frame)? {
                Value::Struct(fields) => fields[*index].clone(),
                other => panic!("checked code read a field of {other:?}"),
            },
            ExprKind::Index(base, index) => {
                let list = self.eval(base, frame)?;
                let position = position(self.eval(index, frame)?.as_int(), span)?;
                item(&list, position, span)?.clone()
            }
            ExprKind::Not(operand) => Value::Bool(!self.eval(operand, frame)?.as_bool()),
            ExprKind::Negate(operand) => match self.eval(operand, frame)? {
                Value::Int(value) => Value::Int(
                    value
                        .checked_neg()
                        .ok_or_else(|| RuntimeError::new(&Failure::TooLarge, span))?,
                ),
                Value::Float(value) => Value::Float(-value),
                other => panic!("checked code negated {other:?}"),
            },
            ExprKind::Binary(op, lhs, rhs) => self.binary_expr(*op, lhs, rhs, frame, span)?,
            ExprKind::Call(function, args) => self.call(*function, args, frame, span)?,
            ExprKind::Len(value) => Value::Int(length(&self.eval(value, frame)?, span)?),
            ExprKind::IsEmpty(value) => Value::Bool(length(&self.eval(value, frame)?, span)? == 0),
            ExprKind::Push(place, value) => {
                let value = self.eval(value, frame)?;
                let location = self.locate(place, frame)?;
                self.modify(&location, frame, span, |list| {
                    list_items(list).push(value);
                    Ok(())
                })?;
                Value::Void
            }
            ExprKind::Remove(place, index) => {
                let position = position(self.eval(index, frame)?.as_int(), span)?;
                let location = self.locate(place, frame)?;
                self.modify(&location, frame, span, |list| {
                    let items = list_items(list);
                    if position >= items.len() {
                        return Err(out_of_range(position, items.len(), span));
                    }
                    items.remove(position);
                    Ok(())
                })?;
                Value::Void
            }
            ExprKind::Download(download) => {
                self.request_download(download, frame, span)?;
                Value::Void
            }
            ExprKind::DownloadCommand(command, id) => {
                let id = self.eval(id, frame)?.as_str().to_owned();
                self.download_commands.borrow_mut().push((*command, id));
                Value::Void
            }
            ExprKind::FormatBytes(bytes) => Value::String(Rc::from(nifty_download::format_bytes(
                self.eval(bytes, frame)?.as_int(),
            ))),
            ExprKind::Dialog(dialog) => {
                self.ask_dialog(dialog, frame, span)?;
                Value::Void
            }
            ExprKind::Window(action, window) => {
                self.window_requests.borrow_mut().push(WindowRequest {
                    action: *action,
                    window: window.0,
                });
                Value::Void
            }
            ExprKind::Error => panic!("programs with errors are never run"),
        })
    }

    fn eval_all(&self, exprs: &[Expr], frame: &mut Frame) -> Result<Vec<Value>> {
        exprs.iter().map(|expr| self.eval(expr, frame)).collect()
    }

    fn read_local(&self, local: LocalId, frame: &Frame, span: Span) -> Result<Value> {
        match &frame[local.0] {
            Slot::Value(value) => Ok(value.clone()),
            Slot::Alias(location) => self.read(location, frame, span),
            Slot::Empty => panic!("the checker only allows reading locals after they are set"),
        }
    }

    fn binary_expr(
        &self,
        op: BinaryOp,
        lhs: &Expr,
        rhs: &Expr,
        frame: &mut Frame,
        span: Span,
    ) -> Result<Value> {
        match op {
            BinaryOp::And => Ok(Value::Bool(
                self.eval(lhs, frame)?.as_bool() && self.eval(rhs, frame)?.as_bool(),
            )),
            BinaryOp::Or => Ok(Value::Bool(
                self.eval(lhs, frame)?.as_bool() || self.eval(rhs, frame)?.as_bool(),
            )),
            _ => {
                let lhs = self.eval(lhs, frame)?;
                let rhs = self.eval(rhs, frame)?;
                binary(op, &lhs, &rhs, span)
            }
        }
    }

    pub(crate) fn call(
        &self,
        function: FnId,
        args: &[Expr],
        frame: &mut Frame,
        span: Span,
    ) -> Result<Value> {
        let program = Rc::clone(&self.program);
        let definition = &program.functions[function.0];
        let mut callee = new_frame(definition.locals.len());
        for (slot, arg) in callee.iter_mut().zip(args) {
            *slot = Slot::Value(self.eval(arg, frame)?);
        }
        let depth = self.call_depth.get();
        if depth >= MAX_CALL_DEPTH {
            let function = definition.name.clone();
            return Err(RuntimeError::new(&Failure::TooDeep { function }, span));
        }
        self.call_depth.set(depth + 1);
        let flow = self.exec_block(&definition.body, &mut callee);
        self.call_depth.set(depth);
        Ok(match flow? {
            Flow::Return(value) => value,
            Flow::Normal => Value::Void,
        })
    }
}

/// Applies a binary operator other than `&&` and `||` to evaluated operands.
pub(crate) fn binary(op: BinaryOp, lhs: &Value, rhs: &Value, span: Span) -> Result<Value> {
    use Value::{Bool, Float, Int, String};
    let too_large = || RuntimeError::new(&Failure::TooLarge, span);
    Ok(match (op, lhs, rhs) {
        (BinaryOp::Eq, _, _) => Bool(lhs == rhs),
        (BinaryOp::NotEq, _, _) => Bool(lhs != rhs),
        (BinaryOp::Add, Int(a), Int(b)) => Int(a.checked_add(*b).ok_or_else(too_large)?),
        (BinaryOp::Sub, Int(a), Int(b)) => Int(a.checked_sub(*b).ok_or_else(too_large)?),
        (BinaryOp::Mul, Int(a), Int(b)) => Int(a.checked_mul(*b).ok_or_else(too_large)?),
        (BinaryOp::Div | BinaryOp::Rem, Int(_), Int(0)) => {
            return Err(RuntimeError::new(&Failure::DivideByZero, span));
        }
        (BinaryOp::Div, Int(a), Int(b)) => Int(a.checked_div(*b).ok_or_else(too_large)?),
        (BinaryOp::Rem, Int(a), Int(b)) => Int(a.checked_rem(*b).ok_or_else(too_large)?),
        (BinaryOp::Add, Float(a), Float(b)) => Float(a + b),
        (BinaryOp::Sub, Float(a), Float(b)) => Float(a - b),
        (BinaryOp::Mul, Float(a), Float(b)) => Float(a * b),
        (BinaryOp::Div, Float(a), Float(b)) => Float(a / b),
        (BinaryOp::Rem, Float(a), Float(b)) => Float(a % b),
        (BinaryOp::Add, String(a), String(b)) => String(Rc::from(format!("{a}{b}"))),
        (BinaryOp::Lt | BinaryOp::LtEq | BinaryOp::Gt | BinaryOp::GtEq, _, _) => {
            let ordering = match (lhs, rhs) {
                (Int(a), Int(b)) => a.partial_cmp(b),
                (Float(a), Float(b)) => a.partial_cmp(b),
                (String(a), String(b)) => a.partial_cmp(b),
                _ => panic!("checked code compared {lhs:?} and {rhs:?}"),
            };
            Bool(ordering.is_some_and(|ordering| match op {
                BinaryOp::Lt => ordering.is_lt(),
                BinaryOp::LtEq => ordering.is_le(),
                BinaryOp::Gt => ordering.is_gt(),
                _ => ordering.is_ge(),
            }))
        }
        _ => panic!("checked code applied {op:?} to {lhs:?} and {rhs:?}"),
    })
}

/// The value a download's report writes, as the interpreter holds it.
fn written_value(written: Written) -> Value {
    match written {
        Written::Int(number) => Value::Int(number),
        Written::Text(text) => Value::String(Rc::from(text)),
    }
}

/// A list position from an index value, which must not be negative.
fn position(index: i64, span: Span) -> Result<usize> {
    usize::try_from(index).map_err(|_| RuntimeError::new(&Failure::NegativePosition(index), span))
}

fn item(list: &Value, position: usize, span: Span) -> Result<&Value> {
    let Value::List(items) = list else {
        panic!("checked code indexed into {list:?}");
    };
    items
        .get(position)
        .ok_or_else(|| out_of_range(position, items.len(), span))
}

fn list_items(list: &mut Value) -> &mut Vec<Value> {
    match list {
        Value::List(items) => Rc::make_mut(items),
        other => panic!("checked code changed {other:?} as a list"),
    }
}

fn length(value: &Value, span: Span) -> Result<i64> {
    let length = match value {
        Value::List(items) => items.len(),
        Value::String(text) => text.chars().count(),
        other => panic!("checked code measured {other:?}"),
    };
    i64::try_from(length).map_err(|_| RuntimeError::new(&Failure::TooLong, span))
}

fn out_of_range(position: usize, len: usize, span: Span) -> RuntimeError {
    RuntimeError::new(&Failure::NoItem { position, len }, span)
}
