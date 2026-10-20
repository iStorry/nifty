//! The built-in UI elements and the arguments each one takes. The checker dispatches on
//! [`Element::kind`], and tools such as the language server read the same table.

use std::borrow::Cow;
use std::rc::Rc;

use crate::hir::Type;

/// A built-in element such as `label` or `row`.
#[derive(Debug)]
pub struct Element {
    pub name: &'static str,
    pub kind: ElementKind,
    pub params: &'static [Param],
    pub block: TrailingBlock,
    pub summary: &'static str,
}

/// Which element a table entry is, for the checker to dispatch on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementKind {
    Window,
    Column,
    Row,
    Scroll,
    Label,
    Button,
    TextField,
    Checkbox,
    Switch,
    Slider,
    Picker,
    NavItem,
    Progress,
    Spacer,
    Divider,
}

#[derive(Debug)]
pub struct Param {
    pub name: &'static str,
    /// Positional parameters come first and have no label; the rest are always named.
    pub positional: bool,
    pub required: bool,
    pub kind: ParamKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParamKind {
    /// Text that may change, such as `"{count} left"`.
    Text,
    /// Text written literally, without interpolation.
    FixedText,
    /// An `int` that may change, such as `item.percent`.
    Number,
    /// A `bool` that may change, such as `filter == "all"`.
    Condition,
    /// A list of text, such as `["png", "jpg"]`.
    TextList,
    /// A size in points, written as a number.
    Points,
    /// A whole number written literally, such as `-10`.
    FixedNumber,
    /// `true` or `false`, written literally.
    Flag,
    /// A two-way binding such as `$draft`.
    Binding(BoundType),
    /// One of a fixed set of `.name` values.
    Option(Options),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundType {
    String,
    Bool,
    Int,
    /// A list of strings, `[string]`.
    Strings,
}

impl BoundType {
    #[must_use]
    pub fn ty(self) -> Type {
        match self {
            Self::String => Type::String,
            Self::Bool => Type::Bool,
            Self::Int => Type::Int,
            Self::Strings => Type::List(Rc::new(Type::String)),
        }
    }

    /// How the bound type is written, such as `int` or `[string]`.
    #[must_use]
    pub fn name(self) -> Cow<'static, str> {
        match self.ty() {
            Type::List(item) => Cow::Owned(format!("[{}]", primitive_name(&item))),
            primitive => Cow::Borrowed(primitive_name(&primitive)),
        }
    }
}

fn primitive_name(ty: &Type) -> &'static str {
    match ty.primitive_name() {
        Some(name) => name,
        None => unreachable!("bindings are to primitive types or lists of them"),
    }
}

/// A set of values written as `.name`; [`Options::names`] lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Options {
    Alignment,
    Justification,
    TextStyle,
    TextColor,
    TextAlignment,
    ButtonStyle,
    Surface,
    Icon,
    Backdrop,
}

/// What the `{ … }` after an element holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrailingBlock {
    None,
    /// Child elements.
    Children,
    /// Statements to run when the element is used.
    Action,
}

impl ParamKind {
    /// How the expected value reads in signatures, such as `number` or `$bool`.
    #[must_use]
    pub fn describe(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::Text => "string",
            Self::FixedText => "fixed string",
            Self::Number => "int",
            Self::TextList => "[string]",
            Self::Points => "number",
            Self::FixedNumber => "fixed int",
            Self::Flag | Self::Condition => "bool",
            Self::Option(Options::Alignment) => "alignment",
            Self::Option(Options::Justification) => "justification",
            Self::Option(Options::TextStyle) => "text style",
            Self::Option(Options::TextColor) => "text color",
            Self::Option(Options::TextAlignment) => "text alignment",
            Self::Option(Options::ButtonStyle) => "button style",
            Self::Option(Options::Surface) => "surface",
            Self::Option(Options::Icon) => "icon",
            Self::Option(Options::Backdrop) => "backdrop",
            Self::Binding(bound) => return Cow::Owned(format!("${}", bound.name())),
        })
    }
}

const STACK_PARAMS: &[Param] = &[
    named("spacing", ParamKind::Points),
    named("padding", ParamKind::Points),
    named("align", ParamKind::Option(Options::Alignment)),
    named("justify", ParamKind::Option(Options::Justification)),
    named("surface", ParamKind::Option(Options::Surface)),
    named("width", ParamKind::Points),
    named("max_width", ParamKind::Points),
    named("grow", ParamKind::Flag),
];

/// A scroll area always takes the space its parent has left, so it takes every stack
/// setting but `grow`, which comes last.
const SCROLL_PARAMS: &[Param] = match STACK_PARAMS.split_last() {
    Some((_grow, rest)) => rest,
    None => STACK_PARAMS,
};

