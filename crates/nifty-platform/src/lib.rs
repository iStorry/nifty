//! Picks the native backend for the operating system Nifty is built for.

#[cfg(not(target_os = "macos"))]
compile_error!("Nifty has a macOS backend only so far; Windows and Linux backends are next");

#[cfg(target_os = "macos")]
pub type PlatformBackend = nifty_macos::MacBackend;

/// # Panics
/// When called off the main thread, which native UI toolkits require.
#[cfg(target_os = "macos")]
#[must_use]
pub fn platform_backend() -> PlatformBackend {
    nifty_macos::MacBackend::new()
}
