//! Push buttons and checkboxes painted in the window's palette. They stay real `BUTTON`
//! controls, so keyboard, focus and accessibility behave as usual; only their pixels come
//! from Nifty, because Win32's themes for them have no documented dark mode.

use windows::Win32::Foundation::{COLORREF, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreatePen, DC_BRUSH, DC_PEN, DRAW_TEXT_FORMAT, DT_CALCRECT, DT_CENTER, DT_END_ELLIPSIS,
    DT_HIDEPREFIX, DT_LEFT, DT_NOPREFIX, DT_RIGHT, DT_SINGLELINE, DT_VCENTER, DrawFocusRect,
    DrawTextW, FillRect, GetStockObject, HBRUSH, HDC, HFONT, NULL_BRUSH, PS_SOLID, Polyline,
    RoundRect, SetBkMode, SetDCBrushColor, SetDCPenColor, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Controls::{
    CDIS_FOCUS, CDIS_HOT, CDIS_SELECTED, CDIS_SHOWKEYBOARDCUES, NMCUSTOMDRAW_DRAW_STATE_FLAGS,
};
use windows::core::Result;

use nifty_runtime::{ButtonStyle, Icon, Size, Surface, TextColor};

use crate::convert::Scale;
use crate::gdi::{Owned, Selected};
use crate::icons::glyph;
use crate::palette::{Fill, Palette, blend};

/// Fluent's control corner radius and focus ring width, in DIPs.
const CORNER_RADIUS: f32 = 4.0;
/// Fluent's corner radius for cards and other panels, in DIPs.
const CARD_CORNER_RADIUS: f32 = 8.0;
/// The Windows guideline's smallest push button, in DIPs.
const MIN_BUTTON_SIZE: Size = Size::new(75.0, 23.0);
/// An icon's square, and the space between it and the text beside it, in DIPs.
const ICON_SIZE: f32 = 16.0;
const ICON_GAP: f32 = 8.0;
/// The side of a button that shows only its icon.
const ICON_BUTTON_SIDE: f32 = 28.0;
/// A nav item's height, the space inside its ends, and its selection mark, in DIPs.
const NAV_ITEM_HEIGHT: f32 = 32.0;
const NAV_ITEM_INSET: f32 = 12.0;
const SELECTION_MARK: Size = Size::new(3.0, 16.0);
/// How strongly the text color tints a borderless control's backdrop, in percent: while
/// the pointer is over it, while pressed, and while a nav item is selected.
const SUBTLE_HOT: u32 = 6;
const SUBTLE_PRESSED: u32 = 4;
const SUBTLE_SELECTED: u32 = 9;
const FOCUS_RING: f32 = 2.0;
/// The side of a checkbox's box, and the space between it and its title, in DIPs.
const CHECKBOX_BOX: f32 = 16.0;
const CHECKBOX_GAP: f32 = 8.0;
/// Room for the focus rectangle drawn around a checkbox's title.
const FOCUS_MARGIN: f32 = 2.0;
/// The check mark's stroke, and its three points as fractions of the box.
const CHECK_STROKE: f32 = 1.5;
const CHECK_MARK: [(f32, f32); 3] = [(0.22, 0.52), (0.42, 0.72), (0.78, 0.30)];

/// The text format checkbox titles are drawn and measured with; `&&` shows one `&`.
pub(crate) const CHECKBOX_TEXT_FORMAT: DRAW_TEXT_FORMAT = DT_SINGLELINE;

/// What the control looks like right now, from its custom-draw notification.
#[derive(Clone, Copy, Debug)]
pub(crate) struct State(pub(crate) NMCUSTOMDRAW_DRAW_STATE_FLAGS);

impl State {
    fn fill_color(self, fill: &Fill) -> COLORREF {
        if self.0.contains(CDIS_SELECTED) {
            fill.pressed
        } else if self.0.contains(CDIS_HOT) {
            fill.hot
        } else {
            fill.rest
        }
    }

    /// Focus shows only while the user works with the keyboard, as in native controls.
    fn shows_focus(self) -> bool {
        self.0.contains(CDIS_FOCUS) && self.0.contains(CDIS_SHOWKEYBOARDCUES)
    }

    /// Mnemonic underlines likewise show only with keyboard cues.
    fn text_format(self) -> DRAW_TEXT_FORMAT {
        if self.0.contains(CDIS_SHOWKEYBOARDCUES) {
            DRAW_TEXT_FORMAT(0)
        } else {
            DT_HIDEPREFIX
        }
    }
}

/// One paint of one control: its device context, area and title.
pub(crate) struct Canvas<'a> {
    pub(crate) dc: HDC,
    pub(crate) rect: RECT,
    /// What the control sits on: the window's background or a surface's.
    pub(crate) backdrop: COLORREF,
    pub(crate) font: HFONT,
    pub(crate) icon_font: HFONT,
    pub(crate) title: &'a mut [u16],
    pub(crate) palette: &'a Palette,
    pub(crate) scale: Scale,
}

