//! The checked program: every name resolved to an id, every expression typed, and every
//! dynamic part of the UI annotated with the state it reads.

use std::rc::Rc;

pub use nifty_runtime::DialogKind;
use nifty_runtime::{Backdrop, ButtonStyle, Icon, LabelStyle, Size, StackStyle, ToggleStyle};
pub use nifty_runtime::{StateId, StateSet};
use nifty_syntax::Span;
pub use nifty_syntax::ast::BinaryOp;

macro_rules! id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub usize);
    };
}

id!(StructId);
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
    /// The type of every local, by id.
    pub locals: Vec<Type>,
}

#[derive(Debug)]
pub struct ViewDef {
    pub name: String,
    pub params: usize,
    pub body: Nodes,
    /// The type of every local, by id.
    pub locals: Vec<Type>,
}

/// A sidebar row, selected while `selection` holds `value`.
#[derive(Debug)]
pub struct NavItem {
    pub title: Rc<Dynamic>,
    pub value: Rc<Dynamic>,
    pub selection: Rc<Binding>,
    pub icon: Icon,
    pub badge: Option<Rc<Dynamic>>,
    /// Everything the row shows depends on: its texts and its selection.
    pub deps: StateSet,
}

#[derive(Debug)]
pub struct WindowDef {
    pub title: String,
    pub size: Size,
    pub backdrop: Backdrop,
    /// The stack holding its content: its one top-level `column` or `row`, or else an
    /// implicit column around its top-level elements, such as a `scroll` area.
    pub style: StackStyle,
    pub children: Nodes,
    /// The type of every local, by id.
    pub locals: Vec<Type>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

/// The built-in types; [`Type::primitive_name`] spells them.
pub const PRIMITIVE_TYPES: &[Type] = &[Type::Int, Type::Float, Type::Bool, Type::String];

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
            .find(|primitive| primitive.primitive_name() == Some(name))
            .cloned()
    }

    /// How a built-in type is written, or `None` for any other type.
    #[must_use]
    pub const fn primitive_name(&self) -> Option<&'static str> {
        match self {
            Self::Int => Some("int"),
            Self::Float => Some("float"),
            Self::Bool => Some("bool"),
            Self::String => Some("string"),
            Self::List(_) | Self::Struct(_) | Self::Void | Self::Error => None,
        }
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
    Download(Box<Download>),
    Dialog(Box<Dialog>),
    /// Stands in for an expression with an error; never evaluated.
    Error,
}

/// A `download(…)` call: fetches `url` into the folder `folder` on another thread, writing
/// its percentage done to `progress` and a description of where it stands to `status`.
#[derive(Debug)]
pub struct Download {
    pub url: Expr,
    pub folder: Expr,
    pub progress: Place,
    pub status: Place,
}

/// A `pick_…(…)` call: once the event finishes, shows a file dialog, and writes the path
/// or paths the user picks to `answer`. Cancelling leaves `answer` as it was. Settings the
/// call leaves out are empty: `""`, or `[]` for `types`.
#[derive(Debug)]
pub struct Dialog {
    pub kind: DialogKind,
    pub answer: Place,
    pub title: Expr,
    pub types: Expr,
    pub folder: Expr,
    pub name: Expr,
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

/// UI parts are shared, so whatever runs the program can keep them for as long as it shows
/// them.
#[derive(Debug)]
pub enum Node {
    /// A `column` or `row`, or, when it `scrolls`, a `scroll` area.
    Stack {
        style: StackStyle,
        children: Nodes,
        scrolls: bool,
    },
    Label {
        text: Rc<Dynamic>,
        style: LabelStyle,
    },
    /// Shown as selected while `selected` holds, when it has one.
    Button {
        title: Rc<Dynamic>,
        style: ButtonStyle,
        icon: Icon,
        selected: Option<Rc<Dynamic>>,
        action: Block,
    },
    /// Edits text on one line, or wraps it over `lines` lines and scrolls it.
    TextField {
        value: Rc<Binding>,
        placeholder: String,
        lines: u32,
    },
    /// A `checkbox`, or a `switch`, by its `style`.
    Checkbox {
        checked: Rc<Binding>,
        title: Option<Rc<Dynamic>>,
        style: ToggleStyle,
    },
    /// Picks a whole number from `min` to `max`.
    Slider {
        value: Rc<Binding>,
        min: i64,
        max: i64,
    },
    /// Picks one of `options` into `selection`.
    Picker {
        selection: Rc<Binding>,
        options: Rc<Dynamic>,
        /// What it shows depends on: its options and its selection.
        deps: StateSet,
    },
    NavItem(NavItem),
    /// A progress bar showing `value` percent done, or that work is under way when it has
    /// no value.
    Progress {
        value: Option<Rc<Dynamic>>,
    },
    Spacer,
    Divider,
    /// Rebuilt when `deps` change; the first branch whose condition holds is shown.
    If {
        branches: Vec<(Rc<Expr>, Nodes)>,
        otherwise: Nodes,
        deps: StateSet,
    },
    /// Rebuilt when `deps` change; `body` is shown once per element.
    For {
        local: LocalId,
        iterable: Rc<Iterable>,
        body: Nodes,
        deps: StateSet,
    },
    /// Rebuilt when `deps` change; the arguments fill the view's parameters.
    View {
        view: ViewId,
        args: Rc<[Expr]>,
        deps: StateSet,
    },
}
