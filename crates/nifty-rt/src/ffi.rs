//! Plain-data building blocks of the descriptors compiled programs carry: a slice and a
//! string, laid out as a pointer and a length.

use std::{fmt, slice, str};

/// `len` items starting at `ptr`, which live for the rest of the program.
#[repr(C)]
pub struct Slice<T: 'static> {
    ptr: *const T,
    len: usize,
}

// SAFETY: a slice only reads static, immutable items, so sharing it shares `&T`s.
unsafe impl<T: Sync> Sync for Slice<T> {}

impl<T> Slice<T> {
    /// Where the pointer and the length sit, for code that emits slices as data.
    pub const PTR_OFFSET: usize = std::mem::offset_of!(Self, ptr);
    pub const LEN_OFFSET: usize = std::mem::offset_of!(Self, len);

    pub const EMPTY: Self = Self {
        ptr: std::ptr::null(),
        len: 0,
    };

    #[must_use]
    pub const fn new(items: &'static [T]) -> Self {
        Self {
            ptr: items.as_ptr(),
            len: items.len(),
        }
    }

    #[must_use]
    pub fn items(&self) -> &'static [T] {
        if self.len == 0 {
            return &[];
        }
        // SAFETY: a `Slice` is made from a static slice, or emitted by `nf build` with the
        // same layout, which `nifty_rt_main`'s caller vouches for.
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl<T: fmt::Debug> fmt::Debug for Slice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.items()).finish()
    }
}

/// UTF-8 text that lives for the rest of the program, laid out as a `Slice<u8>`.
#[repr(C)]
pub struct Str(Slice<u8>);

impl Str {
    pub const EMPTY: Self = Self(Slice::EMPTY);

    #[must_use]
    pub const fn new(text: &'static str) -> Self {
        Self(Slice::new(text.as_bytes()))
    }

    /// The text, with anything that isn't UTF-8 replaced.
    #[must_use]
    pub fn lossy(&self) -> String {
        String::from_utf8_lossy(self.0.items()).into_owned()
    }

    /// # Errors
    /// When the bytes aren't UTF-8.
    pub fn text(&self) -> Result<&'static str, str::Utf8Error> {
        str::from_utf8(self.0.items())
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.lossy(), f)
    }
}
