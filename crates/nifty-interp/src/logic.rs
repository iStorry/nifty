//! The interpreter as the runtime engine's [`Logic`]: it evaluates the checked code each
//! template points at, and starts the downloads and file dialogs that code asks for.

use std::rc::{Rc, Weak};
use std::sync::Arc;

use nifty_check::hir::{Binding, Block, Dynamic, Expr, Iterable, LocalId, StateSet, ViewId};
use nifty_runtime::{Backend, Dialogs, Engine, Logic, Shown, Templates, Ui, Waker};

use crate::machine::{
    Answer, Frame, Machine, Result, RuntimeError, Slot, Targets, list_len, new_frame,
};
use crate::value::Value;

pub(crate) struct Interpreter {
    machine: Machine,
    downloads: nifty_download::Downloads<Targets>,
    dialogs: Dialogs<Answer>,
    /// Indexed by `ViewId`.
    views: Vec<ViewTemplate>,
}

/// A view's body, and how many locals its frame needs.
#[derive(Debug)]
pub(crate) struct ViewTemplate {
    pub body: Templates<Interpreter>,
    pub locals: usize,
}

#[derive(Debug)]
pub(crate) enum Region {
    /// Shows the first branch whose condition holds, or else `otherwise`.
    If {
        branches: Vec<(Rc<Expr>, Templates<Interpreter>)>,
        otherwise: Templates<Interpreter>,
    },
    /// Shows `body` once per item, with the item in `local`.
    For {
        local: LocalId,
        iterable: Rc<Iterable>,
        body: Templates<Interpreter>,
    },
    /// Shows the view's body, with the arguments as its parameters.
    View { view: ViewId, args: Rc<[Expr]> },
}

impl Interpreter {
    /// Runs `machine` for `engine`, which shows file dialogs and receives their answers and
    /// download reports through `ui`.
    pub(crate) fn new<B: Backend>(
        machine: Machine,
        ui: &Ui<B>,
        engine: Weak<Engine<Self, B>>,
    ) -> Self {
        let waker: Arc<dyn Waker> = Arc::new(ui.waker(Box::new(move || {
            if let Some(engine) = engine.upgrade() {
                engine.logic().dialogs.show_asked();
                engine.handle_event(Self::receive);
            }
        })));
        Self {
            views: crate::lower::views(machine.program()),
            machine,
            downloads: nifty_download::Downloads::new(Arc::clone(&waker)),
            dialogs: Dialogs::new(ui.dialogs(), waker),
        }
    }

    pub(crate) fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Waits until every running download has sent its last report, without applying them.
    #[cfg(test)]
    pub(crate) fn wait_for_downloads(&self) {
        self.downloads.wait();
    }

    /// Runs code for an event, then starts the downloads and asks for the dialogs it asked
    /// for, even if it failed after asking.
    fn event(&self, run: impl FnOnce(&Machine) -> Result<()>) -> Result<()> {
        let result = run(&self.machine);
        for request in self.machine.take_downloads() {
            self.downloads.start(request);
        }
        for (dialog, answer) in self.machine.take_dialogs() {
            self.dialogs.ask(dialog, answer);
        }
        result
    }

    fn receive(&self) -> Result<()> {
        self.downloads
            .receive(|targets, report| self.machine.report_download(targets, report))?;
        self.dialogs
            .receive(|answer, picked| self.machine.answer_dialog(answer, picked))
    }

    fn read(&self, binding: &Binding, frame: &Frame) -> Result<Value> {
        let mut scratch = frame.clone();
        let location = self.machine.locate(&binding.place, &mut scratch)?;
        self.machine.read(&location, &scratch, binding.place.span)
    }

    fn write(&self, binding: &Binding, frame: &Frame, value: Value) -> Result<()> {
        self.event(|machine| {
            let mut scratch = frame.clone();
            let location = machine.locate(&binding.place, &mut scratch)?;
            machine.write(&location, &mut scratch, value, binding.place.span)
        })
    }

