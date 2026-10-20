//! Top-level windows, and the window procedure that routes their child controls' events.

use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::rc::Rc;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWM_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW, DWMSBT_NONE, DWMSBT_TRANSIENTWINDOW,
    DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWINDOWATTRIBUTE,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    FillRect, GetMonitorInfoW, HDC, HFONT, MONITOR_DEFAULTTONEAREST, MONITORINFO, MapWindowPoints,
    MonitorFromWindow, RDW_ALLCHILDREN, RDW_ERASE, RDW_FRAME, RDW_INVALIDATE, RedrawWindow,
    SetBkColor, SetTextColor,
};
use windows::Win32::UI::Controls::{
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, CDRF_SKIPDEFAULT,
    DRAWITEMSTRUCT, NM_CUSTOMDRAW, NMCUSTOMDRAW, NMHDR, TBCD_CHANNEL, TBCD_THUMB,
};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, CW_USEDEFAULT, CreateWindowExW, DC_HASDEFID, DM_GETDEFID, DefWindowProcW,
    DestroyWindow, EN_CHANGE, GA_ROOT, GetAncestor, GetClientRect, GetDlgCtrlID, GetParent,
    GetWindowRect, IsChild, MINMAXINFO, PostQuitMessage, SIZE_MINIMIZED, SW_SHOW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, SetWindowTextW, ShowWindow, WA_INACTIVE,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT,
    WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_DRAWITEM, WM_ERASEBKGND, WM_GETMINMAXINFO,
    WM_HSCROLL, WM_NCDESTROY, WM_NOTIFY, WM_SETFOCUS, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCOLORCHANGE,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::{BOOL, PCWSTR, Result, w};

use nifty_runtime::{
    Backdrop, ButtonStyle, ClickHandler, Icon, ImageSize, NumberHandler, ResizeHandler, Size,
    Surface, TextColor, TextHandler, TextStyle, ToggleHandler,
};

use crate::convert::{Scale, dc_param, high_word, hwnd_key, hwnd_param, low_word, rect_size, wide};
use crate::draw::{
    Canvas, State, draw_button, draw_checkbox, draw_image, draw_nav_item, draw_picker,
    draw_slider_thumb, draw_slider_track, draw_switch,
};
use crate::fonts::Fonts;
use crate::image::Picture;
use crate::menu;
use crate::palette::{Palette, Theme};
use crate::scroll::scroll_area;
use crate::settings::{Appearance, Settings, reads_right_to_left};
use crate::surfaces::Surfaces;
use crate::tooltips::Tooltips;
use crate::widgets::{is_checked, read_text, set_font, slider_position, thumb_rect, typed_text};

pub(crate) const WINDOW_CLASS: PCWSTR = w!("NiftyWindow");
const STYLE: WINDOW_STYLE = WS_OVERLAPPEDWINDOW;
const EX_STYLE: WINDOW_EX_STYLE = WINDOW_EX_STYLE(0);
/// Control ids start above the dialog command ids, such as `IDOK`, that Win32 sends itself.
const FIRST_CONTROL_ID: u16 = 100;

thread_local! {
    /// The window procedure finds a window's state here; entries leave on `WM_NCDESTROY`.
    static OPEN_WINDOWS: RefCell<HashMap<usize, Rc<WindowState>>> = RefCell::new(HashMap::new());
}

#[derive(Debug)]
pub struct WinWindow(Rc<WindowState>);

impl WinWindow {
    pub(crate) fn state(&self) -> &Rc<WindowState> {
        &self.0
    }
}

impl Drop for WinWindow {
    fn drop(&mut self) {
        if !self.0.destroyed.get() {
            // SAFETY: the window belongs to this thread and has not been destroyed.
            unsafe { DestroyWindow(self.0.hwnd) }.expect("a live Nifty window can be destroyed");
        }
    }
}

/// What a child control reports, run by the window procedure on its notifications.
pub(crate) enum Events {
    None,
    Click(ClickHandler),
    Toggle(ToggleHandler),
    Change(TextHandler),
    /// A slider's moves, and the value it last showed, so a drag within one value reports
    /// nothing new.
    Slide {
        on_change: NumberHandler,
        last: Cell<i64>,
    },
    /// The option picked from a picker's menu.
    Pick(TextHandler),
}

