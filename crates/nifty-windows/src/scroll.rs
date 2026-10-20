//! Scroll areas: child windows whose controls are their own children, which they move up
//! and down as the user scrolls. Controls report to their parent, so a scroll area passes
//! those reports on to its top-level window.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::fmt;
use std::mem;
use std::rc::Rc;

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    FillRect, HDC, RDW_ALLCHILDREN, RDW_ERASE, RDW_INVALIDATE, RedrawWindow,
};
use windows::Win32::UI::Controls::SetScrollInfo;
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, GetScrollInfo, SB_BOTTOM,
    SB_LINEDOWN, SB_LINEUP, SB_PAGEDOWN, SB_PAGEUP, SB_THUMBTRACK, SB_TOP, SB_VERT,
    SCROLLBAR_COMMAND, SCROLLINFO, SIF_DISABLENOSCROLL, SIF_PAGE, SIF_POS, SIF_RANGE, SIF_TRACKPOS,
    SM_CXVSCROLL, SPI_GETWHEELSCROLLLINES, SW_ERASE, SW_HIDE, SW_INVALIDATE, SW_SCROLLCHILDREN,
    SWP_NOACTIVATE, SWP_NOZORDER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, ScrollWindowEx,
    SetWindowPos, ShowWindow, SystemParametersInfoW, WHEEL_DELTA, WM_CONTEXTMENU, WM_ERASEBKGND,
    WM_MOUSEWHEEL, WM_NCDESTROY, WM_SIZE, WM_VSCROLL, WS_CHILD, WS_CLIPCHILDREN,
    WS_EX_CONTROLPARENT, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, Result, w};

use nifty_runtime::{Frame, Size, Surface};

use crate::areas::Surfaces;
use crate::context_menu::{self, MenuAreas};
use crate::convert::{Scale, dc_param, high_word, hwnd_key, low_word, rect_size};
use crate::window::WindowState;

pub(crate) const SCROLL_CLASS: PCWSTR = w!("NiftyScroll");
/// How far one line of a mouse wheel's notch scrolls, in DIPs: about a line of body text.
const WHEEL_LINE: f32 = 20.0;
/// How far the scroll bar's arrows scroll, in DIPs.
const ARROW_STEP: f32 = 40.0;
/// The lines a wheel notch scrolls when the user set it to scroll a page at a time; the
/// documented `WHEEL_PAGESCROLL`, which the `windows` crate doesn't define.
const WHEEL_PAGESCROLL: u32 = u32::MAX;

thread_local! {
    /// The scroll procedure finds an area's state here; entries leave on `WM_NCDESTROY`.
    static OPEN_SCROLLS: RefCell<HashMap<usize, Rc<ScrollArea>>> = RefCell::new(HashMap::new());
}

/// What a widget is placed in: its window's client area, or a scroll area's content.
#[derive(Clone, Debug)]
pub(crate) enum Host {
    Window,
    Scroll(Rc<ScrollArea>),
}

impl Host {
    pub(crate) fn hwnd(&self, window: &WindowState) -> HWND {
        match self {
            Self::Window => window.hwnd(),
            Self::Scroll(area) => area.hwnd,
        }
    }

    pub(crate) fn surfaces<'a>(&'a self, window: &'a WindowState) -> &'a Surfaces {
        match self {
            Self::Window => window.surfaces(),
            Self::Scroll(area) => &area.surfaces,
        }
    }

    pub(crate) fn context_menus<'a>(&'a self, window: &'a WindowState) -> &'a MenuAreas {
        match self {
            Self::Window => window.context_menus(),
            Self::Scroll(area) => &area.context_menus,
        }
    }

    /// How far the host's content is scrolled up, in pixels.
    fn offset(&self) -> i32 {
        match self {
            Self::Window => 0,
            Self::Scroll(area) => area.offset.get(),
        }
    }

    /// Whether the host is gone, and every control in it with it.
    pub(crate) fn is_destroyed(&self, window: &WindowState) -> bool {
        window.is_destroyed()
            || match self {
                Self::Window => false,
                Self::Scroll(area) => area.destroyed.get(),
            }
    }