    /// The frames a `for` region shows its body in, one per item.
    fn item_frames(
        &self,
        local: LocalId,
        iterable: &Iterable,
        frame: &Frame,
    ) -> Result<Vec<Rc<Frame>>> {
        let with_item = |item: Slot| {
            let mut item_frame = frame.clone();
            item_frame[local.0] = item;
            Rc::new(item_frame)
        };
        let mut scratch = frame.clone();
        Ok(match iterable {
            Iterable::Place(place) => {
                let location = self.machine.locate(place, &mut scratch)?;
                let len = list_len(&self.machine.read(&location, &scratch, place.span)?);
                (0..len)
                    .map(|index| with_item(Slot::Alias(location.item(index))))
                    .collect()
            }
            Iterable::Value(list) => match self.machine.eval(list, &mut scratch)? {
                Value::List(items) => items
                    .iter()
                    .cloned()
                    .map(Slot::Value)
                    .map(with_item)
                    .collect(),
                other => panic!("the checker only allows looping over lists, not {other:?}"),
            },
        })
    }
}

impl Logic for Interpreter {
    type Frame = Frame;
    type Value = Rc<Dynamic>;
    type Binding = Rc<Binding>;
    type Action = Block;
    type Region = Region;
    type Error = RuntimeError;

    fn text(&self, text: &Rc<Dynamic>, frame: &Frame) -> Result<String> {
        let value = self.machine.eval(&text.expr, &mut frame.clone())?;
        Ok(value.as_str().to_owned())
    }

    fn number(&self, number: &Rc<Dynamic>, frame: &Frame) -> Result<i64> {
        Ok(self
            .machine
            .eval(&number.expr, &mut frame.clone())?
            .as_int())
    }

    fn read_text(&self, binding: &Rc<Binding>, frame: &Frame) -> Result<String> {
        Ok(self.read(binding, frame)?.as_str().to_owned())
    }

    fn read_checked(&self, binding: &Rc<Binding>, frame: &Frame) -> Result<bool> {
        Ok(self.read(binding, frame)?.as_bool())
    }

    fn write_text(&self, binding: &Rc<Binding>, frame: &Frame, text: &str) -> Result<()> {
        self.write(binding, frame, Value::String(Rc::from(text)))
    }

    fn write_checked(&self, binding: &Rc<Binding>, frame: &Frame, checked: bool) -> Result<()> {
        self.write(binding, frame, Value::Bool(checked))
    }

    fn run(&self, action: &Block, frame: &Frame) -> Result<()> {
        self.event(|machine| {
            machine.exec_block(action, &mut frame.clone())?;
            Ok(())
        })
    }

    fn expand(&self, region: &Region, frame: &Rc<Frame>) -> Result<Vec<Shown<Self>>> {
        match region {
            Region::If {
                branches,
                otherwise,
            } => {
                let mut scratch = Frame::clone(frame);
                let mut shown = otherwise;
                for (condition, body) in branches {
                    if self.machine.eval(condition, &mut scratch)?.as_bool() {
                        shown = body;
                        break;
                    }
                }
                Ok(vec![Shown {
                    templates: Rc::clone(shown),
                    frame: Rc::clone(frame),
                }])
            }
            Region::For {
                local,
                iterable,
                body,
            } => Ok(self
                .item_frames(*local, iterable, frame)?
                .into_iter()
                .map(|frame| Shown {
                    templates: Rc::clone(body),
                    frame,
                })
                .collect()),
            Region::View { view, args } => {
                let view = &self.views[view.0];
                let mut scratch = Frame::clone(frame);
                let mut view_frame = new_frame(view.locals);
                for (slot, arg) in view_frame.iter_mut().zip(args.iter()) {
                    *slot = Slot::Value(self.machine.eval(arg, &mut scratch)?);
                }
                Ok(vec![Shown {
                    templates: Rc::clone(&view.body),
                    frame: Rc::new(view_frame),
                }])
            }
        }
    }

    fn take_changed(&self) -> StateSet {
        self.machine.take_changed()
    }
}