/// How the window paints a control in its palette.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Paint {
    Text(TextColor),
    Divider,
    Field,
    /// Drawn entirely by Nifty, through custom draw.
    Button(ButtonStyle, Icon),
    Checkbox,
    Switch,
    NavItem(Icon),
    /// Its track and thumb drawn by Nifty, each through custom draw.
    Slider,
    Picker,
    /// Drawn by the system's theme, on the window's own background.
    Progress,
    /// Drawn by Nifty on its window's behalf, as an owner-drawn static control.
    Image,
}

/// What a control shows besides its window text, for the ones Nifty draws.
#[derive(Debug)]
pub(crate) enum Content {
    /// Its window text, if anything.
    Text,
    /// A nav item's selection and badge; its window text is its title.
    Nav {
        selected: Cell<bool>,
        badge: RefCell<String>,
    },
    /// A picker's options, and the one it shows, which is its window text, or none.
    Options(RefCell<Vec<String>>),
    /// An image, or none when its file can't be read, and the size it is shown at.
    Image {
        picture: RefCell<Option<Picture>>,
        size: ImageSize,
    },
}

/// A child control as its window sees it.
pub(crate) struct Control {
    pub(crate) hwnd: HWND,
    pub(crate) font: TextStyle,
    pub(crate) paint: Paint,
    pub(crate) events: Events,
    /// Set while code changes a text field's value, which Win32 reports like an edit.
    pub(crate) changing_in_code: Cell<bool>,
    pub(crate) content: Content,
}

impl Control {
    fn notify(&self, code: u32) {
        match &self.events {
            Events::Click(on_click) if code == BN_CLICKED => on_click(),
            Events::Toggle(on_toggle) if code == BN_CLICKED => on_toggle(is_checked(self.hwnd)),
            Events::Change(on_change) if code == EN_CHANGE && !self.changing_in_code.get() => {
                on_change(&typed_text(self.hwnd));
            }
            Events::Pick(on_pick) if code == BN_CLICKED => self.pick(on_pick),
            _ => {}
        }
    }

    /// Shows a picker's menu under it, and reports the option the user picks. The
    /// options are copied first, since the menu runs its own loop.
    fn pick(&self, on_pick: &TextHandler) {
        let Content::Options(options) = &self.content else {
            return;
        };
        let options = options.borrow().clone();
        let shown = read_text(self.hwnd);
        if let Some(picked) = menu::choose(self.hwnd, &options, &shown) {
            on_pick(&options[picked]);
        }
    }

    /// Reports a slider's new value, once it differs from the one it showed.
    fn slid(&self) {
        if let Events::Slide { on_change, last } = &self.events {
            let value = slider_position(self.hwnd);
            if last.replace(value) != value {
                on_change(value);
            }
        }
    }
}

impl fmt::Debug for Control {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Control")
            .field("hwnd", &self.hwnd)
            .field("font", &self.font)
            .field("paint", &self.paint)
            .finish_non_exhaustive()
    }
}

pub(crate) struct WindowState {
    hwnd: HWND,
    on_resize: ResizeHandler,
    scale: Cell<Scale>,
    settings: Cell<Settings>,
    theme: RefCell<Theme>,
    right_to_left: bool,
    fonts: RefCell<Fonts>,
    reported_size: Cell<Size>,
    controls: RefCell<HashMap<u16, Rc<Control>>>,
    next_id: Cell<u16>,
    surfaces: Surfaces,
    /// The narrowest the content area may be, in DIPs.
    min_content_width: Cell<f32>,
    backdrop: Cell<Backdrop>,
    tooltips: Tooltips,
    /// Primary buttons in the order they were made; the last one still shown is the button
    /// Return presses.
    primary_buttons: RefCell<Vec<u16>>,
    /// The control that had the keyboard focus when the window was last deactivated.
    focus: Cell<Option<HWND>>,
    shown: Cell<bool>,
    /// Shared by every window; the app quits when the last shown window is destroyed.
    shown_windows: Rc<Cell<usize>>,
    destroyed: Cell<bool>,
}

impl fmt::Debug for WindowState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowState")
            .field("hwnd", &self.hwnd)
            .field("scale", &self.scale)
            .field("destroyed", &self.destroyed)
            .finish_non_exhaustive()
    }
}

