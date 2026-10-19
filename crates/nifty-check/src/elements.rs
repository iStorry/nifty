//! The built-in UI elements and the arguments each one takes, for tools such as the
//! language server. The checker accepts exactly these; a test keeps the two in step.

/// A built-in element such as `label` or `row`.
#[derive(Debug)]
pub struct Element {
    pub name: &'static str,
    pub params: &'static [Param],
    pub block: TrailingBlock,
    pub summary: &'static str,
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
    /// A size in points, written as a number.
    Points,
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
    pub fn describe(self) -> &'static str {
        match self {
            Self::Text => "string",
            Self::FixedText => "fixed string",
            Self::Points => "number",
            Self::Flag => "bool",
            Self::Binding(BoundType::String) => "$string",
            Self::Binding(BoundType::Bool) => "$bool",
            Self::Option(Options::Alignment) => "alignment",
            Self::Option(Options::Justification) => "justification",
            Self::Option(Options::TextStyle) => "text style",
            Self::Option(Options::TextColor) => "text color",
            Self::Option(Options::TextAlignment) => "text alignment",
            Self::Option(Options::ButtonStyle) => "button style",
        }
    }
}

const STACK_PARAMS: &[Param] = &[
    named("spacing", ParamKind::Points),
    named("padding", ParamKind::Points),
    named("align", ParamKind::Option(Options::Alignment)),
    named("justify", ParamKind::Option(Options::Justification)),
];

pub const ELEMENTS: &[Element] = &[
    Element {
        name: "window",
        params: &[
            positional("title", ParamKind::FixedText),
            named("width", ParamKind::Points),
            named("height", ParamKind::Points),
        ],
        block: TrailingBlock::Children,
        summary: "A window; only at the top level.",
    },
    Element {
        name: "column",
        params: STACK_PARAMS,
        block: TrailingBlock::Children,
        summary: "Stacks its children top to bottom.",
    },
    Element {
        name: "row",
        params: STACK_PARAMS,
        block: TrailingBlock::Children,
        summary: "Stacks its children left to right.",
    },
    Element {
        name: "label",
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
        params: &[
            positional("title", ParamKind::Text),
            named("style", ParamKind::Option(Options::ButtonStyle)),
        ],
        block: TrailingBlock::Action,
        summary: "Runs its block when clicked.",
    },
    Element {
        name: "text_field",
        params: &[
            positional("value", ParamKind::Binding(BoundType::String)),
            named("placeholder", ParamKind::FixedText),
        ],
        block: TrailingBlock::None,
        summary: "Edits bound text.",
    },
    Element {
        name: "checkbox",
        params: &[
            Param {
                required: true,
                ..named("checked", ParamKind::Binding(BoundType::Bool))
            },
            named("title", ParamKind::Text),
        ],
        block: TrailingBlock::None,
        summary: "Toggles a bound `bool`.",
    },
    Element {
        name: "spacer",
        params: &[],
        block: TrailingBlock::None,
        summary: "Takes the remaining space.",
    },
    Element {
        name: "divider",
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

const fn positional(name: &'static str, kind: ParamKind) -> Param {
    Param {
        name,
        positional: true,
        required: true,
        kind,
    }
}

const fn named(name: &'static str, kind: ParamKind) -> Param {
    Param {
        name,
        positional: false,
        required: false,
        kind,
    }
}
