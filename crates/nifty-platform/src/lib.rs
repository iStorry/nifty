//! Picks the native backend for the operating system Nifty is built for.

#[cfg(not(any(target_os = "macos", windows)))]
compile_error!("Nifty has macOS and Windows backends so far; the Linux backend is next");

#[cfg(target_os = "macos")]
pub type PlatformBackend = nifty_macos::MacBackend;

#[cfg(windows)]
pub type PlatformBackend = nifty_windows::WinBackend;

/// # Panics
/// On macOS, when called off the main thread, which `AppKit` forbids; on Windows, when the
/// system can't register Nifty's window classes.
#[must_use]
pub fn platform_backend() -> PlatformBackend {
    PlatformBackend::new()
}