pub const ELEMENTS: &[Element] = &[
    Element {
        name: WINDOW,
        kind: ElementKind::Window,
        params: &[
            positional("title", ParamKind::FixedText),
            named("width", ParamKind::Points),
            named("height", ParamKind::Points),
            named("backdrop", ParamKind::Option(Options::Backdrop)),
        ],
        block: TrailingBlock::Children,
        summary: "A window; only at the top level.",
    },
    Element {
        name: "column",
        kind: ElementKind::Column,
        params: STACK_PARAMS,
        block: TrailingBlock::Children,
        summary: "Stacks its children top to bottom.",
    },
    Element {
        name: "row",
        kind: ElementKind::Row,
        params: STACK_PARAMS,
        block: TrailingBlock::Children,
        summary: "Stacks its children left to right.",
    },
    Element {
        name: "scroll",
        kind: ElementKind::Scroll,
        params: SCROLL_PARAMS,
        block: TrailingBlock::Children,
        summary: "Stacks its children top to bottom in the space left, scrolling them when they're too tall.",
    },
    Element {
        name: "label",
        kind: ElementKind::Label,
        params: &[
            positional("text", ParamKind::Text),
            named("style", ParamKind::Option(Options::TextStyle)),
            named("color", ParamKind::Option(Options::TextColor)),
            named("align", ParamKind::Option(Options::TextAlignment)),
            named("selectable", ParamKind::Flag),
        ],
        block: TrailingBlock::None,
        summary: "Shows text.",
    },
    Element {
        name: "button",
        kind: ElementKind::Button,
        params: &[
            positional("title", ParamKind::Text),
            named("style", ParamKind::Option(Options::ButtonStyle)),
            named("icon", ParamKind::Option(Options::Icon)),
            named("selected", ParamKind::Condition),
        ],
        block: TrailingBlock::Action,
        summary: "Runs its block when clicked.",
    },
    Element {
        name: "text_field",
        kind: ElementKind::TextField,
        params: &[
            positional("value", ParamKind::Binding(BoundType::String)),
            named("placeholder", ParamKind::FixedText),
            named("lines", ParamKind::FixedNumber),
        ],
        block: TrailingBlock::None,
        summary: "Edits bound text, on one line or several.",
    },
    Element {
        name: "checkbox",
        kind: ElementKind::Checkbox,
        params: &[
            required("checked", ParamKind::Binding(BoundType::Bool)),
            named("title", ParamKind::Text),
        ],
        block: TrailingBlock::None,
        summary: "Toggles a bound `bool`.",
    },
    Element {
        name: "switch",
        kind: ElementKind::Switch,
        params: &[
            required("on", ParamKind::Binding(BoundType::Bool)),
            named("title", ParamKind::Text),
        ],
        block: TrailingBlock::None,
        summary: "Turns a bound `bool` on and off, with a switch at the end of its title.",
    },
    Element {
        name: "slider",
        kind: ElementKind::Slider,
        params: &[
            positional("value", ParamKind::Binding(BoundType::Int)),
            named("min", ParamKind::FixedNumber),
            named("max", ParamKind::FixedNumber),
        ],
        block: TrailingBlock::None,
        summary: "Picks a bound `int` between `min` and `max` by dragging.",
    },
    Element {
        name: "picker",
        kind: ElementKind::Picker,
        params: &[
            positional("selection", ParamKind::Binding(BoundType::String)),
            required("options", ParamKind::TextList),
        ],
        block: TrailingBlock::None,
        summary: "Picks one of a list of strings from a pop-up menu, into a bound `string`.",
    },
    Element {
        name: "nav_item",
        kind: ElementKind::NavItem,
        params: &[
            positional("title", ParamKind::Text),
            required("value", ParamKind::Text),
            required("selection", ParamKind::Binding(BoundType::String)),
            named("icon", ParamKind::Option(Options::Icon)),
            named("badge", ParamKind::Text),
        ],
        block: TrailingBlock::None,
        summary: "A sidebar row, selected while its binding holds its value.",
    },
    Element {
        name: "progress",
        kind: ElementKind::Progress,
        params: &[named("value", ParamKind::Number)],
        block: TrailingBlock::None,
        summary: "Shows how far along work is, as a percentage; without `value`, that it's busy.",
    },
    Element {
        name: "spacer",
        kind: ElementKind::Spacer,
        params: &[],
        block: TrailingBlock::None,
        summary: "Takes the remaining space.",
    },
    Element {
        name: "divider",
        kind: ElementKind::Divider,
        params: &[],
        block: TrailingBlock::None,
        summary: "A line between elements.",
    },
];

/// The element called `name`, if it is built in.
#[must_use]
pub fn element(name: &str) -> Option<&'static Element> {
    ELEMENTS.iter().find(|element| element.name == name)
}

/// The element that declares a window, which only appears at the top level.
pub const WINDOW: &str = "window";

pub(crate) const fn positional(name: &'static str, kind: ParamKind) -> Param {
    Param {
        name,
        positional: true,
        required: true,
        kind,
    }
}

pub(crate) const fn named(name: &'static str, kind: ParamKind) -> Param {
    Param {
        name,
        positional: false,
        required: false,
        kind,
    }
}

pub(crate) const fn required(name: &'static str, kind: ParamKind) -> Param {
    Param {
        required: true,
        ..named(name, kind)
    }
}