impl WindowState {
    /// The window starts hidden, and reports resizes only once it is fully set up.
    pub(crate) fn open(
        instance: HINSTANCE,
        title: &str,
        size: Size,
        on_resize: ResizeHandler,
        shown_windows: Rc<Cell<usize>>,
    ) -> Result<WinWindow> {
        let title = wide(title);
        // SAFETY: the class is registered by `WinBackend::new` and the title is NUL-terminated.
        let hwnd = unsafe {
            CreateWindowExW(
                EX_STYLE,
                WINDOW_CLASS,
                PCWSTR(title.as_ptr()),
                STYLE,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                None,
                None,
                Some(instance),
                None,
            )?
        };
        let settings = Settings::current();
        let (scale, fonts, theme, reported_size) = match prepare(hwnd, size, settings) {
            Ok(prepared) => prepared,
            Err(error) => {
                // SAFETY: the window was just created on this thread and nothing else has it.
                unsafe { DestroyWindow(hwnd)? };
                return Err(error);
            }
        };
        let state = Rc::new(Self {
            hwnd,
            on_resize,
            scale: Cell::new(scale),
            settings: Cell::new(settings),
            theme: RefCell::new(theme),
            right_to_left: reads_right_to_left(),
            fonts: RefCell::new(fonts),
            reported_size: Cell::new(reported_size),
            focus: Cell::new(None),
            controls: RefCell::new(HashMap::new()),
            next_id: Cell::new(FIRST_CONTROL_ID),
            surfaces: Surfaces::default(),
            min_content_width: Cell::new(0.0),
            backdrop: Cell::new(Backdrop::default()),
            tooltips: Tooltips::default(),
            primary_buttons: RefCell::new(Vec::new()),
            shown: Cell::new(false),
            shown_windows,
            destroyed: Cell::new(false),
        });
        OPEN_WINDOWS.with_borrow_mut(|open| open.insert(hwnd_key(hwnd), Rc::clone(&state)));
        Ok(WinWindow(state))
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(crate) fn scale(&self) -> Scale {
        self.scale.get()
    }

    pub(crate) fn is_destroyed(&self) -> bool {
        self.destroyed.get()
    }

    pub(crate) fn reads_right_to_left(&self) -> bool {
        self.right_to_left
    }

    pub(crate) fn font(&self, style: TextStyle) -> HFONT {
        self.fonts.borrow().get(style)
    }

    pub(crate) fn content_size(&self) -> Size {
        content_size(self.hwnd, self.scale()).expect("a live window has a client area")
    }

    /// Centers the window on its monitor's work area and shows it.
    pub(crate) fn show(&self) {
        center_on_monitor(self.hwnd).expect("a live window can be moved");
        if !self.shown.replace(true) {
            self.shown_windows.set(self.shown_windows.get() + 1);
        }
        // SAFETY: `hwnd` is a live window on this thread.
        let _was_visible = unsafe { ShowWindow(self.hwnd, SW_SHOW) };
    }

    pub(crate) fn set_title(&self, title: &str) {
        let title = wide(title);
        // SAFETY: the title is NUL-terminated and outlives the call.
        unsafe { SetWindowTextW(self.hwnd, PCWSTR(title.as_ptr())) }
            .expect("a live window can be retitled");
    }

    /// Win32 sends `WM_SIZE` from inside this call, which reports the new size.
    pub(crate) fn set_content_size(&self, size: Size) {
        resize_content(self.hwnd, self.scale(), size).expect("a live window can be resized");
    }

    /// The next id not used by a live control. Ids are 16 bits in `WM_COMMAND`, so they
    /// are reused once controls are removed.
    pub(crate) fn allocate_id(&self) -> u16 {
        let controls = self.controls.borrow();
        let mut id = self.next_id.get();
        while controls.contains_key(&id) {
            id = id.checked_add(1).unwrap_or(FIRST_CONTROL_ID);
        }
        self.next_id
            .set(id.checked_add(1).unwrap_or(FIRST_CONTROL_ID));
        id
    }

    /// Added after the control is created, since creating it sends messages that read the map.
    pub(crate) fn add_control(&self, id: u16, control: Rc<Control>) {
        self.controls.borrow_mut().insert(id, control);
    }

    /// The control's events stop now, but a handler already running keeps its `Rc` alive.
    pub(crate) fn remove_control(&self, id: u16) {
        self.controls.borrow_mut().remove(&id);
        self.primary_buttons
            .borrow_mut()
            .retain(|&primary| primary != id);
    }

    /// The surfaces painted behind the controls straight in the window.
    pub(crate) fn surfaces(&self) -> &Surfaces {
        &self.surfaces
    }

    /// The palette the window paints in now, and its brushes.
    pub(crate) fn theme(&self) -> Ref<'_, Theme> {
        self.theme.borrow()
    }