/// A checkbox's natural size around its title's.
pub(crate) fn checkbox_size(title: Size) -> Size {
    Size::new(
        CHECKBOX_BOX + CHECKBOX_GAP + title.width + 2.0 * FOCUS_MARGIN,
        CHECKBOX_BOX.max(title.height + 2.0 * FOCUS_MARGIN),
    )
}

/// A button's natural size, from the size Win32 gives its title with padding.
pub(crate) fn button_size(ideal: Size, style: ButtonStyle, icon: Icon) -> Size {
    let icon_width = if glyph(icon).is_some() {
        ICON_SIZE + ICON_GAP
    } else {
        0.0
    };
    let height = ideal.height.max(MIN_BUTTON_SIZE.height);
    match style {
        ButtonStyle::Normal | ButtonStyle::Primary => Size::new(
            (ideal.width + icon_width).max(MIN_BUTTON_SIZE.width),
            height,
        ),
        ButtonStyle::Plain => Size::new(ideal.width + icon_width, height),
        ButtonStyle::Icon => Size::new(ICON_BUTTON_SIDE, ICON_BUTTON_SIDE),
    }
}

/// A nav item's natural size around its title's and badge's.
pub(crate) fn nav_item_size(title: Size, badge: Size, icon: Icon) -> Size {
    let icon_width = if glyph(icon).is_some() {
        ICON_SIZE + ICON_GAP
    } else {
        0.0
    };
    let badge_width = if badge.width > 0.0 {
        ICON_GAP + badge.width
    } else {
        0.0
    };
    Size::new(
        2.0 * NAV_ITEM_INSET + icon_width + title.width + badge_width,
        NAV_ITEM_HEIGHT.max(title.height),
    )
}

pub(crate) fn draw_button(canvas: &mut Canvas<'_>, style: ButtonStyle, icon: Icon, state: State) {
    let palette = canvas.palette;
    canvas.clear();
    let content = match style {
        ButtonStyle::Normal | ButtonStyle::Primary => {
            let (fill, border) = if style == ButtonStyle::Primary {
                (&palette.accent_control, None)
            } else {
                (&palette.control, Some(palette.control_border))
            };
            let color = state.fill_color(fill);
            canvas.round_rect(canvas.rect, color, border.unwrap_or(color));
            fill.text
        }
        ButtonStyle::Plain | ButtonStyle::Icon => {
            canvas.subtle_fill(state, false);
            if style == ButtonStyle::Plain {
                palette.text_color(TextColor::Accent)
            } else {
                palette.text_color(TextColor::Secondary)
            }
        }
    };
    if state.shows_focus() {
        canvas.focus_ring();
    }
    canvas.centered_content(glyph(icon), style != ButtonStyle::Icon, content, state);
}

