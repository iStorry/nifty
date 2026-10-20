//! Top-level windows, and the window procedure that routes their child controls' events.

use std::cell::{Cell, RefCell};
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
    CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_SKIPDEFAULT, NM_CUSTOMDRAW, NMCUSTOMDRAW, NMHDR,
};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::{
    BN_CLICKED, CW_USEDEFAULT, CreateWindowExW, DC_HASDEFID, DM_GETDEFID, DefWindowProcW,
    DestroyWindow, EN_CHANGE, GA_ROOT, GetAncestor, GetClientRect, GetDlgCtrlID, GetWindowRect,
    IsChild, MINMAXINFO, PostQuitMessage, SIZE_MINIMIZED, SW_SHOW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetWindowPos, SetWindowTextW, ShowWindow, WA_INACTIVE,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLOREDIT,
    WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_NCDESTROY,
    WM_NOTIFY, WM_SETFOCUS, WM_SETTINGCHANGE, WM_SIZE, WM_SYSCOLORCHANGE, WS_OVERLAPPEDWINDOW,
};
use windows::core::{BOOL, PCWSTR, Result, w};

use nifty_runtime::{
    Backdrop, ButtonStyle, ClickHandler, Frame, Icon, ResizeHandler, Size, Surface, TextColor,
    TextHandler, TextStyle, ToggleHandler,
};

use crate::convert::{Scale, high_word, low_word, rect_size, wide};
use crate::draw::{Canvas, State, draw_button, draw_checkbox, draw_nav_item, paint_surface};
use crate::fonts::Fonts;
use crate::palette::{Palette, Theme};
use crate::settings::{Appearance, Settings, reads_right_to_left};
use crate::tooltips::Tooltips;
use crate::widgets::{is_checked, read_text, set_font};

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
    NavItem(Icon),
}

/// What a nav item shows besides its title, which its control keeps as window text.
#[derive(Debug, Default)]
pub(crate) struct NavState {
    pub(crate) selected: Cell<bool>,
    pub(crate) badge: RefCell<String>,
}

/// A child control as its window sees it.
pub(crate) struct Control {
    pub(crate) hwnd: HWND,
    pub(crate) font: TextStyle,
    pub(crate) paint: Paint,
    pub(crate) events: Events,
    /// Set while code changes a text field's value, which Win32 reports like an edit.
    pub(crate) changing_in_code: Cell<bool>,
    /// Present for nav items only.
    pub(crate) nav: Option<NavState>,
}