    pub(crate) fn tooltips(&self) -> &Tooltips {
        &self.tooltips
    }

    pub(crate) fn set_backdrop(&self, backdrop: Backdrop) {
        self.backdrop.set(backdrop);
        apply_backdrop(self.hwnd, backdrop, self.settings.get().appearance);
    }

    pub(crate) fn set_min_content_width(&self, width: f32) {
        self.min_content_width.set(width);
    }

    /// The topmost surface under the middle of `child`, which is what the control sits on:
    /// one its host paints, or else what its host sits on.
    pub(crate) fn surface_under(&self, child: HWND) -> Surface {
        // SAFETY: `child` is a live child window of this window.
        let host = unsafe { GetParent(child) }.expect("a control has a parent");
        let mut rect = RECT::default();
        // SAFETY: `child` is a live child window, and `rect` a valid RECT.
        unsafe { GetWindowRect(child, &raw mut rect) }.expect("a live control has a rect");
        let mut corners = [
            POINT {
                x: rect.left,
                y: rect.top,
            },
            POINT {
                x: rect.right,
                y: rect.bottom,
            },
        ];
        // SAFETY: the points are in screen coordinates, mapped into the host's client area.
        unsafe { MapWindowPoints(None, Some(host), &mut corners) };
        let middle = POINT {
            x: i32::midpoint(corners[0].x, corners[1].x),
            y: i32::midpoint(corners[0].y, corners[1].y),
        };
        if host == self.hwnd {
            return self.surfaces.at(middle).unwrap_or(Surface::None);
        }
        scroll_area(host).map_or(Surface::None, |area| {
            area.surface_at(middle)
                .unwrap_or_else(|| self.surface_under(area.hwnd()))
        })
    }

    /// Answers `WM_GETMINMAXINFO`: the window may not get narrower than its content.
    fn limit_size(&self, limits: &mut MINMAXINFO) {
        let scale = self.scale();
        let mut outer = RECT {
            right: scale.to_pixels(self.min_content_width.get()),
            ..RECT::default()
        };
        // SAFETY: `outer` is a valid RECT to adjust in place.
        unsafe { AdjustWindowRectExForDpi(&raw mut outer, STYLE, false, EX_STYLE, scale.dpi()) }
            .expect("Windows can size a frame around a content area");
        limits.ptMinTrackSize.x = limits.ptMinTrackSize.x.max(rect_size(&outer).0);
    }

    /// Return presses the newest primary button, through the dialog manager's `DM_GETDEFID`.
    pub(crate) fn add_primary_button(&self, id: u16) {
        self.primary_buttons.borrow_mut().push(id);
    }

    fn default_button(&self) -> Option<u16> {
        self.primary_buttons.borrow().last().copied()
    }

    fn control(&self, id: u16) -> Option<Rc<Control>> {
        self.controls.borrow().get(&id).cloned()
    }

    fn is_child(&self, hwnd: HWND) -> bool {
        // SAFETY: `IsChild` accepts any window handle.
        !hwnd.is_invalid() && unsafe { IsChild(self.hwnd, hwnd) }.as_bool()
    }

    fn report_size(&self, always: bool) {
        let size = self.content_size();
        let previous = self.reported_size.replace(size);
        if always || previous != size {
            (self.on_resize)(size);
        }
    }