/// A sidebar row: Fluent's navigation item, with a selection mark at its start.
pub(crate) fn draw_nav_item(
    canvas: &mut Canvas<'_>,
    icon: Icon,
    badge: &mut [u16],
    selected: bool,
    state: State,
) {
    let palette = canvas.palette;
    let accent = palette.text_color(TextColor::Accent);
    canvas.clear();
    canvas.subtle_fill(state, selected);
    if selected {
        let width = canvas.scale.to_pixels(SELECTION_MARK.width);
        let height = canvas.scale.to_pixels(SELECTION_MARK.height);
        let top = canvas.rect.top + (canvas.rect.bottom - canvas.rect.top - height) / 2;
        let mark = RECT {
            left: canvas.rect.left,
            top,
            right: canvas.rect.left + width,
            bottom: top + height,
        };
        round_rect(canvas.dc, mark, width / 2, accent, accent);
    }
    if state.shows_focus() {
        canvas.focus_ring();
    }
    let inset = canvas.scale.to_pixels(NAV_ITEM_INSET);
    let mut rest = RECT {
        left: canvas.rect.left + inset,
        right: canvas.rect.right - inset,
        ..canvas.rect
    };
    if let Some(glyph) = glyph(icon) {
        let side = canvas.scale.to_pixels(ICON_SIZE);
        let mut icon_rect = RECT {
            right: rest.left + side,
            ..rest
        };
        canvas.glyph(&mut icon_rect, glyph, accent);
        rest.left += side + canvas.scale.to_pixels(ICON_GAP);
    }
    if !badge.is_empty() {
        let mut badge_rect = rest;
        draw_text(
            canvas.dc,
            badge,
            canvas.font,
            &mut badge_rect,
            palette.text_color(TextColor::Secondary),
            DT_RIGHT | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        rest.right -= text_width(canvas.dc, badge, canvas.font) + canvas.scale.to_pixels(ICON_GAP);
    }
    canvas.text(
        &mut rest,
        palette.text,
        DT_LEFT | DT_VCENTER | DT_SINGLELINE | DT_END_ELLIPSIS | state.text_format(),
    );
}

pub(crate) fn draw_checkbox(canvas: &mut Canvas<'_>, checked: bool, state: State) {
    let palette = canvas.palette;
    let side = canvas.scale.to_pixels(CHECKBOX_BOX);
    let top = canvas.rect.top + (canvas.rect.bottom - canvas.rect.top - side) / 2;
    let left = canvas.rect.left;
    let check_box = RECT {
        left,
        top,
        right: left + side,
        bottom: top + side,
    };
    canvas.clear();
    if checked {
        let color = state.fill_color(&palette.accent_control);
        canvas.round_rect(check_box, color, color);
        canvas
            .check_mark(check_box, palette.accent_control.text)
            .expect("Windows can create a pen for the check mark");
    } else {
        let color = state.fill_color(&palette.control);
        canvas.round_rect(check_box, color, palette.strong_border);
    }
    let margin = canvas.scale.to_pixels(FOCUS_MARGIN);
    let mut title_rect = RECT {
        left: check_box.right + canvas.scale.to_pixels(CHECKBOX_GAP) + margin,
        top: canvas.rect.top + margin,
        right: canvas.rect.right - margin,
        bottom: canvas.rect.bottom - margin,
    };
    canvas.text(
        &mut title_rect,
        palette.text,
        DT_LEFT | DT_VCENTER | CHECKBOX_TEXT_FORMAT | state.text_format(),
    );
    if state.shows_focus() {
        let focus = RECT {
            left: title_rect.left - margin,
            right: title_rect.right + margin,
            ..canvas.rect
        };
        // SAFETY: `focus` is a valid RECT inside the control.
        unsafe { DrawFocusRect(canvas.dc, &raw const focus) }
            .expect("a control's device context can draw a focus rectangle");
    }
}

impl Canvas<'_> {
    /// Paints the whole control in its backdrop, behind its rounded corners.
    fn clear(&self) {
        let brush = self.dc_brush(self.backdrop);
        // SAFETY: `rect` is the control's area in its own device context.
        unsafe { FillRect(self.dc, &raw const self.rect, brush) };
    }

    fn round_rect(&self, rect: RECT, fill: COLORREF, border: COLORREF) {
        round_rect(
            self.dc,
            rect,
            self.scale.to_pixels(CORNER_RADIUS),
            fill,
            border,
        );
    }

    /// Fluent's focus ring, drawn in the text color just inside the edge.
    fn focus_ring(&self) {
        let diameter = 2 * self.scale.to_pixels(CORNER_RADIUS);
        let _pen = self.dc_pen(self.palette.text);
        // SAFETY: NULL_BRUSH is a stock object, which never needs deleting.
        let _brush = Selected::new(self.dc, unsafe { GetStockObject(NULL_BRUSH) });
        for inset in 0..self.scale.to_pixels(FOCUS_RING) {
            // SAFETY: the DC pen and the hollow brush are selected into `dc`.
            unsafe {
                RoundRect(
                    self.dc,
                    self.rect.left + inset,
                    self.rect.top + inset,
                    self.rect.right - inset,
                    self.rect.bottom - inset,
                    diameter,
                    diameter,
                )
            }
            .expect("a control's device context can draw a rounded rectangle");
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "check mark points lie inside a box a few dozen pixels wide"
    )]
    fn check_mark(&self, check_box: RECT, color: COLORREF) -> Result<()> {
        let side = (check_box.right - check_box.left) as f32;
        let points = CHECK_MARK.map(|(x, y)| POINT {
            x: check_box.left + (x * side).round() as i32,
            y: check_box.top + (y * side).round() as i32,
        });
        let width = self.scale.to_pixels(CHECK_STROKE).max(1);
        // SAFETY: `CreatePen` accepts any width and color.
        let pen = Owned::new(unsafe { CreatePen(PS_SOLID, width, color) })?;
        let _pen = Selected::new(self.dc, pen.get());
        // SAFETY: the pen is selected into `dc` for this call.
        unsafe { Polyline(self.dc, &points) }.ok()
    }

    /// Fluent's subtle fill for borderless controls: the backdrop tinted toward the text
    /// color while selected, hovered or pressed, and left alone otherwise.
    fn subtle_fill(&self, state: State, selected: bool) {
        let tint = if state.0.contains(CDIS_SELECTED) {
            SUBTLE_PRESSED
        } else if selected {
            SUBTLE_SELECTED
        } else if state.0.contains(CDIS_HOT) {
            SUBTLE_HOT
        } else {
            return;
        };
        let color = blend(self.palette.text, self.backdrop, tint);
        self.round_rect(self.rect, color, color);
    }

    /// An icon and the title after it, centered together, either of them optional.
    fn centered_content(
        &mut self,
        icon: Option<char>,
        show_title: bool,
        color: COLORREF,
        state: State,
    ) {
        let side = self.scale.to_pixels(ICON_SIZE);
        let gap = self.scale.to_pixels(ICON_GAP);
        let title_width = if show_title {
            text_width(self.dc, self.title, self.font)
        } else {
            0
        };
        let icon_width = match (icon, show_title) {
            (None, _) => 0,
            (Some(_), false) => side,
            (Some(_), true) => side + gap,
        };
        let width = icon_width + title_width;
        let mut left = self.rect.left + (self.rect.right - self.rect.left - width) / 2;
        if let Some(icon) = icon {
            let mut icon_rect = RECT {
                left,
                right: left + side,
                ..self.rect
            };
            self.glyph(&mut icon_rect, icon, color);
            left += icon_width;
        }
        if show_title {
            let mut title_rect = RECT { left, ..self.rect };
            self.text(
                &mut title_rect,
                color,
                DT_LEFT | DT_VCENTER | DT_SINGLELINE | state.text_format(),
            );
        }
    }

    fn glyph(&self, rect: &mut RECT, glyph: char, color: COLORREF) {
        let mut buffer = [0_u16; 2];
        draw_text(
            self.dc,
            glyph.encode_utf16(&mut buffer),
            self.icon_font,
            rect,
            color,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
    }

    fn text(&mut self, rect: &mut RECT, color: COLORREF, format: DRAW_TEXT_FORMAT) {
        draw_text(self.dc, self.title, self.font, rect, color, format);
    }

    fn dc_brush(&self, color: COLORREF) -> HBRUSH {
        dc_brush(self.dc, color)
    }

    fn dc_pen(&self, color: COLORREF) -> Selected {
        dc_pen(self.dc, color)
    }
}

fn draw_text(
    dc: HDC,
    text: &mut [u16],
    font: HFONT,
    rect: &mut RECT,
    color: COLORREF,
    format: DRAW_TEXT_FORMAT,
) {
    if text.is_empty() {
        return;
    }
    let _font = Selected::new(dc, font);
    // SAFETY: `dc` is a device context being painted, and `rect` a valid RECT inside it.
    unsafe {
        SetBkMode(dc, TRANSPARENT);
        SetTextColor(dc, color);
        DrawTextW(dc, text, rect, format);
    }
}

/// How wide `text` is in `font`, in pixels.
fn text_width(dc: HDC, text: &mut [u16], font: HFONT) -> i32 {
    if text.is_empty() {
        return 0;
    }
    let _font = Selected::new(dc, font);
    let mut bounds = RECT::default();
    // SAFETY: `bounds` is a valid RECT; DT_CALCRECT only measures.
    unsafe {
        DrawTextW(
            dc,
            text,
            &raw mut bounds,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
    }
    bounds.right - bounds.left
}

/// Fills a surface's area of the window, `rect` being in the window's pixels.
pub(crate) fn paint_surface(
    dc: HDC,
    rect: RECT,
    surface: Surface,
    palette: &Palette,
    scale: Scale,
) {
    let fill = palette.surface(surface);
    match surface {
        Surface::Card => round_rect(
            dc,
            rect,
            scale.to_pixels(CARD_CORNER_RADIUS),
            fill,
            palette.card_border,
        ),
        Surface::None | Surface::Sidebar | Surface::Content => {
            // SAFETY: `rect` is inside the window's client area, which `dc` paints.
            unsafe { FillRect(dc, &raw const rect, dc_brush(dc, fill)) };
        }
    }
}

fn round_rect(dc: HDC, rect: RECT, radius: i32, fill: COLORREF, border: COLORREF) {
    let diameter = 2 * radius;
    let _pen = dc_pen(dc, border);
    let _brush = Selected::new(dc, dc_brush(dc, fill));
    // SAFETY: the DC pen and brush are selected into `dc` for this call.
    unsafe {
        RoundRect(
            dc,
            rect.left,
            rect.top,
            rect.right,
            rect.bottom,
            diameter,
            diameter,
        )
    }
    .expect("a device context can draw a rounded rectangle");
}

/// The stock DC brush in `color`. Stock objects never need deleting.
fn dc_brush(dc: HDC, color: COLORREF) -> HBRUSH {
    // SAFETY: `dc` is a live device context; DC_BRUSH is a stock object.
    unsafe {
        SetDCBrushColor(dc, color);
        HBRUSH(GetStockObject(DC_BRUSH).0)
    }
}

/// The stock DC pen in `color`, selected until the guard drops.
fn dc_pen(dc: HDC, color: COLORREF) -> Selected {
    // SAFETY: `dc` is a live device context; DC_PEN is a stock object.
    let pen = unsafe {
        SetDCPenColor(dc, color);
        GetStockObject(DC_PEN)
    };
    Selected::new(dc, pen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_checkbox_is_never_shorter_than_its_box() {
        let size = checkbox_size(Size::new(40.0, 4.0));
        assert!((size.height - CHECKBOX_BOX).abs() < f32::EPSILON);
        assert!(size.width > 40.0 + CHECKBOX_BOX);
    }
}