    /// `frame`, in the host's content, as a rect in its client area's pixels.
    pub(crate) fn client_rect(&self, scale: Scale, frame: Frame) -> RECT {
        let content = scale.frame_to_pixels(frame);
        let offset = self.offset();
        RECT {
            top: content.top - offset,
            bottom: content.bottom - offset,
            ..content
        }
    }

    /// Places `child` at `frame` in the host's content.
    pub(crate) fn place(&self, child: HWND, scale: Scale, frame: Frame) {
        let rect = self.client_rect(scale, frame);
        let (width, height) = rect_size(&rect);
        // SAFETY: `child` is a live child window of this host, on this thread.
        unsafe {
            SetWindowPos(
                child,
                None,
                rect.left,
                rect.top,
                width,
                height,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
        }
        .expect("a live child window can be moved");
    }

    /// Moves surface `id` to `frame`, repainting where it was and where it is.
    pub(crate) fn place_surface(&self, window: &WindowState, id: u32, frame: Frame) {
        let rect = window.scale().frame_to_pixels(frame);
        if let Some(previous) = self.surfaces(window).place(id, rect) {
            self.repaint(window, previous);
            self.repaint(window, rect);
        }
    }

    pub(crate) fn remove_surface(&self, window: &WindowState, id: u32) {
        let removed = self.surfaces(window).remove(id);
        self.repaint(window, removed);
    }

    /// Repaints `content`, a rect of the host's content, and the controls over it, whose
    /// backgrounds may have changed.
    fn repaint(&self, window: &WindowState, content: RECT) {
        let offset = self.offset();
        let client = RECT {
            top: content.top - offset,
            bottom: content.bottom - offset,
            ..content
        };
        // SAFETY: the host is a live window on this thread, and `client` a valid RECT.
        let _redrawn = unsafe {
            RedrawWindow(
                Some(self.hwnd(window)),
                Some(&raw const client),
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
            )
        };
    }
}

/// A scroll area: a child window of its host, with a vertical scroll bar that always
/// shows, so the content's width doesn't change as it starts or stops scrolling.
pub(crate) struct ScrollArea {
    hwnd: HWND,
    window: Rc<WindowState>,
    host: Host,
    /// How far the content is scrolled up, in pixels.
    offset: Cell<i32>,
    content_height: Cell<i32>,
    /// What a wheel moved that didn't add up to a whole pixel yet, in wheel units.
    wheel_rest: Cell<i32>,
    surfaces: Surfaces,
    context_menus: MenuAreas,
    destroyed: Cell<bool>,
}

impl fmt::Debug for ScrollArea {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollArea")
            .field("hwnd", &self.hwnd)
            .field("offset", &self.offset)
            .field("content_height", &self.content_height)
            .finish_non_exhaustive()
    }
}

