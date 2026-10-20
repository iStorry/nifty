//! Appearance options shared by every backend.

/// Declares a fieldless enum whose variants have stable numeric codes, numbered in
/// declaration order, which is how compiled apps store styles.
macro_rules! coded_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident {
            $($(#[$variant_meta:meta])* $variant:ident,)+
        }
    ) => {
        $(#[$meta])*
        #[repr(u8)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
        }

        impl $name {
            pub const ALL: &[Self] = &[$(Self::$variant,)+];

            #[must_use]
            pub fn code(self) -> u8 {
                self as u8
            }

            #[must_use]
            pub fn from_code(code: u8) -> Option<Self> {
                Self::ALL.get(usize::from(code)).copied()
            }
        }
    };
}

coded_enum! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Axis {
        Horizontal,
        Vertical,
    }
}

coded_enum! {
    /// Placement of children across a stack's axis.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Align {
        #[default]
        Stretch,
        Start,
        Center,
        End,
    }
}

coded_enum! {
    /// Distribution of children along a stack's axis.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Justify {
        #[default]
        Start,
        Center,
        End,
        SpaceBetween,
    }
}

/// `repr(C)` because compiled apps store it as is.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct StackStyle {
    pub axis: Axis,
    pub spacing: f32,
    pub padding: f32,
    pub align: Align,
    pub justify: Justify,
}

impl StackStyle {
    #[must_use]
    pub const fn new(axis: Axis) -> Self {
        Self {
            axis,
            spacing: 0.0,
            padding: 0.0,
            align: Align::Stretch,
            justify: Justify::Start,
        }
    }
}

coded_enum! {
    /// The platform's system text styles, so apps follow the system font and accessibility sizes.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextStyle {
        #[default]
        Body,
        Title,
        Headline,
        Caption,
    }
}

impl TextStyle {
    /// Captions are secondary text; every other style reads as primary.
    #[must_use]
    pub const fn default_color(self) -> TextColor {
        match self {
            Self::Caption => TextColor::Secondary,
            Self::Body | Self::Title | Self::Headline => TextColor::Primary,
        }
    }
}

coded_enum! {
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
}

coded_enum! {
    /// Placement of text within its frame.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum TextAlign {
        #[default]
        Start,
        Center,
        End,
    }
}

/// `repr(C)` because compiled apps store it as is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct LabelStyle {
    pub font: TextStyle,
    pub color: TextColor,
    pub align: TextAlign,
    /// Whether the user can select and copy the text.
    pub selectable: bool,
}

impl LabelStyle {
    #[must_use]
    pub const fn new(font: TextStyle) -> Self {
        Self {
            font,
            color: font.default_color(),
            align: TextAlign::Start,
            selectable: false,
        }
    }
}

impl Default for LabelStyle {
    fn default() -> Self {
        Self::new(TextStyle::default())
    }
}

coded_enum! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ButtonStyle {
        #[default]
        Normal,
        /// The window's default action, drawn in the accent color and triggered by Return.
        Primary,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trips<T: Copy + PartialEq + std::fmt::Debug>(
        all: &[T],
        code: fn(T) -> u8,
        from_code: fn(u8) -> Option<T>,
    ) {
        for (index, variant) in all.iter().enumerate() {
            assert_eq!(usize::from(code(*variant)), index);
            assert_eq!(from_code(code(*variant)), Some(*variant));
        }
        let past_the_end = u8::try_from(all.len()).expect("few variants");
        assert_eq!(from_code(past_the_end), None);
    }

    #[test]
    fn every_style_round_trips_through_its_code() {
        round_trips(Axis::ALL, Axis::code, Axis::from_code);
        round_trips(Align::ALL, Align::code, Align::from_code);
        round_trips(Justify::ALL, Justify::code, Justify::from_code);
        round_trips(TextStyle::ALL, TextStyle::code, TextStyle::from_code);
        round_trips(TextColor::ALL, TextColor::code, TextColor::from_code);
        round_trips(TextAlign::ALL, TextAlign::code, TextAlign::from_code);
        round_trips(ButtonStyle::ALL, ButtonStyle::code, ButtonStyle::from_code);
    }
}
