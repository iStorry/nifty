//! Appearance options shared by every backend.

use crate::geometry::{Frame, Point, Size};

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

coded_enum! {
    /// What a stack is drawn on. Each surface is one native view behind the stack's
    /// children, using the platform's own materials and colors.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Surface {
        /// Nothing: the children sit on whatever is behind the stack.
        #[default]
        None,
        /// The side panel of a window, translucent where the platform supports it.
        Sidebar,
        /// The window's main pane, in the color content is read on.
        Content,
        /// A rounded, outlined panel that groups related content.
        Card,
    }
}

coded_enum! {
    /// What a window is drawn on, in each platform's own materials.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Backdrop {
        /// The platform's usual window: Mica in the frame on Windows 11, a plain window
        /// on macOS.
        #[default]
        Standard,
        /// See-through: Acrylic in the frame on Windows 11, and a translucent window that
        /// shows what's behind it on macOS.
        Translucent,
        /// Opaque everywhere, with no material.
        Solid,
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
    pub surface: Surface,
    /// Whether the stack takes the space its parent has left along the parent's axis.
    pub grow: bool,
    /// A fixed width in points, or [`StackStyle::FIT`] to fit the content.
    pub width: f32,
    /// The widest the stack may get in points, or [`StackStyle::FIT`] for no limit. A
    /// stack with a limit fills the width it is given, up to the limit.
    pub max_width: f32,
}

impl StackStyle {
    /// A `width` or `max_width` that sets no size; compiled apps store sizes as plain
    /// numbers, so this stands in for "none".
    pub const FIT: f32 = 0.0;

    #[must_use]
    pub const fn new(axis: Axis) -> Self {
        Self {
            axis,
            spacing: 0.0,
            padding: 0.0,
            align: Align::Stretch,
            justify: Justify::Start,
            surface: Surface::None,
            grow: false,
            width: Self::FIT,
            max_width: Self::FIT,
        }
    }

    /// The fixed width, if the stack has one.
    #[must_use]
    pub fn fixed_width(&self) -> Option<f32> {
        sets_size(self.width)
    }

    /// The width limit, if the stack has one.
    #[must_use]
    pub fn width_limit(&self) -> Option<f32> {
        sets_size(self.max_width)
    }
}

fn sets_size(points: f32) -> Option<f32> {
    (points > StackStyle::FIT).then_some(points)
}

/// The size an image is shown at: a fixed `width` or `height` in points, or
/// [`ImageSize::FIT`] for the image's own. Fixing only one keeps the image's shape.
/// `repr(C)` because compiled apps store it as is.
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct ImageSize {
    pub width: f32,
    pub height: f32,
}

impl ImageSize {
    /// A `width` or `height` that sets no size.
    pub const FIT: f32 = 0.0;

    /// The frame an image of `natural` size takes: the fixed sizes, with the other side
    /// following the image's shape, or the image's own size when neither is fixed. An
    /// image with no size, such as one that can't be read, takes the fixed sizes alone.
    #[must_use]
    pub fn frame(self, natural: Size) -> Size {
        let ratio = |along: f32, across: f32| if along > 0.0 { across / along } else { 0.0 };
        match (sets_size(self.width), sets_size(self.height)) {
            (Some(width), Some(height)) => Size::new(width, height),
            (Some(width), None) => Size::new(width, width * ratio(natural.width, natural.height)),
            (None, Some(height)) => {
                Size::new(height * ratio(natural.height, natural.width), height)
            }
            (None, None) => natural,
        }
    }

    /// Where an image of `natural` size is drawn in `frame`: as large as fits, keeping its
    /// shape, centred.
    #[must_use]
    pub fn fit(natural: Size, frame: Size) -> Frame {
        if natural.width <= 0.0 || natural.height <= 0.0 {
            return Frame::default();
        }
        let scale = (frame.width / natural.width).min(frame.height / natural.height);
        let size = Size::new(natural.width * scale, natural.height * scale);
        Frame {
            origin: Point {
                x: (frame.width - size.width) / 2.0,
                y: (frame.height - size.height) / 2.0,
            },
            size,
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
        /// The largest heading, for the name of what a window shows.
        LargeTitle,
    }
}

impl TextStyle {
    /// Captions are secondary text; every other style reads as primary.
    #[must_use]
    pub const fn default_color(self) -> TextColor {
        match self {
            Self::Caption => TextColor::Secondary,
            Self::Body | Self::Title | Self::Headline | Self::LargeTitle => TextColor::Primary,
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
        /// Borderless, for actions that shouldn't draw the eye, such as those in a row.
        Plain,
        /// Borderless and showing only its icon; the title becomes its tooltip and what
        /// screen readers say.
        Icon,
    }
}

coded_enum! {
    /// How a control that turns a `bool` on and off looks.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum ToggleStyle {
        /// A tick box before its title.
        #[default]
        Checkbox,
        /// A switch at the end of its title, as in settings.
        Switch,
    }
}

coded_enum! {
    /// Symbols from the platform's own icon set: SF Symbols on macOS, Segoe Fluent Icons
    /// on Windows. Nothing is bundled with the app.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum Icon {
        /// No icon.
        #[default]
        None,
        Plus,
        Trash,
        Star,
        StarFill,
        Check,
        CheckCircle,
        Circle,
        Home,
        Inbox,
        Archive,
        List,
        Calendar,
        Sun,
        Flag,
        Close,
        Gear,
        Search,
        Mail,
        Send,
        Folder,
        Clock,
        Tag,
        Person,
        Document,
        Download,
        Heart,
        Bell,
        Pencil,
        Share,
        Info,
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
    fn images_keep_their_shape_unless_both_sides_are_fixed() {
        let natural = Size::new(200.0, 100.0);
        let size = |width, height| ImageSize { width, height };
        assert_eq!(size(0.0, 0.0).frame(natural), natural);
        assert_eq!(size(50.0, 0.0).frame(natural), Size::new(50.0, 25.0));
        assert_eq!(size(0.0, 50.0).frame(natural), Size::new(100.0, 50.0));
        assert_eq!(size(30.0, 30.0).frame(natural), Size::new(30.0, 30.0));
        assert_eq!(size(30.0, 0.0).frame(Size::ZERO), Size::new(30.0, 0.0));
        assert_eq!(
            ImageSize::fit(natural, Size::new(50.0, 50.0)),
            Frame {
                origin: Point { x: 0.0, y: 12.5 },
                size: Size::new(50.0, 25.0),
            }
        );
    }

    #[test]
    fn every_style_round_trips_through_its_code() {
        round_trips(Axis::ALL, Axis::code, Axis::from_code);
        round_trips(Align::ALL, Align::code, Align::from_code);
        round_trips(Justify::ALL, Justify::code, Justify::from_code);
        round_trips(Surface::ALL, Surface::code, Surface::from_code);
        round_trips(Backdrop::ALL, Backdrop::code, Backdrop::from_code);
        round_trips(TextStyle::ALL, TextStyle::code, TextStyle::from_code);
        round_trips(TextColor::ALL, TextColor::code, TextColor::from_code);
        round_trips(TextAlign::ALL, TextAlign::code, TextAlign::from_code);
        round_trips(ButtonStyle::ALL, ButtonStyle::code, ButtonStyle::from_code);
        round_trips(ToggleStyle::ALL, ToggleStyle::code, ToggleStyle::from_code);
        round_trips(Icon::ALL, Icon::code, Icon::from_code);
    }
}
