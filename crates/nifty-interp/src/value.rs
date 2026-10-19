//! Runtime values. Lists and structs share their contents until one copy is changed, so
//! passing them around is cheap while keeping value semantics.

use std::fmt;
use std::rc::Rc;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Rc<str>),
    List(Rc<Vec<Value>>),
    Struct(Rc<Vec<Value>>),
    Void,
}

impl Value {
    /// # Panics
    /// When the value is not an `int`; the checker rules that out.
    #[must_use]
    pub fn as_int(&self) -> i64 {
        match self {
            Self::Int(value) => *value,
            other => panic!("checked code produced {other:?} where an int was expected"),
        }
    }

    /// # Panics
    /// When the value is not a `bool`; the checker rules that out.
    #[must_use]
    pub fn as_bool(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            other => panic!("checked code produced {other:?} where a bool was expected"),
        }
    }

    /// # Panics
    /// When the value is not a `string`; the checker rules that out.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::String(value) => value,
            other => panic!("checked code produced {other:?} where a string was expected"),
        }
    }
}

/// How a value reads inside an interpolated string.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int(value) => write!(f, "{value}"),
            Self::Float(value) => write!(f, "{value}"),
            Self::Bool(value) => write!(f, "{value}"),
            Self::String(value) => f.write_str(value),
            Self::List(items) | Self::Struct(items) => {
                f.write_str("[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                f.write_str("]")
            }
            Self::Void => Ok(()),
        }
    }
}
