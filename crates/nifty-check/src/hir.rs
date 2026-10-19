//! The checked program: every name resolved to an id, every expression typed, and every
//! dynamic part of the UI annotated with the state it reads.

use std::rc::Rc;

use nifty_runtime::{ButtonStyle, Size, StackStyle, TextStyle};
use nifty_syntax::Span;
pub use nifty_syntax::ast::BinaryOp;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    };
}

id!(StructId);
id!(StateId);
id!(FnId);
id!(ViewId);
id!(LocalId);

#[derive(Debug, Default)]
pub struct Program {
    pub structs: Vec<StructDef>,
    pub states: Vec<StateDef>,
    pub functions: Vec<FnDef>,
    pub views: Vec<ViewDef>,
    pub windows: Vec<WindowDef>,
}

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug)]
pub struct StateDef {
    pub name: String,
    pub ty: Type,
    pub init: Expr,
}

/// Parameters occupy the first locals, in order.
#[derive(Debug)]
pub struct FnDef {
    pub name: String,
    pub params: usize,
    pub return_type: Type,
    pub body: Block,
    pub locals: usize,
}

#[derive(Debug)]
pub struct ViewDef {
    pub name: String,
    pub params: usize,
    pub body: Nodes,
    pub locals: usize,
}

#[derive(Debug)]
pub struct WindowDef {
    pub title: String,
    pub size: Size,
    /// Always a stack; several top-level elements are wrapped in an implicit column.
    pub content: Node,
    pub locals: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Int,
    Float,
    Bool,
    String,
    List(Rc<Type>),
    Struct(StructId),
    Void,
    /// The type of an expression that already has an error, so it reports nothing further.
    Error,
}

/// The built-in types and the names they are written with.
pub const PRIMITIVE_TYPES: &[(&str, Type)] = &[
    ("int", Type::Int),
    ("float", Type::Float),
    ("bool", Type::Bool),
    ("string", Type::String),
];

impl Type {
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error)
    }

    /// The built-in type written as `name`.
    #[must_use]
    pub fn primitive(name: &str) -> Option<Self> {
        PRIMITIVE_TYPES
            .iter()
            .find(|(primitive, _)| *primitive == name)
            .map(|(_, ty)| ty.clone())
    }

    /// How a built-in type is written, or `None` for any other type.
    #[must_use]
    pub fn primitive_name(&self) -> Option<&'static str> {
        PRIMITIVE_TYPES
            .iter()
            .find(|(_, ty)| ty == self)
            .map(|&(name, _)| name)
    }
}

/// States read by a dynamic part of the UI, sorted and without duplicates.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StateSet(Vec<StateId>);

impl StateSet {
    pub fn insert(&mut self, state: StateId) {
        if let Err(position) = self.0.binary_search(&state) {
            self.0.insert(position, state);
        }
    }

    pub fn extend(&mut self, other: &Self) {
        for state in &other.0 {
            self.insert(*state);
        }
    }

    #[must_use]
    pub fn contains(&self, state: StateId) -> bool {
        self.0.binary_search(&state).is_ok()
    }

    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        self.0.iter().any(|state| other.contains(*state))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = StateId> + '_ {
        self.0.iter().copied()
    }
}

pub type Block = Rc<[Stmt]>;
pub type Nodes = Rc<[Node]>;

#[derive(Debug)]
pub enum Stmt {
    Let(LocalId, Expr),
    Assign {
        place: Place,
        /// `Some` for compound assignment such as `+=`.
        op: Option<BinaryOp>,
        value: Expr,
    },
    If {
        branches: Vec<(Expr, Block)>,
        otherwise: Option<Block>,
    },
    For {
        local: LocalId,
        iterable: Iterable,
        body: Block,
    },
    Return(Option<Expr>),
    Expr(Expr),
}

/// What a `for` loops over. Looping over a place makes the loop variable an alias of each
/// element, so assigning through it changes the original list.
#[derive(Debug)]
pub enum Iterable {
    Place(Place),
    Value(Expr),
}

/// A location that can be assigned or bound: state or a local, then fields and indexes.
#[derive(Debug)]
pub struct Place {
    pub root: PlaceRoot,
    pub path: Vec<PlaceStep>,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceRoot {
    State(StateId),
    Local(LocalId),
}

#[derive(Debug)]
pub enum PlaceStep {
    Field(usize),
    Index(Expr),
}

#[derive(Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Debug)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(Rc<str>),
    Interpolate(Vec<Segment>),
    State(StateId),
    Local(LocalId),
    List(Vec<Expr>),
    Construct(StructId, Vec<Expr>),
    Field(Box<Expr>, usize),
    Index(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Negate(Box<Expr>),
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Call(FnId, Vec<Expr>),
    Len(Box<Expr>),
    IsEmpty(Box<Expr>),
    Push(Place, Box<Expr>),
    Remove(Place, Box<Expr>),
    /// Stands in for an expression with an error; never evaluated.
    Error,
}

#[derive(Debug)]
pub enum Segment {
    Text(Rc<str>),
    Value(Expr),
}

/// An expression the UI shows, with the states whose changes require showing it again.
#[derive(Debug)]
pub struct Dynamic {
    pub expr: Expr,
    pub deps: StateSet,
}

/// A two-way binding: the control shows the place's value and writes the user's edits to it.
#[derive(Debug)]
pub struct Binding {
    pub place: Place,
    pub deps: StateSet,
}

#[derive(Debug)]
pub enum Node {
    Stack {
        style: StackStyle,
        children: Nodes,
    },
    Label {
        text: Dynamic,
        style: TextStyle,
    },
    Button {
        title: Dynamic,
        style: ButtonStyle,
        action: Block,
    },
    TextField {
        value: Binding,
        placeholder: String,
    },
    Checkbox {
        checked: Binding,
        title: Option<Dynamic>,
    },
    Spacer,
    Divider,
    /// Rebuilt when `deps` change; the first branch whose condition holds is shown.
    If {
        branches: Vec<(Expr, Nodes)>,
        otherwise: Nodes,
        deps: StateSet,
    },
    /// Rebuilt when `deps` change; `body` is shown once per element.
    For {
        local: LocalId,
        iterable: Iterable,
        body: Nodes,
        deps: StateSet,
    },
    /// Rebuilt when `deps` change; the arguments fill the view's parameters.
    View {
        view: ViewId,
        args: Vec<Expr>,
        deps: StateSet,
    },
}
