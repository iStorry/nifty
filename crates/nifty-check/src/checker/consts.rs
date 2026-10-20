//! Element arguments that must be written literally: layout numbers, fixed text, flags and
//! styles.

use nifty_runtime::{
    Align, Backdrop, ButtonStyle, Icon, Justify, Surface, TextAlign, TextColor, TextStyle,
};
use nifty_syntax::ast::{self, ExprKind as Ast, StringPart, UnaryOp};

use super::Checker;
use crate::elements::Options;

pub(super) const ALIGNMENTS: &[(&str, Align)] = &[
    ("stretch", Align::Stretch),
    ("start", Align::Start),
    ("center", Align::Center),
    ("end", Align::End),
];

pub(super) const JUSTIFICATIONS: &[(&str, Justify)] = &[
    ("start", Justify::Start),
    ("center", Justify::Center),
    ("end", Justify::End),
    ("space_between", Justify::SpaceBetween),
];

pub(super) const SURFACES: &[(&str, Surface)] = &[
    ("none", Surface::None),
    ("sidebar", Surface::Sidebar),
    ("content", Surface::Content),
    ("card", Surface::Card),
];

pub(super) const BACKDROPS: &[(&str, Backdrop)] = &[
    ("standard", Backdrop::Standard),
    ("translucent", Backdrop::Translucent),
    ("solid", Backdrop::Solid),
];

pub(super) const TEXT_STYLES: &[(&str, TextStyle)] = &[
    ("body", TextStyle::Body),
    ("title", TextStyle::Title),
    ("headline", TextStyle::Headline),
    ("caption", TextStyle::Caption),
    ("large_title", TextStyle::LargeTitle),
];

pub(super) const TEXT_COLORS: &[(&str, TextColor)] = &[
    ("primary", TextColor::Primary),
    ("secondary", TextColor::Secondary),
    ("tertiary", TextColor::Tertiary),
    ("accent", TextColor::Accent),
    ("success", TextColor::Success),
    ("warning", TextColor::Warning),
    ("danger", TextColor::Danger),
];

pub(super) const TEXT_ALIGNMENTS: &[(&str, TextAlign)] = &[
    ("start", TextAlign::Start),
    ("center", TextAlign::Center),
    ("end", TextAlign::End),
];

pub(super) const BUTTON_STYLES: &[(&str, ButtonStyle)] = &[
    ("normal", ButtonStyle::Normal),
    ("primary", ButtonStyle::Primary),
    ("plain", ButtonStyle::Plain),
    ("icon", ButtonStyle::Icon),
];

/// Every icon but `Icon::None`, which is what leaving `icon:` out means.
pub(super) const ICONS: &[(&str, Icon)] = &[
    ("plus", Icon::Plus),
    ("trash", Icon::Trash),
    ("star", Icon::Star),
    ("star_fill", Icon::StarFill),
    ("check", Icon::Check),
    ("check_circle", Icon::CheckCircle),
    ("circle", Icon::Circle),
    ("home", Icon::Home),
    ("inbox", Icon::Inbox),
    ("archive", Icon::Archive),
    ("list", Icon::List),
    ("calendar", Icon::Calendar),
    ("sun", Icon::Sun),
    ("flag", Icon::Flag),
    ("close", Icon::Close),
    ("gear", Icon::Gear),
    ("search", Icon::Search),
    ("mail", Icon::Mail),
    ("send", Icon::Send),
    ("folder", Icon::Folder),
    ("clock", Icon::Clock),
    ("tag", Icon::Tag),
    ("person", Icon::Person),
    ("document", Icon::Document),
    ("download", Icon::Download),
    ("heart", Icon::Heart),
    ("bell", Icon::Bell),
    ("pencil", Icon::Pencil),
    ("share", Icon::Share),
    ("info", Icon::Info),
];