impl Control {
    fn notify(&self, code: u32) {
        match &self.events {
            Events::Click(on_click) if code == BN_CLICKED => on_click(),
            Events::Toggle(on_toggle) if code == BN_CLICKED => on_toggle(is_checked(self.hwnd)),
            Events::Change(on_change) if code == EN_CHANGE && !self.changing_in_code.get() => {
                on_change(&read_text(self.hwnd));
            }
            _ => {}
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

/// A surface the window paints behind its controls, in window pixels.
#[derive(Clone, Copy, Debug)]
struct PaintedSurface {
    id: u32,
    surface: Surface,
    rect: RECT,
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
    /// In the order they were created, which is the order they are painted in.
    surfaces: RefCell<Vec<PaintedSurface>>,
    next_surface_id: Cell<u32>,
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
            surfaces: RefCell::new(Vec::new()),
            next_surface_id: Cell::new(0),
            min_content_width: Cell::new(0.0),
            backdrop: Cell::new(Backdrop::default()),
            tooltips: Tooltips::default(),
            primary_buttons: RefCell::new(Vec::new()),
            shown: Cell::new(false),
            shown_windows,
            destroyed: Cell::new(false),
        });
        OPEN_WINDOWS.with_borrow_mut(|open| open.insert(key(hwnd), Rc::clone(&state)));
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

    /// Adds `surface` above the surfaces already painted, with no area until it is placed.
    pub(crate) fn add_surface(&self, surface: Surface) -> u32 {
        let id = self.next_surface_id.get();
        self.next_surface_id.set(id.wrapping_add(1));
        self.surfaces.borrow_mut().push(PaintedSurface {
            id,
            surface,
            rect: RECT::default(),
        });
        id
    }

    pub(crate) fn place_surface(&self, id: u32, frame: Frame) {
        let rect = self.scale().frame_to_pixels(frame);
        let previous = self.with_surface(id, |painted| mem::replace(&mut painted.rect, rect));
        if previous == rect {
            return;
        }
        self.repaint(&previous);
        self.repaint(&rect);
    }

    pub(crate) fn remove_surface(&self, id: u32) {
        let removed = self.with_surface(id, |painted| painted.rect);
        self.surfaces
            .borrow_mut()
            .retain(|painted| painted.id != id);
        self.repaint(&removed);
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

    fn with_surface<R>(&self, id: u32, change: impl FnOnce(&mut PaintedSurface) -> R) -> R {
        let mut surfaces = self.surfaces.borrow_mut();
        let painted = surfaces
            .iter_mut()
            .find(|painted| painted.id == id)
            .expect("surfaces are only used until they are removed");
        change(painted)
    }

    /// Repaints `rect` and the controls over it, whose backgrounds may have changed.
    fn repaint(&self, rect: &RECT) {
        // SAFETY: `hwnd` is a live window on this thread, and `rect` a valid RECT.
        let _redrawn = unsafe {
            RedrawWindow(
                Some(self.hwnd),
                Some(rect),
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
            )
        };
    }

    /// The topmost surface under the middle of `child`, which is what the control sits on.
    fn surface_under(&self, child: HWND) -> Surface {
        let mut rect = RECT::default();
        // SAFETY: `child` is a live control of this window, and `rect` a valid RECT.
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
        // SAFETY: the points are in screen coordinates, mapped into this window's client area.
        unsafe { MapWindowPoints(None, Some(self.hwnd), &mut corners) };
        let middle = POINT {
            x: i32::midpoint(corners[0].x, corners[1].x),
            y: i32::midpoint(corners[0].y, corners[1].y),
        };
        self.surfaces
            .borrow()
            .iter()
            .rev()
            .find(|painted| contains(&painted.rect, middle))
            .map_or(Surface::None, |painted| painted.surface)
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
            Paint::Button(..) | Paint::Checkbox | Paint::NavItem(_) => (palette.text, on_surface),
        };
        // SAFETY: `dc` is the control's device context for this paint.
        unsafe {
            SetTextColor(dc, text);
            SetBkColor(dc, background);
        }
        Some(LRESULT(brush.0.expose_provenance().cast_signed()))
    }

    /// Paints buttons and checkboxes at the start of their custom draw, and tells the
    /// control to skip its own painting.
    fn custom_draw(&self, draw: &NMCUSTOMDRAW) -> Option<LRESULT> {
        if draw.dwDrawStage != CDDS_PREPAINT {
            return Some(custom_draw_result(CDRF_DODEFAULT));
        }
        let control = self.control(u16::try_from(draw.hdr.idFrom).ok()?)?;
        let theme = self.theme.borrow();
        let mut title: Vec<u16> = read_text(control.hwnd).encode_utf16().collect();
        let backdrop = theme.palette.surface(self.surface_under(control.hwnd));
        let mut canvas = Canvas {
            dc: draw.hdc,
            rect: draw.rc,
            backdrop,
            font: self.font(control.font),
            icon_font: self.fonts.borrow().icons(),
            title: &mut title,
            palette: &theme.palette,
            scale: self.scale(),
        };
        let state = State(draw.uItemState);
        match control.paint {
            Paint::Button(style, icon) => draw_button(&mut canvas, style, icon, state),
            Paint::Checkbox => draw_checkbox(&mut canvas, is_checked(control.hwnd), state),
            Paint::NavItem(icon) => {
                let nav = control.nav.as_ref()?;
                let mut badge: Vec<u16> = nav.badge.borrow().encode_utf16().collect();
                draw_nav_item(&mut canvas, icon, &mut badge, nav.selected.get(), state);
            }
            Paint::Text(_) | Paint::Divider | Paint::Field => return None,
        }
        Some(custom_draw_result(CDRF_SKIPDEFAULT))
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
        for painted in self.surfaces.borrow().iter() {
            paint_surface(
                dc,
                painted.rect,
                painted.surface,
                &theme.palette,
                self.scale(),
            );
        }
        LRESULT(1)
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
                let child = HWND(std::ptr::with_exposed_provenance_mut(
                    lparam.0.cast_unsigned(),
                ));
                self.color_control(child, dc_param(wparam))
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
                OPEN_WINDOWS.with_borrow_mut(|open| open.remove(&key(self.hwnd)));
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
    let state = OPEN_WINDOWS.with_borrow(|open| open.get(&key(hwnd)).cloned());
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
        .with_borrow(|open| open.contains_key(&key(root)))
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

/// The device context a paint message passes in its `WPARAM`.
fn dc_param(wparam: WPARAM) -> HDC {
    HDC(std::ptr::with_exposed_provenance_mut(wparam.0))
}

fn contains(rect: &RECT, point: POINT) -> bool {
    (rect.left..rect.right).contains(&point.x) && (rect.top..rect.bottom).contains(&point.y)
}

fn key(hwnd: HWND) -> usize {
    hwnd.0.addr()
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
