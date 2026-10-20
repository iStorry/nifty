//! `Win32` backend: every Nifty widget is a real common control from the system.
//!
//! Executables must embed an application manifest that selects Common Controls 6 and
//! per-monitor DPI awareness, as `nifty-cli`'s build script does for `nf` and `nf-runner`.
#![cfg(windows)]

mod convert;
mod fonts;
mod settings;
mod waker;
mod widgets;
mod window;

use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Rc;

use windows::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, HINSTANCE};
use windows::Win32::Graphics::Gdi::{COLOR_WINDOW, HBRUSH};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, DispatchMessageW, GetMessageW, IDC_ARROW, IsDialogMessageW,
    LoadCursorW, MSG, RegisterClassExW, TranslateMessage, WNDCLASSEXW, WNDPROC,
};
use windows::core::{Error, PCWSTR, Result};

use nifty_runtime::{
    Backend, ButtonStyle, ClickHandler, Frame, LabelStyle, ResizeHandler, Size, TextHandler,
    ToggleHandler, WakeHandler,
};

pub use waker::WinWaker;
use waker::{WAKER_CLASS, WakerWindow, waker_proc};
pub use widgets::WinWidget;
pub use window::WinWindow;
use window::{WINDOW_CLASS, WindowState, nifty_root, window_proc};

#[derive(Debug)]
pub struct WinBackend {
    instance: HINSTANCE,
    shown_windows: Rc<Cell<usize>>,
    /// Removed widgets, destroyed once the event that removed them has been handled.
    retired: RefCell<Vec<WinWidget>>,
    waker_window: WakerWindow,
}

impl WinBackend {
    /// # Panics
    /// When Windows can't register Nifty's window classes or create its waker window.
    #[must_use]
    pub fn new() -> Self {
        Self::try_new().expect("Windows can set up Nifty's window classes")
    }

    fn try_new() -> Result<Self> {
        // SAFETY: a null name asks for the module of the running executable.
        let instance = HINSTANCE::from(unsafe { GetModuleHandleW(PCWSTR::null())? });
        register_class(instance, WINDOW_CLASS, Some(window_proc))?;
        register_class(instance, WAKER_CLASS, Some(waker_proc))?;
        Ok(Self {
            instance,
            shown_windows: Rc::new(Cell::new(0)),
            retired: RefCell::new(Vec::new()),
            waker_window: WakerWindow::new(instance)?,
        })
    }

    fn release_retired_widgets(&self) {
        let retired = mem::take(&mut *self.retired.borrow_mut());
        drop(retired);
    }
}

impl Default for WinBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn register_class(instance: HINSTANCE, name: PCWSTR, procedure: WNDPROC) -> Result<()> {
    let class = WNDCLASSEXW {
        cbSize: u32::try_from(mem::size_of::<WNDCLASSEXW>()).expect("WNDCLASSEXW is small"),
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: procedure,
        hInstance: instance,
        // SAFETY: IDC_ARROW is a system cursor, loaded without a module.
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW)? },
        // Win32's convention for a system color brush is the color index plus one.
        hbrBackground: HBRUSH(std::ptr::without_provenance_mut(
            usize::try_from(COLOR_WINDOW.0 + 1).expect("small constant"),
        )),
        lpszClassName: name,
        ..Default::default()
    };
    // SAFETY: `class` is fully initialized and its name is a static string.
    if unsafe { RegisterClassExW(&raw const class) } != 0 {
        return Ok(());
    }
    let error = Error::from_thread();
    // A second backend in the same process finds the classes already registered.
    if error.code() == ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
        Ok(())
    } else {
        Err(error)
    }
}

impl Backend for WinBackend {
    type Window = WinWindow;
    type Widget = WinWidget;
    type Waker = WinWaker;

    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> WinWindow {
        WindowState::open(
            self.instance,
            title,
            size,
            on_resize,
            Rc::clone(&self.shown_windows),
        )
        .expect("Windows can create a window")
    }

    fn content_size(&self, window: &WinWindow) -> Size {
        window.state().content_size()
    }

    fn show_window(&self, window: &WinWindow) {
        window.state().show();
    }

    fn set_title(&self, window: &WinWindow, title: &str) {
        window.state().set_title(title);
    }

    fn set_content_size(&self, window: &WinWindow, size: Size) {
        window.state().set_content_size(size);
    }

    fn close_window(&self, window: WinWindow) {
        drop(window);
    }

    fn create_label(&self, window: &WinWindow, text: &str, style: LabelStyle) -> WinWidget {
        WinWidget::label(window.state(), text, style).expect("Windows can create a label")
    }

    fn create_button(
        &self,
        window: &WinWindow,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> WinWidget {
        WinWidget::button(window.state(), title, style, on_click)
            .expect("Windows can create a button")
    }

    fn create_text_field(
        &self,
        window: &WinWindow,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> WinWidget {
        WinWidget::text_field(window.state(), text, placeholder, on_change)
            .expect("Windows can create a text field")
    }

    fn create_checkbox(
        &self,
        window: &WinWindow,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> WinWidget {
        WinWidget::checkbox(window.state(), title, checked, on_toggle)
            .expect("Windows can create a checkbox")
    }

    fn create_divider(&self, window: &WinWindow) -> WinWidget {
        WinWidget::divider(window.state()).expect("Windows can create a divider")
    }

    fn set_text(&self, widget: &WinWidget, text: &str) {
        widget.set_text(text);
    }

    fn set_checked(&self, widget: &WinWidget, checked: bool) {
        widget.set_checked(checked);
    }

    fn measure(&self, widget: &WinWidget) -> Size {
        widget.measure()
    }

    fn set_frame(&self, widget: &WinWidget, frame: Frame) {
        widget.set_frame(frame);
    }

    /// The control may be the one whose notification is being handled right now, which
    /// Win32 forbids destroying, so it is hidden now and destroyed after the event.
    fn remove_widget(&self, widget: WinWidget) {
        widget.retire();
        self.retired.borrow_mut().push(widget);
    }

    fn waker(&self, on_wake: WakeHandler) -> WinWaker {
        self.waker_window.waker(on_wake)
    }

    /// `IsDialogMessageW` gives Nifty windows the keyboard behaviour of dialogs: Tab moves
    /// between controls and Return presses the primary button.
    fn run(&self) {
        let mut message = MSG::default();
        loop {
            // SAFETY: `message` is a valid MSG to write into.
            match unsafe { GetMessageW(&raw mut message, None, 0, 0) }.0 {
                0 => return,
                -1 => panic!("GetMessageW failed: {}", Error::from_thread()),
                _ => {}
            }
            let handled = nifty_root(message.hwnd)
                // SAFETY: `root` is a live Nifty window and `message` came from its queue.
                .is_some_and(|root| {
                    unsafe { IsDialogMessageW(root, &raw const message) }.as_bool()
                });
            if !handled {
                // SAFETY: `message` came from `GetMessageW`.
                unsafe {
                    let _translated = TranslateMessage(&raw const message);
                    DispatchMessageW(&raw const message);
                }
            }
            self.release_retired_widgets();
        }
    }
}
