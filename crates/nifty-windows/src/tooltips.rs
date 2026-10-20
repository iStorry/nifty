//! The tooltips of a window's icon-only buttons, which show the title the button leaves out.

use std::cell::OnceCell;
use std::mem;

use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::Controls::{
    TOOLTIPS_CLASSW, TTF_IDISHWND, TTF_SUBCLASS, TTM_ADDTOOLW, TTM_DELTOOLW, TTM_UPDATETIPTEXTW,
    TTS_ALWAYSTIP, TTS_NOPREFIX, TTTOOLINFOW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CW_USEDEFAULT, CreateWindowExW, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WS_POPUP,
};
use windows::core::{PCWSTR, PWSTR};

use crate::convert::wide;

/// One tooltip control serves every button in a window. It is made when the first button
/// needs it, and Windows destroys it with the window that owns it.
#[derive(Debug, Default)]
pub(crate) struct Tooltips {
    control: OnceCell<HWND>,
}

impl Tooltips {
    /// Shows `text` while the pointer rests on `tool`, a child of `window`.
    pub(crate) fn add(&self, window: HWND, tool: HWND, text: &str) {
        self.send(window, tool, TTM_ADDTOOLW, text);
    }

    pub(crate) fn update(&self, window: HWND, tool: HWND, text: &str) {
        self.send(window, tool, TTM_UPDATETIPTEXTW, text);
    }

    pub(crate) fn remove(&self, window: HWND, tool: HWND) {
        self.send(window, tool, TTM_DELTOOLW, "");
    }

    fn send(&self, window: HWND, tool: HWND, message: u32, text: &str) {
        let control = *self.control.get_or_init(|| create(window));
        let mut text = wide(text);
        let info = TTTOOLINFOW {
            cbSize: u32::try_from(mem::size_of::<TTTOOLINFOW>()).expect("TTTOOLINFOW is small"),
            uFlags: TTF_IDISHWND | TTF_SUBCLASS,
            hwnd: window,
            uId: tool.0.addr(),
            lpszText: PWSTR(text.as_mut_ptr()),
            ..Default::default()
        };
        // SAFETY: the tooltip messages take a TTTOOLINFOW, and copy its text before
        // returning, so `text` only needs to live for the call.
        unsafe {
            SendMessageW(
                control,
                message,
                None,
                Some(LPARAM(
                    std::ptr::from_ref(&info).expose_provenance().cast_signed(),
                )),
            );
        }
    }
}

fn create(window: HWND) -> HWND {
    // SAFETY: the tooltip class is registered by Common Controls 6, which Nifty's manifest
    // selects, and `window` is a live window on this thread that will own the tooltip.
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            TOOLTIPS_CLASSW,
            PCWSTR::null(),
            WS_POPUP | WINDOW_STYLE(TTS_ALWAYSTIP | TTS_NOPREFIX),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            Some(window),
            None,
            None,
            None,
        )
    }
    .expect("Windows can create a tooltip")
}