    /// New fonts and a new window size for the monitor the window moved to; the layout then
    /// re-places every control at the new scale.
    fn change_dpi(&self, dpi: u32, suggested: &RECT) {
        self.scale.set(Scale::new(dpi));
        self.refresh_fonts();
        let (width, height) = rect_size(suggested);
        // SAFETY: `hwnd` is a live window on this thread.
        unsafe {
            SetWindowPos(
                self.hwnd,
                None,
                suggested.left,
                suggested.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .expect("a live window can be moved");
        self.report_size(true);
    }

    /// Remakes every control's font at the window's scale and the user's text size.
    fn refresh_fonts(&self) {
        let fonts = Fonts::new(self.scale(), self.settings.get().text_size_percent)
            .expect("the system font exists at every size");
        let controls: Vec<Rc<Control>> = self.controls.borrow().values().cloned().collect();
        for control in controls {
            set_font(control.hwnd, fonts.get(control.font));
        }
        *self.fonts.borrow_mut() = fonts;
    }

    /// Follows a change to the appearance, accent color or text size: repaints in the new
    /// palette, and re-lays out when text changed size.
    fn change_settings(&self) {
        let settings = Settings::current();
        let previous = self.settings.replace(settings);
        if previous.text_size_percent != settings.text_size_percent {
            self.refresh_fonts();
            self.report_size(true);
        }
        // High contrast themes change their colors without changing the settings.
        let palette = Palette::of(&settings);
        if palette == self.theme.borrow().palette {
            return;
        }
        *self.theme.borrow_mut() = Theme::new(palette).expect("Windows can create solid brushes");
        follow_appearance_in_title_bar(self.hwnd, settings.appearance);
        apply_backdrop(self.hwnd, self.backdrop.get(), settings.appearance);
        // SAFETY: `hwnd` is a live window on this thread.
        let _redrawn = unsafe {
            RedrawWindow(
                Some(self.hwnd),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_ALLCHILDREN,
            )
        };
    }

    /// Answers `WM_CTLCOLOR*`: the control's text colors, and the brush for its background.
    fn color_control(&self, child: HWND, dc: HDC) -> Option<LRESULT> {
        // SAFETY: `child` is the control asking to be painted.
        let id = u16::try_from(unsafe { GetDlgCtrlID(child) }).ok()?;
        let paint = self.control(id)?.paint;
        let surface = self.surface_under(child);
        let theme = self.theme.borrow();
        let palette = &theme.palette;
        let on_surface = (palette.surface(surface), theme.surface_brush(surface));
        let (text, (background, brush)) = match paint {
            Paint::Text(color) => (palette.text_color(color), on_surface),
            Paint::Field => (palette.text, (palette.field, theme.field.get())),
            Paint::Divider => (palette.text, (palette.divider, theme.divider.get())),
            Paint::Button(..)
            | Paint::Checkbox
            | Paint::Switch
            | Paint::NavItem(_)
            | Paint::Slider
            | Paint::Picker
            | Paint::Progress
            | Paint::Image => (palette.text, on_surface),
        };
        // SAFETY: `dc` is the control's device context for this paint.
        unsafe {
            SetTextColor(dc, text);
            SetBkColor(dc, background);
        }
        Some(LRESULT(brush.0.expose_provenance().cast_signed()))
    }

    /// Paints buttons, checkboxes, switches, nav items and pickers at the start of their
    /// custom draw, and a slider's track and thumb as each is drawn, and tells the control
    /// to skip its own painting.
    fn custom_draw(&self, draw: &NMCUSTOMDRAW) -> Option<LRESULT> {
        let control = self.control(u16::try_from(draw.hdr.idFrom).ok()?)?;
        let slider = matches!(control.paint, Paint::Slider);
        let flags = match draw.dwDrawStage {
            CDDS_PREPAINT if slider => CDRF_NOTIFYITEMDRAW,
            CDDS_PREPAINT => {
                self.paint_control(&control, draw)?;
                CDRF_SKIPDEFAULT
            }
            CDDS_ITEMPREPAINT if slider => {
                self.paint_slider_part(&control, draw);
                CDRF_SKIPDEFAULT
            }
            _ => CDRF_DODEFAULT,
        };
        Some(custom_draw_result(flags))
    }

    /// Paints the whole of a control Nifty draws, or answers `None` for one it doesn't.
    fn paint_control(&self, control: &Control, draw: &NMCUSTOMDRAW) -> Option<()> {
        let theme = self.theme.borrow();
        let mut title: Vec<u16> = read_text(control.hwnd).encode_utf16().collect();
        let mut canvas = self.canvas(control, draw, &theme.palette, &mut title);
        let state = State(draw.uItemState);
        match (control.paint, &control.content) {
            (Paint::Button(style, icon), _) => {
                draw_button(&mut canvas, (style, icon), is_checked(control.hwnd), state);
            }
            (Paint::Checkbox, _) => draw_checkbox(&mut canvas, is_checked(control.hwnd), state),
            (Paint::Switch, _) => draw_switch(&mut canvas, is_checked(control.hwnd), state),
            (Paint::NavItem(icon), Content::Nav { selected, badge }) => {
                let mut badge: Vec<u16> = badge.borrow().encode_utf16().collect();
                draw_nav_item(&mut canvas, icon, &mut badge, selected.get(), state);
            }
            (Paint::Picker, _) => draw_picker(&mut canvas, state),
            _ => return None,
        }
        Some(())
    }

    /// Paints a slider's track, filled up to the middle of its thumb, or its thumb.
    fn paint_slider_part(&self, control: &Control, draw: &NMCUSTOMDRAW) {
        let theme = self.theme.borrow();
        let canvas = self.canvas(control, draw, &theme.palette, &mut []);
        match u32::try_from(draw.dwItemSpec) {
            Ok(TBCD_CHANNEL) => {
                let thumb = thumb_rect(control.hwnd);
                draw_slider_track(&canvas, i32::midpoint(thumb.left, thumb.right));
            }
            Ok(TBCD_THUMB) => draw_slider_thumb(&canvas, State(draw.uItemState)),
            _ => {}
        }
    }

    /// Draws an image control on the surface under it.
    fn draw_item(&self, item: &DRAWITEMSTRUCT) -> Option<()> {
        let control = self.control(u16::try_from(item.CtlID).ok()?)?;
        let Content::Image { picture, .. } = &control.content else {
            return None;
        };
        let theme = self.theme.borrow();
        let backdrop = theme.palette.surface(self.surface_under(control.hwnd));
        draw_image(
            item.hDC,
            item.rcItem,
            backdrop,
            picture.borrow().as_ref(),
            self.scale(),
        );
        Some(())
    }

    fn canvas<'a>(
        &self,
        control: &Control,
        draw: &NMCUSTOMDRAW,
        palette: &'a Palette,
        title: &'a mut [u16],
    ) -> Canvas<'a> {
        Canvas {
            dc: draw.hdc,
            rect: draw.rc,
            backdrop: palette.surface(self.surface_under(control.hwnd)),
            font: self.font(control.font),
            icon_font: self.fonts.borrow().icons(),
            title,
            palette,
            scale: self.scale(),
        }
    }

