//! The declarative view tree an app describes; the runtime turns it into native widgets.

use std::fmt;

use crate::backend::ClickHandler;
use crate::geometry::Size;

const DEFAULT_WINDOW_SIZE: Size = Size::new(480.0, 320.0);

/// Text that is either fixed or recomputed whenever the signals it reads change.
pub enum Text {
    Static(String),
    Dynamic(Box<dyn Fn() -> String>),
}

impl Text {
    pub(crate) fn static_value(&self) -> &str {
        match self {
            Self::Static(text) => text,
            Self::Dynamic(_) => "",
        }
    }
}

impl fmt::Debug for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Static(text) => f.debug_tuple("Static").field(text).finish(),
            Self::Dynamic(_) => f.write_str("Dynamic"),
        }
    }
}

pub trait IntoText {
    fn into_text(self) -> Text;
}

impl IntoText for &str {
    fn into_text(self) -> Text {
        Text::Static(self.to_owned())
    }
}

impl IntoText for String {
    fn into_text(self) -> Text {
        Text::Static(self)
    }
}

impl<F: Fn() -> String + 'static> IntoText for F {
    fn into_text(self) -> Text {
        Text::Dynamic(Box::new(self))
    }
}

#[derive(Debug)]
pub enum View {
    Label(Label),
    Button(Button),
    Stack(Stack),
}

#[derive(Debug)]
pub struct Label {
    pub(crate) text: Text,
}

pub fn label(text: impl IntoText) -> Label {
    Label {
        text: text.into_text(),
    }
}

pub struct Button {
    pub(crate) title: Text,
    pub(crate) on_click: ClickHandler,
}

pub fn button(title: impl IntoText, on_click: impl Fn() + 'static) -> Button {
    Button {
        title: title.into_text(),
        on_click: Box::new(on_click),
    }
}

impl fmt::Debug for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Button")
            .field("title", &self.title)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Placement of children across the stack's axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

/// Distribution of children along the stack's axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Debug)]
pub struct Stack {
    pub(crate) axis: Axis,
    pub(crate) spacing: f32,
    pub(crate) padding: f32,
    pub(crate) align: Align,
    pub(crate) justify: Justify,
    pub(crate) children: Vec<View>,
}

#[must_use]
pub fn column() -> Stack {
    Stack::new(Axis::Vertical)
}

#[must_use]
pub fn row() -> Stack {
    Stack::new(Axis::Horizontal)
}

impl Stack {
    fn new(axis: Axis) -> Self {
        Self {
            axis,
            spacing: 0.0,
            padding: 0.0,
            align: Align::default(),
            justify: Justify::default(),
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    #[must_use]
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    #[must_use]
    pub fn justify(mut self, justify: Justify) -> Self {
        self.justify = justify;
        self
    }

    #[must_use]
    pub fn child(mut self, child: impl Into<View>) -> Self {
        self.children.push(child.into());
        self
    }
}

impl From<Label> for View {
    fn from(label: Label) -> Self {
        Self::Label(label)
    }
}

impl From<Button> for View {
    fn from(button: Button) -> Self {
        Self::Button(button)
    }
}

impl From<Stack> for View {
    fn from(stack: Stack) -> Self {
        Self::Stack(stack)
    }
}

#[derive(Debug)]
pub struct Window {
    pub(crate) title: String,
    pub(crate) size: Size,
    pub(crate) content: View,
}

impl Window {
    pub fn new(title: impl Into<String>, content: impl Into<View>) -> Self {
        Self {
            title: title.into(),
            size: DEFAULT_WINDOW_SIZE,
            content: content.into(),
        }
    }

    #[must_use]
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.size = Size::new(width, height);
        self
    }
}
