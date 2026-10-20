//! The common control behind each Nifty widget, and how it is sized and updated.

use std::borrow::Cow;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use windows::Win32::Foundation::{HWND, LPARAM, RECT, SIZE};
use windows::Win32::Graphics::Gdi::{
    DT_CALCRECT, DT_EXPANDTABS, DT_NOPREFIX, DrawTextW, GetDC, GetTextMetricsW, HDC, HFONT,
    HGDIOBJ, ReleaseDC, SelectObject, TEXTMETRICW,
};
use windows::Win32::System::SystemServices::{SS_CENTER, SS_LEFT, SS_NOPREFIX, SS_RIGHT};
use windows::Win32::UI::Controls::{
    BCM_GETIDEALSIZE, BST_CHECKED, BST_UNCHECKED, EM_GETMARGINS, EM_SETCUEBANNER,
};
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    BM_SETCHECK, BS_AUTOCHECKBOX, BS_DEFPUSHBUTTON, BS_PUSHBUTTON, CreateWindowExW, DestroyWindow,
    ES_AUTOHSCROLL, ES_CENTER, ES_LEFT, ES_MULTILINE, ES_READONLY, ES_RIGHT, GetWindowTextLengthW,
    GetWindowTextW, HMENU, SM_CXEDGE, SM_CYEDGE, SW_HIDE, SWP_NOACTIVATE, SWP_NOZORDER,
    SendMessageW, SetWindowPos, SetWindowTextW, ShowWindow, WINDOW_EX_STYLE, WINDOW_STYLE,
    WM_SETFONT, WS_CHILD, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
};
use windows::core::{PCWSTR, Result, w};

use nifty_runtime::{
    ButtonStyle, ClickHandler, Frame, LabelStyle, Size, TextAlign, TextColor, TextHandler,
    TextStyle, ToggleHandler,
};

use crate::convert::{Scale, handle_param, high_word, low_word, out_param, rect_size, wide};
use crate::window::{Control, Events, Paint, WindowState};

/// Text fields have no useful natural width when empty, so they start at least this wide.
const MIN_TEXT_FIELD_WIDTH: f32 = 200.0;
/// Space between a text field's border and its text, above and below.
const TEXT_FIELD_INSET: f32 = 2.0;
/// The Windows guideline's smallest push button, in DIPs.
const MIN_BUTTON_SIZE: Size = Size::new(75.0, 23.0);
const DIVIDER_SIZE: Size = Size::new(1.0, 1.0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WidgetKind {
    Label,
    /// A borderless read-only edit control, which is how Win32 lets users select text.
    SelectableLabel,
    Button,
    TextField,
    Checkbox,
    Divider,
}

/// Where a label's text sits, once its start and end are resolved for the reading direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Edge {
    Left,
    Center,
    Right,
}

impl Edge {
    fn of(align: TextAlign, right_to_left: bool) -> Self {
        match (align, right_to_left) {
            (TextAlign::Center, _) => Self::Center,
            (TextAlign::Start, false) | (TextAlign::End, true) => Self::Left,
            (TextAlign::Start, true) | (TextAlign::End, false) => Self::Right,
        }
    }
}

/// Destroying the control is left to `Drop`, so a removed widget can be kept until the
/// event that removed it has finished.
pub struct WinWidget {
    id: u16,
    kind: WidgetKind,
    control: Rc<Control>,
    window: Rc<WindowState>,
}

impl fmt::Debug for WinWidget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WinWidget")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("control", &self.control)
            .finish_non_exhaustive()
    }
}

/// The class, styles and behaviour a new control is created with.
struct Spec<'a> {
    kind: WidgetKind,
    class: PCWSTR,
    text: &'a str,
    style: WINDOW_STYLE,
    ex_style: WINDOW_EX_STYLE,
    font: TextStyle,
    paint: Option<Paint>,
    events: Events,
}

impl WinWidget {
    pub(crate) fn label(window: &Rc<WindowState>, text: &str, style: LabelStyle) -> Result<Self> {
        let edge = Edge::of(style.align, window.reads_right_to_left());
        let (kind, class, class_bits) = if style.selectable {
            let align = match edge {
                Edge::Left => ES_LEFT,
                Edge::Center => ES_CENTER,
                Edge::Right => ES_RIGHT,
            };
            let bits = class_style(ES_READONLY | ES_MULTILINE | align);
            (WidgetKind::SelectableLabel, w!("EDIT"), bits)
        } else {
            let align = match edge {
                Edge::Left => SS_LEFT,
                Edge::Center => SS_CENTER,
                Edge::Right => SS_RIGHT,
            };
            (WidgetKind::Label, w!("STATIC"), SS_NOPREFIX.0 | align.0)
        };
        Self::create(
            window,
            Spec {
                kind,
                class,
                text,
                style: WINDOW_STYLE(class_bits),
                ex_style: WINDOW_EX_STYLE(0),
                font: style.font,
                paint: Some(Paint::Text(style.color)),
                events: Events::None,
            },
        )
    }

