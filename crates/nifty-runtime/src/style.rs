//! Appearance options shared by every backend.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Placement of children across a stack's axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

/// Distribution of children along a stack's axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StackStyle {
    pub axis: Axis,
    pub spacing: f32,
    pub padding: f32,
    pub align: Align,
    pub justify: Justify,
}

impl StackStyle {
    #[must_use]
    pub fn new(axis: Axis) -> Self {
        Self {
            axis,
            spacing: 0.0,
            padding: 0.0,
            align: Align::default(),
            justify: Justify::default(),
        }
    }
}

/// The platform's system text styles, so apps follow the system font and accessibility sizes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextStyle {
    #[default]
    Body,
    Title,
    Headline,
    Caption,
}

impl TextStyle {
    /// Captions are secondary text; every other style reads as primary.
    #[must_use]
    pub fn default_color(self) -> TextColor {
        match self {
            Self::Caption => TextColor::Secondary,
            Self::Body | Self::Title | Self::Headline => TextColor::Primary,
        }
    }
}

/// Semantic text colors, so text follows the system appearance and accent color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextColor {
    #[default]
    Primary,
    Secondary,
    Tertiary,
    Accent,
    Success,
    Warning,
    Danger,
}

/// Placement of text within its frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelStyle {
    pub font: TextStyle,
    pub color: TextColor,
    pub align: TextAlign,
    /// Whether the user can select and copy the text.
    pub selectable: bool,
}

impl LabelStyle {
    #[must_use]
    pub fn new(font: TextStyle) -> Self {
        Self {
            font,
            color: font.default_color(),
            align: TextAlign::default(),
            selectable: false,
        }
    }
}

impl Default for LabelStyle {
    fn default() -> Self {
        Self::new(TextStyle::default())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonStyle {
    #[default]
    Normal,
    /// The window's default action, drawn in the accent color and triggered by Return.
    Primary,
}