impl Options {
    /// The values in this set, written after a `.`.
    #[must_use]
    pub fn names(self) -> Vec<&'static str> {
        match self {
            Self::Alignment => names(ALIGNMENTS),
            Self::Justification => names(JUSTIFICATIONS),
            Self::TextStyle => names(TEXT_STYLES),
            Self::TextColor => names(TEXT_COLORS),
            Self::TextAlignment => names(TEXT_ALIGNMENTS),
            Self::ButtonStyle => names(BUTTON_STYLES),
            Self::Surface => names(SURFACES),
            Self::Icon => names(ICONS),
            Self::Backdrop => names(BACKDROPS),
        }
    }
}

fn names<T>(options: &[(&'static str, T)]) -> Vec<&'static str> {
    options.iter().map(|&(name, _)| name).collect()
}

/// Layout sizes are points on screen; anything larger is a mistake.
const MAX_POINTS: f64 = 10_000.0;
/// Whole numbers written into settings, such as a slider's range, stay this close to zero,
/// so every platform's controls hold them exactly.
pub(super) const MAX_FIXED_NUMBER: i64 = 1_000_000_000;

impl Checker {
    /// A size in points, written as a number literal.
    pub(super) fn const_points(&mut self, expr: &ast::Expr) -> Option<f32> {
        let value = match expr.kind {
            Ast::Int(value) => u16::try_from(value).ok().map(f64::from),
            Ast::Float(value) => Some(value),
            _ => {
                self.error("this must be a number such as `12`", expr.span);
                return None;
            }
        };
        if let Some(value) = value.filter(|value| (0.0..=MAX_POINTS).contains(value)) {
            return Some(points(value));
        }
        self.error(
            format!("this must be between 0 and {MAX_POINTS}"),
            expr.span,
        );
        None
    }

    /// A whole number written literally, such as `10` or `-10`.
    pub(super) fn const_number(&mut self, expr: &ast::Expr) -> Option<i64> {
        let value = match &expr.kind {
            Ast::Int(value) => Some(*value),
            Ast::Unary {
                op: UnaryOp::Neg,
                operand,
            } => match operand.kind {
                Ast::Int(value) => Some(-value),
                _ => None,
            },
            _ => None,
        };
        let Some(value) = value else {
            self.error("this must be a whole number such as `10`", expr.span);
            return None;
        };
        if (-MAX_FIXED_NUMBER..=MAX_FIXED_NUMBER).contains(&value) {
            return Some(value);
        }
        self.error(
            format!("this must be between -{MAX_FIXED_NUMBER} and {MAX_FIXED_NUMBER}"),
            expr.span,
        );
        None
    }

    /// Text written as a plain string literal, without interpolation.
    pub(super) fn const_string(&mut self, expr: &ast::Expr) -> Option<String> {
        if let Ast::String(parts) = &expr.kind {
            let text: Option<String> = parts
                .iter()
                .map(|part| match part {
                    StringPart::Text(text) => Some(text.as_str()),
                    StringPart::Expr(_) => None,
                })
                .collect();
            if text.is_some() {
                return text;
            }
        }
        self.error("this must be fixed text such as \"Title\"", expr.span);
        None
    }

    /// `true` or `false`, written literally.
    pub(super) fn const_bool(&mut self, expr: &ast::Expr) -> Option<bool> {
        if let Ast::Bool(value) = expr.kind {
            return Some(value);
        }
        self.error("this must be `true` or `false`", expr.span);
        None
    }

    /// One of `options`, written as `.name`.
    pub(super) fn const_member<T: Copy>(
        &mut self,
        expr: &ast::Expr,
        options: &[(&str, T)],
    ) -> Option<T> {
        if let Ast::ImplicitMember(name) = &expr.kind
            && let Some((_, value)) = options.iter().find(|(option, _)| *option == name.name)
        {
            return Some(*value);
        }
        let names: Vec<String> = options
            .iter()
            .map(|(name, _)| format!("`.{name}`"))
            .collect();
        self.error(format!("expected one of {}", names.join(", ")), expr.span);
        None
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "range-checked to at most MAX_POINTS, where f32 is far finer than a point"
)]
fn points(value: f64) -> f32 {
    value as f32
}