    pub(crate) fn button(
        window: &Rc<WindowState>,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> Result<Self> {
        let button_style = match style {
            ButtonStyle::Normal => BS_PUSHBUTTON,
            ButtonStyle::Primary => BS_DEFPUSHBUTTON,
        };
        let button = Self::create(
            window,
            Spec {
                kind: WidgetKind::Button,
                class: w!("BUTTON"),
                text: title,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(button_style)),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: None,
                events: Events::Click(on_click),
            },
        )?;
        if style == ButtonStyle::Primary {
            window.add_primary_button(button.id);
        }
        Ok(button)
    }

    pub(crate) fn text_field(
        window: &Rc<WindowState>,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> Result<Self> {
        let field = Self::create(
            window,
            Spec {
                kind: WidgetKind::TextField,
                class: w!("EDIT"),
                text,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(ES_AUTOHSCROLL)),
                ex_style: WS_EX_CLIENTEDGE,
                font: TextStyle::Body,
                paint: None,
                events: Events::Change(on_change),
            },
        )?;
        let placeholder = wide(placeholder);
        // SAFETY: the placeholder is NUL-terminated, and the control copies it.
        unsafe {
            SendMessageW(
                field.hwnd(),
                EM_SETCUEBANNER,
                None,
                Some(LPARAM(
                    placeholder.as_ptr().expose_provenance().cast_signed(),
                )),
            );
        }
        Ok(field)
    }

