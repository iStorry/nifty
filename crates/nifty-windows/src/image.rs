//! Images read with the Windows Imaging Component, which ships with Windows and reads PNG,
//! JPEG, GIF, BMP, TIFF and ICO files, and drawn with GDI's alpha blending.

use std::cell::RefCell;
use std::mem;
use std::path::Path;
use std::ptr;

use windows::Win32::Foundation::{GENERIC_READ, RECT};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, AC_SRC_OVER, AlphaBlend, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
    CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, HBITMAP, HDC,
};
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppPBGRA, IWICBitmapSource, IWICImagingFactory,
    WICBitmapCacheOnLoad, WICBitmapDitherTypeNone, WICBitmapInterpolationModeHighQualityCubic,
    WICBitmapPaletteTypeCustom, WICDecodeMetadataCacheOnDemand,
};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::core::{HSTRING, Interface, Result};

use nifty_runtime::Size;

use crate::convert::rect_size;
use crate::gdi::{Owned, Selected};

/// Bytes per pixel of the 32-bit premultiplied BGRA the image is converted to.
const BYTES_PER_PIXEL: u32 = 4;
/// Draws every pixel of the image as it is, blending by its own alpha.
const BLEND: BLENDFUNCTION = BLENDFUNCTION {
    BlendOp: u8_flag(AC_SRC_OVER),
    BlendFlags: 0,
    SourceConstantAlpha: u8::MAX,
    AlphaFormat: u8_flag(AC_SRC_ALPHA),
};

/// An image read from a file, in premultiplied BGRA, scaled to the size it was last drawn
/// at, which is kept until it is drawn at another.
#[derive(Debug)]
pub(crate) struct Picture {
    factory: IWICImagingFactory,
    source: IWICBitmapSource,
    width: u32,
    height: u32,
    scaled: RefCell<Option<Scaled>>,
}

#[derive(Debug)]
struct Scaled {
    width: i32,
    height: i32,
    bitmap: Owned<HBITMAP>,
}

impl Picture {
    /// Reads the first frame of the image in the file at `path` into memory, so the file
    /// isn't kept open while the image shows.
    pub(crate) fn read(path: &Path) -> Result<Self> {
        // SAFETY: COM is initialized on this thread by the backend's `ComApartment`.
        let factory: IWICImagingFactory =
            unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)? };
        let path = HSTRING::from(path);
        // SAFETY: the factory and the strings outlive these calls, and the in-memory bitmap
        // copies the converted pixels before the decoder goes.
        unsafe {
            let decoder = factory.CreateDecoderFromFilename(
                &path,
                None,
                GENERIC_READ,
                WICDecodeMetadataCacheOnDemand,
            )?;
            let frame = decoder.GetFrame(0)?;
            let converter = factory.CreateFormatConverter()?;
            converter.Initialize(
                &frame,
                &GUID_WICPixelFormat32bppPBGRA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )?;
            let bitmap = factory.CreateBitmapFromSource(&converter, WICBitmapCacheOnLoad)?;
            let (mut width, mut height) = (0, 0);
            bitmap.GetSize(&raw mut width, &raw mut height)?;
            Ok(Self {
                factory,
                source: bitmap.cast()?,
                width,
                height,
                scaled: RefCell::new(None),
            })
        }
    }

    /// The image's own size in DIPs, a pixel to a DIP, as images are drawn at 100%.
    pub(crate) fn size(&self) -> Size {
        Size::new(as_dips(self.width), as_dips(self.height))
    }

    /// Draws the image into `dc` filling `rect`, scaled smoothly to its size.
    pub(crate) fn draw(&self, dc: HDC, rect: RECT) -> Result<()> {
        let (width, height) = rect_size(&rect);
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let mut scaled = self.scaled.borrow_mut();
        if !scaled
            .as_ref()
            .is_some_and(|scaled| scaled.width == width && scaled.height == height)
        {
            *scaled = Some(self.scale(width, height)?);
        }
        let Some(scaled) = scaled.as_ref() else {
            return Ok(());
        };
        let memory = MemoryDc::new(dc)?;
        let _bitmap = Selected::new(memory.0, scaled.bitmap.get());
        // SAFETY: both DCs are live, and the bitmap selected into `memory` is the whole
        // source area.
        unsafe {
            AlphaBlend(
                dc, rect.left, rect.top, width, height, memory.0, 0, 0, width, height, BLEND,
            )
        }
        .ok()
    }

    /// The image scaled to `width` by `height` pixels, as a top-down 32-bit bitmap.
    fn scale(&self, width: i32, height: i32) -> Result<Scaled> {
        let (pixel_width, pixel_height) = (width.unsigned_abs(), height.unsigned_abs());
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: u32::try_from(mem::size_of::<BITMAPINFOHEADER>())
                    .expect("BITMAPINFOHEADER is small"),
                biWidth: width,
                // A negative height makes the bitmap top-down, as WIC copies pixels.
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..BITMAPINFOHEADER::default()
            },
            ..BITMAPINFO::default()
        };
        let mut bits = ptr::null_mut();
        // SAFETY: `info` describes a 32-bit bitmap, and `bits` receives its pixels.
        let bitmap = Owned::new(unsafe {
            CreateDIBSection(
                None,
                &raw const info,
                DIB_RGB_COLORS,
                &raw mut bits,
                None,
                0,
            )?
        })?;
        let stride = pixel_width * BYTES_PER_PIXEL;
        let len = usize::try_from(stride * pixel_height).expect("bitmaps fit in memory");
        // SAFETY: the section holds `stride` bytes for each of its rows, and lives as long
        // as `bitmap`, which outlives this slice.
        let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u8>(), len) };
        // SAFETY: the scaler reads the source this picture owns, and writes `stride` bytes
        // a row into `pixels`, which has room for every row.
        unsafe {
            let scaler = self.factory.CreateBitmapScaler()?;
            scaler.Initialize(
                &self.source,
                pixel_width,
                pixel_height,
                WICBitmapInterpolationModeHighQualityCubic,
            )?;
            scaler.CopyPixels(ptr::null(), stride, pixels)?;
        }
        Ok(Scaled {
            width,
            height,
            bitmap,
        })
    }
}

/// A memory device context to blend from, deleted on drop.
struct MemoryDc(HDC);

impl MemoryDc {
    fn new(compatible_with: HDC) -> Result<Self> {
        // SAFETY: `compatible_with` is a live device context.
        let dc = unsafe { CreateCompatibleDC(Some(compatible_with)) };
        if dc.is_invalid() {
            Err(windows::core::Error::from_thread())
        } else {
            Ok(Self(dc))
        }
    }
}

impl Drop for MemoryDc {
    fn drop(&mut self) {
        // SAFETY: the DC was created by `new`, and its bitmap is put back by then.
        let deleted = unsafe { DeleteDC(self.0) };
        debug_assert!(deleted.as_bool(), "a memory DC Nifty made can be deleted");
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "image sides are far inside f32's exact integers"
)]
fn as_dips(pixels: u32) -> f32 {
    pixels as f32
}

/// A blend flag, which Win32 declares as a `u32` though `BLENDFUNCTION` takes a byte; the
/// flags are all below 256, so the low byte is the whole flag.
const fn u8_flag(flag: u32) -> u8 {
    flag.to_le_bytes()[0]
}
