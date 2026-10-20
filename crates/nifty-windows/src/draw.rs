//! Push buttons and checkboxes painted in the window's palette. They stay real `BUTTON`
//! controls, so keyboard, focus and accessibility behave as usual; only their pixels come
//! from Nifty, because Win32's themes for them have no documented dark mode.

use windows::Win32::Foundation::{COLORREF, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    CreatePen, DC_BRUSH, DC_PEN, DRAW_TEXT_FORMAT, DT_CENTER, DT_HIDEPREFIX, DT_LEFT,
    DT_SINGLELINE, DT_VCENTER, DrawFocusRect, DrawTextW, FillRect, GetStockObject, HBRUSH, HDC,
    HFONT, NULL_BRUSH, PS_SOLID, Polyline, RoundRect, SetBkMode, SetDCBrushColor, SetDCPenColor,
    SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::Controls::{
    CDIS_FOCUS, CDIS_HOT, CDIS_SELECTED, CDIS_SHOWKEYBOARDCUES, NMCUSTOMDRAW_DRAW_STATE_FLAGS,
};
use windows::core::Result;

use nifty_runtime::{ButtonStyle, Size};

use crate::convert::Scale;
use crate::gdi::{Owned, Selected};
use crate::palette::{Fill, Palette};

/// Fluent's control corner radius and focus ring width, in DIPs.
const CORNER_RADIUS: f32 = 4.0;
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
    pub(crate) font: HFONT,
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

pub(crate) fn draw_button(canvas: &mut Canvas<'_>, style: ButtonStyle, state: State) {
    let palette = canvas.palette;
    let (fill, border) = match style {
        ButtonStyle::Normal => (&palette.control, Some(palette.control_border)),
        ButtonStyle::Primary => (&palette.accent_control, None),
    };
    let color = state.fill_color(fill);
    canvas.clear();
    canvas.round_rect(canvas.rect, color, border.unwrap_or(color));
    if state.shows_focus() {
        canvas.focus_ring();
    }
    let mut text_rect = canvas.rect;
    canvas.text(
        &mut text_rect,
        fill.text,
        DT_CENTER | DT_VCENTER | DT_SINGLELINE | state.text_format(),
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
    /// Paints the whole control in the window background, behind its rounded corners.
    fn clear(&self) {
        let brush = self.dc_brush(self.palette.background);
        // SAFETY: `rect` is the control's area in its own device context.
        unsafe { FillRect(self.dc, &raw const self.rect, brush) };
    }

    fn round_rect(&self, rect: RECT, fill: COLORREF, border: COLORREF) {
        let diameter = 2 * self.scale.to_pixels(CORNER_RADIUS);
        let _pen = self.dc_pen(border);
        let _brush = Selected::new(self.dc, self.dc_brush(fill));
        // SAFETY: the DC pen and brush are selected into `dc` for this call.
        unsafe {
            RoundRect(
                self.dc,
                rect.left,
                rect.top,
                rect.right,
                rect.bottom,
                diameter,
                diameter,
            )
        }
        .expect("a control's device context can draw a rounded rectangle");
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

    fn text(&mut self, rect: &mut RECT, color: COLORREF, format: DRAW_TEXT_FORMAT) {
        let _font = Selected::new(self.dc, self.font);
        // SAFETY: `dc` is the control's device context for this paint, and `rect` a valid
        // RECT inside it.
        unsafe {
            SetBkMode(self.dc, TRANSPARENT);
            SetTextColor(self.dc, color);
            DrawTextW(self.dc, self.title, rect, format);
        }
    }

    /// The stock DC brush in `color`. Stock objects never need deleting.
    fn dc_brush(&self, color: COLORREF) -> HBRUSH {
        // SAFETY: `dc` is a live device context; DC_BRUSH is a stock object.
        unsafe {
            SetDCBrushColor(self.dc, color);
            HBRUSH(GetStockObject(DC_BRUSH).0)
        }
    }

    /// The stock DC pen in `color`, selected until the guard drops.
    fn dc_pen(&self, color: COLORREF) -> Selected {
        // SAFETY: `dc` is a live device context; DC_PEN is a stock object.
        let pen = unsafe {
            SetDCPenColor(self.dc, color);
            GetStockObject(DC_PEN)
        };
        Selected::new(self.dc, pen)
    }
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