    fn erase_background(&self, dc: HDC) -> LRESULT {
        let mut client = RECT::default();
        let theme = self.theme.borrow();
        // SAFETY: `client` is a valid RECT to write into, and `dc` is this window's DC
        // for the erase.
        unsafe {
            GetClientRect(self.hwnd, &raw mut client).expect("a live window has a client area");
            FillRect(dc, &raw const client, theme.surface_brush(Surface::None));
        }
        self.surfaces.paint(dc, 0, &theme.palette, self.scale());
        LRESULT(1)
    }

    /// Answers what a control in one of the window's scroll areas told the area, as if
    /// the control were straight in the window.
    pub(crate) fn handle_from_child(
        &self,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> Option<LRESULT> {
        match message {
            WM_COMMAND | WM_NOTIFY | WM_HSCROLL | WM_DRAWITEM | WM_CTLCOLORSTATIC
            | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => self.handle(message, wparam, lparam),
            _ => None,
        }
    }

    fn handle(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_COMMAND => {
                // Clone the control out first: its handler may add or remove controls.
                let control = self.control(low_word(wparam.0))?;
                control.notify(u32::from(high_word(wparam.0)));
                Some(LRESULT(0))
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
                self.color_control(hwnd_param(lparam), dc_param(wparam))
            }
            WM_DRAWITEM => {
                // SAFETY: for WM_DRAWITEM, `lparam` points to the DRAWITEMSTRUCT to draw.
                let item = unsafe {
                    &*std::ptr::with_exposed_provenance::<DRAWITEMSTRUCT>(lparam.0.cast_unsigned())
                };
                self.draw_item(item)?;
                Some(LRESULT(1))
            }
            // A slider moved; its `LPARAM` is the slider.
            WM_HSCROLL => {
                // SAFETY: the slider is a live child of this window.
                let id = u16::try_from(unsafe { GetDlgCtrlID(hwnd_param(lparam)) }).ok()?;
                self.control(id)?.slid();
                Some(LRESULT(0))
            }
            WM_NOTIFY => {
                // SAFETY: WM_NOTIFY's `lparam` points to an NMHDR that starts the
                // notification's own struct, which for NM_CUSTOMDRAW is an NMCUSTOMDRAW.
                let header = unsafe {
                    &*std::ptr::with_exposed_provenance::<NMHDR>(lparam.0.cast_unsigned())
                };
                if header.code != NM_CUSTOMDRAW {
                    return None;
                }
                // SAFETY: as above; the code says the full struct is an NMCUSTOMDRAW.
                let draw = unsafe {
                    &*std::ptr::with_exposed_provenance::<NMCUSTOMDRAW>(lparam.0.cast_unsigned())
                };
                self.custom_draw(draw)
            }
            WM_ERASEBKGND => Some(self.erase_background(dc_param(wparam))),
            WM_GETMINMAXINFO => {
                // SAFETY: for WM_GETMINMAXINFO, `lparam` points to the MINMAXINFO to fill in.
                let limits = unsafe {
                    &mut *std::ptr::with_exposed_provenance_mut::<MINMAXINFO>(
                        lparam.0.cast_unsigned(),
                    )
                };
                self.limit_size(limits);
                Some(LRESULT(0))
            }
            DM_GETDEFID => self.default_button().map(|id| {
                let id_and_flag = u32::from(id) | DC_HASDEFID << 16;
                LRESULT(isize::try_from(id_and_flag).expect("Nifty targets 64-bit Windows"))
            }),
            WM_SIZE if wparam.0 != usize::try_from(SIZE_MINIMIZED).expect("small constant") => {
                self.report_size(false);
                Some(LRESULT(0))
            }
            WM_DPICHANGED => {
                // SAFETY: for WM_DPICHANGED, `lparam` points to the suggested window RECT.
                let suggested = unsafe {
                    &*std::ptr::with_exposed_provenance::<RECT>(lparam.0.cast_unsigned())
                };
                self.change_dpi(u32::from(low_word(wparam.0)), suggested);
                Some(LRESULT(0))
            }
            WM_SETTINGCHANGE | WM_SYSCOLORCHANGE => {
                self.change_settings();
                None
            }
            // Dialogs keep the focused control across deactivation; plain windows need to
            // do it themselves.
            WM_ACTIVATE if u32::from(low_word(wparam.0)) == WA_INACTIVE => {
                // SAFETY: `GetFocus` has no preconditions.
                let focused = unsafe { GetFocus() };
                self.focus.set(self.is_child(focused).then_some(focused));
                None
            }
            WM_SETFOCUS => {
                let saved = self.focus.get().filter(|&control| self.is_child(control))?;
                // SAFETY: `saved` is a live child of this window.
                unsafe { SetFocus(Some(saved)) }.ok()?;
                Some(LRESULT(0))
            }
            WM_DESTROY => {
                if self.shown.get() {
                    let remaining = self.shown_windows.get() - 1;
                    self.shown_windows.set(remaining);
                    if remaining == 0 {
                        // SAFETY: posting WM_QUIT to this thread's queue has no preconditions.
                        unsafe { PostQuitMessage(0) };
                    }
                }
                Some(LRESULT(0))
            }
            WM_NCDESTROY => {
                self.destroyed.set(true);
                OPEN_WINDOWS.with_borrow_mut(|open| open.remove(&hwnd_key(self.hwnd)));
                None
            }
            _ => None,
        }
    }
}

