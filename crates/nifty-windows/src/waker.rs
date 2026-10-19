//! Waking the UI thread from others, through a message-only window that receives posted
//! messages even while Win32 runs a modal loop, such as when the user drags a window edge.

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, HWND_MESSAGE, PostMessageW, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP,
};
use windows::core::{PCWSTR, Result, w};

use nifty_runtime::{WakeHandler, Waker};

pub(crate) const WAKER_CLASS: PCWSTR = w!("NiftyWaker");
/// Its `WPARAM` is the index of the waker to run.
const WM_WAKE: u32 = WM_APP;

struct Wake {
    on_wake: Rc<WakeHandler>,
    pending: Arc<AtomicBool>,
}

thread_local! {
    static WAKE_HANDLERS: RefCell<Vec<Wake>> = const { RefCell::new(Vec::new()) };
}

/// The message-only window every waker posts to, destroyed with the backend.
#[derive(Debug)]
pub(crate) struct WakerWindow(HWND);

impl WakerWindow {
    pub(crate) fn new(instance: HINSTANCE) -> Result<Self> {
        // SAFETY: the class is registered by `WinBackend::new`; message-only windows have no
        // title, style or size.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                WAKER_CLASS,
                PCWSTR::null(),
                WINDOW_STYLE(0),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                Some(instance),
                None,
            )?
        };
        Ok(Self(hwnd))
    }

    pub(crate) fn waker(&self, on_wake: WakeHandler) -> WinWaker {
        let pending = Arc::new(AtomicBool::new(false));
        let index = WAKE_HANDLERS.with_borrow_mut(|handlers| {
            handlers.push(Wake {
                on_wake: Rc::new(on_wake),
                pending: Arc::clone(&pending),
            });
            handlers.len() - 1
        });
        WinWaker {
            window: self.0.0.expose_provenance(),
            index,
            pending,
        }
    }
}

impl Drop for WakerWindow {
    fn drop(&mut self) {
        // SAFETY: the window belongs to this thread and has not been destroyed.
        unsafe { DestroyWindow(self.0) }.expect("the waker window can be destroyed");
        WAKE_HANDLERS.with_borrow_mut(Vec::clear);
    }
}

/// Wakes that arrive before the handler runs are coalesced into one run.
pub struct WinWaker {
    /// The waker window's handle, as an address so the waker can cross threads.
    window: usize,
    index: usize,
    pending: Arc<AtomicBool>,
}

impl Waker for WinWaker {
    fn wake(&self) {
        if self.pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let window = HWND(std::ptr::with_exposed_provenance_mut(self.window));
        // SAFETY: `PostMessageW` may be called from any thread with any window handle.
        let posted = unsafe { PostMessageW(Some(window), WM_WAKE, WPARAM(self.index), LPARAM(0)) };
        // Posting fails only once the UI thread has quit; a later wake may try again.
        if posted.is_err() {
            self.pending.store(false, Ordering::Release);
        }
    }
}

impl fmt::Debug for WinWaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WinWaker")
            .field("index", &self.index)
            .finish_non_exhaustive()
    }
}

/// The window procedure of the waker window.
pub(crate) unsafe extern "system" fn waker_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message != WM_WAKE {
        // SAFETY: the parameters are the ones Win32 passed to this window procedure.
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    // Clone the handler out first: it may create another waker.
    let handler = WAKE_HANDLERS.with_borrow(|handlers| {
        handlers.get(wparam.0).map(|wake| {
            wake.pending.store(false, Ordering::Release);
            Rc::clone(&wake.on_wake)
        })
    });
    if let Some(on_wake) = handler {
        on_wake();
    }
    LRESULT(0)
}
