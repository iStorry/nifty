//! Ownership of GDI objects: ones Nifty creates, and ones it selects into a device context.

use windows::Win32::Graphics::Gdi::{DeleteObject, HDC, HGDIOBJ, SelectObject};
use windows::core::{Error, Result};

/// A font, brush or pen Nifty created, deleted on drop.
#[derive(Debug)]
pub(crate) struct Owned<H: Copy + Into<HGDIOBJ>>(H);

impl<H: Copy + Into<HGDIOBJ>> Owned<H> {
    /// Takes the handle a GDI `Create*` call just returned, which is null when it failed.
    pub(crate) fn new(handle: H) -> Result<Self> {
        if handle.into().is_invalid() {
            Err(Error::from_thread())
        } else {
            Ok(Self(handle))
        }
    }

    pub(crate) fn get(&self) -> H {
        self.0
    }
}

impl<H: Copy + Into<HGDIOBJ>> Drop for Owned<H> {
    fn drop(&mut self) {
        // SAFETY: the object was created for this owner, which outlives every control and
        // device context that uses it.
        let deleted = unsafe { DeleteObject(self.0.into()) };
        debug_assert!(
            deleted.as_bool(),
            "a GDI object Nifty created can be deleted"
        );
    }
}

/// An object selected into a device context, with the previous one restored on drop.
#[derive(Debug)]
pub(crate) struct Selected {
    dc: HDC,
    previous: HGDIOBJ,
}

impl Selected {
    pub(crate) fn new(dc: HDC, object: impl Into<HGDIOBJ>) -> Self {
        // SAFETY: `dc` is a live device context and `object` a live GDI object.
        let previous = unsafe { SelectObject(dc, object.into()) };
        Self { dc, previous }
    }
}

impl Drop for Selected {
    fn drop(&mut self) {
        // SAFETY: `previous` came out of this same device context in `new`.
        unsafe { SelectObject(self.dc, self.previous) };
    }
}