    pub(crate) fn checkbox(
        window: &Rc<WindowState>,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> Result<Self> {
        let checkbox = Self::create(
            window,
            Spec {
                kind: WidgetKind::Checkbox,
                class: w!("BUTTON"),
                text: title,
                style: WS_TABSTOP | WINDOW_STYLE(class_style(BS_AUTOCHECKBOX)),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Some(Paint::Text(TextColor::Primary)),
                events: Events::Toggle(on_toggle),
            },
        )?;
        checkbox.set_checked(checked);
        Ok(checkbox)
    }

    pub(crate) fn divider(window: &Rc<WindowState>) -> Result<Self> {
        Self::create(
            window,
            Spec {
                kind: WidgetKind::Divider,
                class: w!("STATIC"),
                text: "",
                style: WINDOW_STYLE(0),
                ex_style: WINDOW_EX_STYLE(0),
                font: TextStyle::Body,
                paint: Some(Paint::Divider),
                events: Events::None,
            },
        )
    }

    fn create(window: &Rc<WindowState>, spec: Spec<'_>) -> Result<Self> {
        let id = window.allocate_id();
        let text = wide(&display_text(spec.kind, spec.text));
        // SAFETY: the parent is a live Nifty window, and `text` outlives the call. The
        // control starts at zero size until the layout places it.
        let hwnd = unsafe {
            CreateWindowExW(
                spec.ex_style,
                spec.class,
                PCWSTR(text.as_ptr()),
                WS_CHILD | WS_VISIBLE | spec.style,
                0,
                0,
                0,
                0,
                Some(window.hwnd()),
                Some(HMENU(std::ptr::without_provenance_mut(usize::from(id)))),
                None,
                None,
            )?
        };
        set_font(hwnd, window.font(spec.font));
        let control = Rc::new(Control {
            hwnd,
            font: spec.font,
            paint: spec.paint,
            events: spec.events,
            changing_in_code: Cell::new(false),
        });
        window.add_control(id, Rc::clone(&control));
        Ok(Self {
            id,
            kind: spec.kind,
            control,
            window: Rc::clone(window),
        })
    }

    fn hwnd(&self) -> HWND {
        self.control.hwnd
    }

    pub(crate) fn set_text(&self, text: &str) {
        // Re-setting an unchanged value would move the cursor while the user types.
        if self.kind == WidgetKind::Divider
            || read_text(self.hwnd()) == display_text(self.kind, text)
        {
            return;
        }
        let text = wide(&display_text(self.kind, text));
        self.control.changing_in_code.set(true);
        // SAFETY: `text` is NUL-terminated and outlives the call.
        let result = unsafe { SetWindowTextW(self.hwnd(), PCWSTR(text.as_ptr())) };
        self.control.changing_in_code.set(false);
        result.expect("a live control can take new text");
    }

    pub(crate) fn set_checked(&self, checked: bool) {
        if self.kind == WidgetKind::Checkbox {
            let state = if checked { BST_CHECKED } else { BST_UNCHECKED };
            // SAFETY: BM_SETCHECK takes the new state and does not notify the parent.
            unsafe {
                SendMessageW(
                    self.hwnd(),
                    BM_SETCHECK,
                    Some(windows::Win32::Foundation::WPARAM(
                        usize::try_from(state.0).expect("check states are small"),
                    )),
                    None,
                );
            }
        }
    }

    /// The natural size in DIPs, measured with the control's own font at its window's DPI.
    pub(crate) fn measure(&self) -> Size {
        let scale = self.window.scale();
        match self.kind {
            WidgetKind::Divider => DIVIDER_SIZE,
            WidgetKind::Button => {
                let ideal = self.ideal_size(scale);
                Size::new(
                    ideal.width.max(MIN_BUTTON_SIZE.width),
                    ideal.height.max(MIN_BUTTON_SIZE.height),
                )
            }
            WidgetKind::Checkbox => self.ideal_size(scale),
            WidgetKind::Label => self.text_size(scale),
            WidgetKind::SelectableLabel => {
                let text = self.text_size(scale);
                Size::new(text.width + self.edit_margins(scale), text.height)
            }
            WidgetKind::TextField => {
                let text = self.text_size(scale);
                let edge_x = scale.to_dips(system_metric(SM_CXEDGE, scale));
                let edge_y = scale.to_dips(system_metric(SM_CYEDGE, scale));
                let width = text.width + self.edit_margins(scale) + 2.0 * edge_x;
                let height = text.height + 2.0 * (edge_y + TEXT_FIELD_INSET);
                Size::new(width.max(MIN_TEXT_FIELD_WIDTH), height)
            }
        }
    }

    pub(crate) fn set_frame(&self, frame: Frame) {
        let rect = self.window.scale().frame_to_pixels(frame);
        let (width, height) = rect_size(&rect);
        // SAFETY: `hwnd` is a live child control on this thread.
        unsafe {
            SetWindowPos(
                self.hwnd(),
                None,
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .expect("a live control can be moved");
    }

    /// Hides the control and stops its events now; the caller decides when to drop it.
    pub(crate) fn retire(&self) {
        self.window.remove_control(self.id);
        // SAFETY: `hwnd` is a live child control on this thread.
        let _was_visible = unsafe { ShowWindow(self.hwnd(), SW_HIDE) };
    }

    /// Button and checkbox sizes, padding included, from Common Controls 6.
    fn ideal_size(&self, scale: Scale) -> Size {
        let mut ideal = SIZE::default();
        // SAFETY: BCM_GETIDEALSIZE writes a SIZE through the pointer it is given.
        let found = unsafe {
            SendMessageW(
                self.hwnd(),
                BCM_GETIDEALSIZE,
                None,
                Some(out_param(&mut ideal)),
            )
        };
        assert!(
            found.0 != 0,
            "BCM_GETIDEALSIZE needs Common Controls 6; embed Nifty's application manifest"
        );
        scale.size_to_dips(ideal.cx, ideal.cy)
    }

    /// The extent of the control's current text, at least one line high even when empty.
    fn text_size(&self, scale: Scale) -> Size {
        let mut text: Vec<u16> = read_text(self.hwnd()).encode_utf16().collect();
        let dc = FontContext::new(self.hwnd(), self.window.font(self.control.font));
        // DrawTextW reads the first character even when told the text is empty, and an
        // empty slice's pointer is dangling.
        if text.is_empty() {
            return scale.size_to_dips(0, dc.line_height());
        }
        let mut bounds = RECT::default();
        // SAFETY: `bounds` is a valid RECT; DT_CALCRECT only measures.
        unsafe {
            DrawTextW(
                dc.dc,
                &mut text,
                &raw mut bounds,
                DT_CALCRECT | DT_NOPREFIX | DT_EXPANDTABS,
            );
        }
        let (width, height) = rect_size(&bounds);
        scale.size_to_dips(width, height.max(dc.line_height()))
    }

    /// The left and right space an edit control keeps around its text.
    fn edit_margins(&self, scale: Scale) -> f32 {
        // SAFETY: EM_GETMARGINS takes no parameters.
        let margins = unsafe { SendMessageW(self.hwnd(), EM_GETMARGINS, None, None) };
        let margins = margins.0.cast_unsigned();
        scale.to_dips(i32::from(low_word(margins)) + i32::from(high_word(margins)))
    }
}

impl Drop for WinWidget {
    /// A destroyed window has already destroyed its controls along with it.
    fn drop(&mut self) {
        if !self.window.is_destroyed() {
            // SAFETY: the control belongs to this thread and has not been destroyed.
            unsafe { DestroyWindow(self.hwnd()) }.expect("a live control can be destroyed");
        }
    }
}

/// A control's device context with its font selected, restored and released on drop.
struct FontContext {
    hwnd: HWND,
    dc: HDC,
    previous: HGDIOBJ,
}

impl FontContext {
    fn new(hwnd: HWND, font: HFONT) -> Self {
        // SAFETY: `hwnd` is a live control; its DC is released in `drop`.
        let dc = unsafe { GetDC(Some(hwnd)) };
        assert!(!dc.is_invalid(), "a live control has a device context");
        // SAFETY: `font` is a live font owned by the control's window.
        let previous = unsafe { SelectObject(dc, font.into()) };
        Self { hwnd, dc, previous }
    }

    fn line_height(&self) -> i32 {
        let mut metrics = TEXTMETRICW::default();
        // SAFETY: `metrics` is a valid TEXTMETRICW to write into.
        let found = unsafe { GetTextMetricsW(self.dc, &raw mut metrics) };
        assert!(
            found.as_bool(),
            "a DC with a font selected has text metrics"
        );
        metrics.tmHeight
    }
}

impl Drop for FontContext {
    fn drop(&mut self) {
        // SAFETY: restores the DC's original font, then releases the DC `new` acquired.
        unsafe {
            SelectObject(self.dc, self.previous);
            ReleaseDC(Some(self.hwnd), self.dc);
        }
    }
}

pub(crate) fn set_font(hwnd: HWND, font: HFONT) {
    // SAFETY: WM_SETFONT takes a font handle and a redraw flag; the font outlives the control.
    unsafe {
        SendMessageW(
            hwnd,
            WM_SETFONT,
            Some(handle_param(font.0)),
            Some(LPARAM(1)),
        );
    }
}

pub(crate) fn read_text(hwnd: HWND) -> String {
    // SAFETY: `hwnd` is a live control on this thread.
    let length = unsafe { GetWindowTextLengthW(hwnd) };
    let mut buffer = vec![0; usize::try_from(length).expect("text lengths are never negative") + 1];
    // SAFETY: `buffer` has room for the text and its NUL terminator.
    let copied = unsafe { GetWindowTextW(hwnd, &mut buffer) };
    buffer.truncate(usize::try_from(copied).expect("copied lengths are never negative"));
    String::from_utf16_lossy(&buffer)
}

/// The text as the control must hold it: `&` marks a mnemonic in button titles, and edit
/// controls break lines only at `\r\n`.
fn display_text(kind: WidgetKind, text: &str) -> Cow<'_, str> {
    match kind {
        WidgetKind::Button | WidgetKind::Checkbox if text.contains('&') => {
            Cow::Owned(text.replace('&', "&&"))
        }
        WidgetKind::SelectableLabel if text.contains('\n') => {
            Cow::Owned(text.replace("\r\n", "\n").replace('\n', "\r\n"))
        }
        _ => Cow::Borrowed(text),
    }
}

fn system_metric(
    metric: windows::Win32::UI::WindowsAndMessaging::SYSTEM_METRICS_INDEX,
    scale: Scale,
) -> i32 {
    // SAFETY: `GetSystemMetricsForDpi` accepts every metric index.
    unsafe { GetSystemMetricsForDpi(metric, scale.dpi()) }
}

/// Win32 declares each control class's style bits in its own type; they all share the low
/// bits of the window style.
fn class_style(bits: impl Into<i64>) -> u32 {
    u32::try_from(bits.into()).expect("class styles fit the window style's low bits")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ampersands_show_literally_in_button_titles() {
        assert_eq!(
            display_text(WidgetKind::Button, "Save & Quit"),
            "Save && Quit"
        );
        assert_eq!(
            display_text(WidgetKind::Label, "Save & Quit"),
            "Save & Quit"
        );
    }

    #[test]
    fn selectable_labels_break_lines_the_way_edit_controls_do() {
        assert_eq!(
            display_text(WidgetKind::SelectableLabel, "one\ntwo\r\nthree"),
            "one\r\ntwo\r\nthree"
        );
    }

    #[test]
    fn start_and_end_follow_the_reading_direction() {
        assert_eq!(Edge::of(TextAlign::Start, false), Edge::Left);
        assert_eq!(Edge::of(TextAlign::End, false), Edge::Right);
        assert_eq!(Edge::of(TextAlign::Start, true), Edge::Right);
        assert_eq!(Edge::of(TextAlign::End, true), Edge::Left);
        assert_eq!(Edge::of(TextAlign::Center, true), Edge::Center);
    }
}