impl ScrollArea {
    /// A scroll area in `host`; it is placed, and given content, by the layout.
    pub(crate) fn create(
        instance: HINSTANCE,
        window: &Rc<WindowState>,
        host: Host,
    ) -> Result<Rc<Self>> {
        // SAFETY: the class is registered by `WinBackend::new`, and the parent is a live
        // window of this thread. The area starts at zero size until the layout places it.
        // `WS_EX_CONTROLPARENT` lets Tab move into its controls.
        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_CONTROLPARENT,
                SCROLL_CLASS,
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_VSCROLL | WS_CLIPCHILDREN,
                0,
                0,
                0,
                0,
                Some(host.hwnd(window)),
                None,
                Some(instance),
                None,
            )?
        };
        let area = Rc::new(Self {
            hwnd,
            window: Rc::clone(window),
            host,
            offset: Cell::new(0),
            content_height: Cell::new(0),
            wheel_rest: Cell::new(0),
            surfaces: Surfaces::default(),
            context_menus: MenuAreas::default(),
            destroyed: Cell::new(false),
        });
        OPEN_SCROLLS.with_borrow_mut(|open| open.insert(hwnd_key(hwnd), Rc::clone(&area)));
        area.update_bar();
        Ok(area)
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(crate) fn window(&self) -> &Rc<WindowState> {
        &self.window
    }

    pub(crate) fn place(&self, frame: Frame) {
        self.host.place(self.hwnd, self.window.scale(), frame);
    }

    /// Sets how tall the content is, scrolling back up if it got shorter than the area
    /// scrolled past.
    pub(crate) fn set_content(&self, size: Size) {
        self.content_height
            .set(self.window.scale().to_pixels(size.height));
        self.scroll_to(self.offset.get());
    }

    /// Hides the area now; `destroy` removes it once the event that removed it is over.
    pub(crate) fn retire(&self) {
        // SAFETY: `hwnd` is a live child window on this thread.
        let _was_visible = unsafe { ShowWindow(self.hwnd, SW_HIDE) };
    }

    /// Destroys the area, unless it went already with its window or the area holding it.
    pub(crate) fn destroy(&self) {
        if !self.destroyed.get() && !self.host.is_destroyed(&self.window) {
            // SAFETY: the area belongs to this thread and has not been destroyed.
            unsafe { DestroyWindow(self.hwnd) }.expect("a live scroll area can be destroyed");
        }
    }

    /// The surface under the point `client` of its client area, if one of its own is there.
    pub(crate) fn surface_at(&self, client: POINT) -> Option<Surface> {
        self.surfaces.topmost_at(POINT {
            x: client.x,
            y: client.y + self.offset.get(),
        })
    }

    /// The scroll bar's width at `scale`, in DIPs, which the content leaves room for.
    pub(crate) fn bar_width(scale: Scale) -> f32 {
        // SAFETY: `GetSystemMetricsForDpi` accepts every metric index.
        scale.to_dips(unsafe { GetSystemMetricsForDpi(SM_CXVSCROLL, scale.dpi()) })
    }

    fn page(&self) -> i32 {
        let mut client = RECT::default();
        // SAFETY: `client` is a valid RECT to write into.
        unsafe { GetClientRect(self.hwnd, &raw mut client) }
            .expect("a live scroll area has a client area");
        rect_size(&client).1
    }

    /// Scrolls so the content's `offset`th pixel row is at the top, as far as it goes.
    fn scroll_to(&self, offset: i32) {
        let furthest = (self.content_height.get() - self.page()).max(0);
        let offset = offset.clamp(0, furthest);
        let moved = self.offset.replace(offset) - offset;
        if moved != 0 {
            // SAFETY: `hwnd` is a live window on this thread; scrolling its children moves
            // the controls with the content.
            unsafe {
                ScrollWindowEx(
                    self.hwnd,
                    0,
                    moved,
                    None,
                    None,
                    None,
                    None,
                    SW_SCROLLCHILDREN | SW_INVALIDATE | SW_ERASE,
                );
            }
        }
        self.update_bar();
    }

    /// Shows the content's height, the visible part and where it is on the scroll bar,
    /// which stays shown, disabled, when there's nothing to scroll.
    fn update_bar(&self) {
        let info = SCROLLINFO {
            cbSize: scroll_info_size(),
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS | SIF_DISABLENOSCROLL,
            nMin: 0,
            nMax: (self.content_height.get() - 1).max(0),
            nPage: u32::try_from(self.page()).unwrap_or(0),
            nPos: self.offset.get(),
            nTrackPos: 0,
        };
        // SAFETY: `info` is a fully set SCROLLINFO, and `hwnd` a live window with a
        // vertical scroll bar.
        unsafe { SetScrollInfo(self.hwnd, SB_VERT, &raw const info, true) };
    }

    /// Answers a click or drag on the scroll bar.
    fn scroll_bar(&self, command: SCROLLBAR_COMMAND) {
        let scale = self.window.scale();
        let offset = self.offset.get();
        let target = match command {
            SB_LINEUP => offset - scale.to_pixels(ARROW_STEP),
            SB_LINEDOWN => offset + scale.to_pixels(ARROW_STEP),
            SB_PAGEUP => offset - self.page(),
            SB_PAGEDOWN => offset + self.page(),
            SB_TOP => 0,
            SB_BOTTOM => self.content_height.get(),
            SB_THUMBTRACK => self.track_position(),
            _ => return,
        };
        self.scroll_to(target);
    }

    fn track_position(&self) -> i32 {
        let mut info = SCROLLINFO {
            cbSize: scroll_info_size(),
            fMask: SIF_TRACKPOS,
            ..SCROLLINFO::default()
        };
        // SAFETY: `info` asks only for the track position, which Windows writes into it.
        unsafe { GetScrollInfo(self.hwnd, SB_VERT, &raw mut info) }
            .expect("a scroll area has a scroll bar");
        info.nTrackPos
    }

    /// Scrolls by the wheel's `delta`, as far a notch as the user set: some lines, or a
    /// page. A fine-grained wheel's small deltas add up until they make a pixel.
    fn wheel(&self, delta: i16) {
        let mut lines: u32 = 0;
        // SAFETY: SPI_GETWHEELSCROLLLINES writes a UINT through the pointer.
        unsafe {
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                Some((&raw mut lines).cast()),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        }
        .expect("Windows says how far a wheel notch scrolls");
        let per_notch = if lines == WHEEL_PAGESCROLL {
            self.page()
        } else {
            let line = self.window.scale().to_pixels(WHEEL_LINE);
            i32::try_from(lines).map_or(i32::MAX, |lines| lines.saturating_mul(line))
        };
        let notch = i32::try_from(WHEEL_DELTA).expect("WHEEL_DELTA is small");
        let moved = self
            .wheel_rest
            .get()
            .saturating_add(i32::from(delta).saturating_mul(per_notch));
        self.wheel_rest.set(moved % notch);
        self.scroll_to(self.offset.get().saturating_sub(moved / notch));
    }

    /// Fills the area with the surface it sits on, then paints its own surfaces.
    fn erase_background(&self, dc: HDC) -> LRESULT {
        let mut client = RECT::default();
        let theme = self.window.theme();
        let under = self.window.surface_under(self.hwnd);
        // SAFETY: `client` is a valid RECT to write into, and `dc` the area's DC for the
        // erase.
        unsafe {
            GetClientRect(self.hwnd, &raw mut client).expect("a live area has a client area");
            FillRect(dc, &raw const client, theme.surface_brush(under));
        }
        self.surfaces
            .paint(dc, self.offset.get(), &theme.palette, self.window.scale());
        LRESULT(1)
    }

    fn handle(&self, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        match message {
            WM_VSCROLL => {
                self.scroll_bar(SCROLLBAR_COMMAND(i32::from(low_word(wparam.0))));
                Some(LRESULT(0))
            }
            WM_MOUSEWHEEL => {
                self.wheel(high_word(wparam.0).cast_signed());
                Some(LRESULT(0))
            }
            WM_SIZE => {
                self.scroll_to(self.offset.get());
                Some(LRESULT(0))
            }
            WM_ERASEBKGND => Some(self.erase_background(dc_param(wparam))),
            WM_CONTEXTMENU => context_menu::show(
                self.hwnd,
                &self.context_menus,
                self.offset.get(),
                wparam,
                lparam,
            ),
            WM_NCDESTROY => {
                self.destroyed.set(true);
                OPEN_SCROLLS.with_borrow_mut(|open| open.remove(&hwnd_key(self.hwnd)));
                None
            }
            // Its controls' notifications, which their window answers.
            _ => self.window.handle_from_child(message, wparam, lparam),
        }
    }
}

/// The scroll area `hwnd` is, if it is one.
pub(crate) fn scroll_area(hwnd: HWND) -> Option<Rc<ScrollArea>> {
    OPEN_SCROLLS.with_borrow(|open| open.get(&hwnd_key(hwnd)).cloned())
}

/// The window procedure of every scroll area.
pub(crate) unsafe extern "system" fn scroll_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Messages sent while `CreateWindowExW` runs arrive before the area is registered.
    scroll_area(hwnd)
        .and_then(|area| area.handle(message, wparam, lparam))
        // SAFETY: the parameters are the ones Win32 passed to this window procedure.
        .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, message, wparam, lparam) })
}

fn scroll_info_size() -> u32 {
    u32::try_from(mem::size_of::<SCROLLINFO>()).expect("SCROLLINFO is small")
}