/// The window procedure of every Nifty window.
pub(crate) unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Messages sent while `CreateWindowExW` runs arrive before the window is registered.
    let state = OPEN_WINDOWS.with_borrow(|open| open.get(&hwnd_key(hwnd)).cloned());
    state
        .and_then(|state| state.handle(message, wparam, lparam))
        // SAFETY: the parameters are the ones Win32 passed to this window procedure.
        .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}

/// The Nifty window `hwnd` is in, which `IsDialogMessageW` needs for Tab and Return.
pub(crate) fn nifty_root(hwnd: HWND) -> Option<HWND> {
    if hwnd.is_invalid() {
        return None;
    }
    // SAFETY: `GetAncestor` accepts any window handle.
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    OPEN_WINDOWS
        .with_borrow(|open| open.contains_key(&hwnd_key(root)))
        .then_some(root)
}

/// The window's scale, fonts, theme and content size, worked out once Win32 has placed it
/// on a monitor.
fn prepare(hwnd: HWND, size: Size, settings: Settings) -> Result<(Scale, Fonts, Theme, Size)> {
    // SAFETY: `hwnd` is a live window on this thread.
    let scale = Scale::new(unsafe { GetDpiForWindow(hwnd) });
    resize_content(hwnd, scale, size)?;
    let fonts = Fonts::new(scale, settings.text_size_percent)?;
    let theme = Theme::new(Palette::of(&settings))?;
    follow_appearance_in_title_bar(hwnd, settings.appearance);
    Ok((scale, fonts, theme, content_size(hwnd, scale)?))
}

