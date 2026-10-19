//! Statement execution.

use nifty_check::hir::{Iterable, Stmt};

use super::{Frame, Machine, Result, Slot, binary};
use crate::value::Value;

/// How a block finished: by running to its end, or by `return`.
#[derive(Debug)]
pub(crate) enum Flow {
    Normal,
    Return(Value),
}

impl Machine {
    pub(crate) fn exec_block(&self, block: &[Stmt], frame: &mut Frame) -> Result<Flow> {
        for stmt in block {
            if let Flow::Return(value) = self.exec(stmt, frame)? {
                return Ok(Flow::Return(value));
            }
        }
        Ok(Flow::Normal)
    }

    fn exec(&self, stmt: &Stmt, frame: &mut Frame) -> Result<Flow> {
        match stmt {
            Stmt::Let(local, value) => {
                frame[local.0] = Slot::Value(self.eval(value, frame)?);
            }
            Stmt::Assign { place, op, value } => {
                let mut value = self.eval(value, frame)?;
                let location = self.locate(place, frame)?;
                if let Some(op) = op {
                    let current = self.read(&location, frame, place.span)?;
                    value = binary(*op, &current, &value, place.span)?;
                }
                self.write(&location, frame, value, place.span)?;
            }
            Stmt::If {
                branches,
                otherwise,
            } => {
                for (condition, body) in branches {
                    if self.eval(condition, frame)?.as_bool() {
                        return self.exec_block(body, frame);
                    }
                }
                if let Some(otherwise) = otherwise {
                    return self.exec_block(otherwise, frame);
                }
            }
            Stmt::For {
                local,
                iterable,
                body,
            } => match iterable {
                Iterable::Place(place) => {
                    let location = self.locate(place, frame)?;
                    let mut index = 0;
                    // The length is read each time round, so the body may change the list.
                    while index < list_len(&self.read(&location, frame, place.span)?) {
                        frame[local.0] = Slot::Alias(location.item(index));
                        if let Flow::Return(value) = self.exec_block(body, frame)? {
                            return Ok(Flow::Return(value));
                        }
                        index += 1;
                    }
                }
                Iterable::Value(list) => {
                    let Value::List(items) = self.eval(list, frame)? else {
                        panic!("the checker only allows looping over lists");
                    };
                    for item in items.iter() {
                        frame[local.0] = Slot::Value(item.clone());
                        if let Flow::Return(value) = self.exec_block(body, frame)? {
                            return Ok(Flow::Return(value));
                        }
                    }
                }
            },
            Stmt::Return(value) => {
                let value = match value {
                    Some(value) => self.eval(value, frame)?,
                    None => Value::Void,
                };
                return Ok(Flow::Return(value));
            }
            Stmt::Expr(expr) => {
                self.eval(expr, frame)?;
            }
        }
        Ok(Flow::Normal)
    }
}

pub(crate) fn list_len(list: &Value) -> usize {
    match list {
        Value::List(items) => items.len(),
        other => panic!("the checker only allows looping over lists, not {other:?}"),
    }
}