/// Windows 10 before version 2004 refuses the attribute, and its title bars stay light.
fn follow_appearance_in_title_bar(hwnd: HWND, appearance: Appearance) {
    let dark = BOOL::from(appearance == Appearance::Dark);
    let _refused_before_windows_10_2004 =
        set_window_attribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark);
}

/// Windows 11 before 22H2, and Windows 10, refuse the attribute, and their frames stay
/// solid, which is the documented fallback.
fn apply_backdrop(hwnd: HWND, backdrop: Backdrop, appearance: Appearance) {
    let _refused_before_windows_11_22h2 = set_window_attribute(
        hwnd,
        DWMWA_SYSTEMBACKDROP_TYPE,
        &system_backdrop(backdrop, appearance),
    );
}

/// The material DWM draws the frame in. High contrast themes allow only their own colors.
fn system_backdrop(backdrop: Backdrop, appearance: Appearance) -> DWM_SYSTEMBACKDROP_TYPE {
    match (backdrop, appearance) {
        (_, Appearance::HighContrast) | (Backdrop::Solid, _) => DWMSBT_NONE,
        (Backdrop::Standard, _) => DWMSBT_MAINWINDOW,
        (Backdrop::Translucent, _) => DWMSBT_TRANSIENTWINDOW,
    }
}

fn set_window_attribute<T>(hwnd: HWND, attribute: DWMWINDOWATTRIBUTE, value: &T) -> Result<()> {
    // SAFETY: each attribute Nifty sets takes a value of the type passed, with its size.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute,
            std::ptr::from_ref(value).cast(),
            u32::try_from(mem::size_of::<T>()).expect("attribute values are small"),
        )
    }
}

fn custom_draw_result(flags: u32) -> LRESULT {
    LRESULT(isize::try_from(flags).expect("custom draw flags are small"))
}

fn content_size(hwnd: HWND, scale: Scale) -> Result<Size> {
    let mut client = RECT::default();
    // SAFETY: `client` is a valid RECT to write into.
    unsafe { GetClientRect(hwnd, &raw mut client)? };
    let (width, height) = rect_size(&client);
    Ok(scale.size_to_dips(width, height))
}

fn resize_content(hwnd: HWND, scale: Scale, size: Size) -> Result<()> {
    let mut outer = RECT {
        left: 0,
        top: 0,
        right: scale.to_pixels(size.width),
        bottom: scale.to_pixels(size.height),
    };
    // SAFETY: `outer` is a valid RECT to adjust in place.
    unsafe { AdjustWindowRectExForDpi(&raw mut outer, STYLE, false, EX_STYLE, scale.dpi())? };
    let (width, height) = rect_size(&outer);
    // SAFETY: `hwnd` is a live window on this thread.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
}

fn center_on_monitor(hwnd: HWND) -> Result<()> {
    let mut window = RECT::default();
    // SAFETY: `window` is a valid RECT to write into.
    unsafe { GetWindowRect(hwnd, &raw mut window)? };
    let mut monitor = MONITORINFO {
        cbSize: u32::try_from(mem::size_of::<MONITORINFO>()).expect("MONITORINFO is small"),
        ..Default::default()
    };
    // SAFETY: `monitor.cbSize` matches the struct passed.
    unsafe {
        GetMonitorInfoW(
            MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST),
            &raw mut monitor,
        )
    }
    .ok()?;
    let (width, height) = rect_size(&window);
    let (work_width, work_height) = rect_size(&monitor.rcWork);
    // SAFETY: `hwnd` is a live window on this thread.
    unsafe {
        SetWindowPos(
            hwnd,
            None,
            monitor.rcWork.left + (work_width - width) / 2,
            monitor.rcWork.top + (work_height - height) / 2,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_backdrop_picks_its_material() {
        assert_eq!(
            system_backdrop(Backdrop::Standard, Appearance::Light),
            DWMSBT_MAINWINDOW
        );
        assert_eq!(
            system_backdrop(Backdrop::Translucent, Appearance::Dark),
            DWMSBT_TRANSIENTWINDOW
        );
        assert_eq!(
            system_backdrop(Backdrop::Solid, Appearance::Light),
            DWMSBT_NONE
        );
    }

    #[test]
    fn high_contrast_drops_every_material() {
        for &backdrop in Backdrop::ALL {
            assert_eq!(
                system_backdrop(backdrop, Appearance::HighContrast),
                DWMSBT_NONE
            );
        }
    }
}
