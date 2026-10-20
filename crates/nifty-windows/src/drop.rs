//! Drop targets: areas of a window's or scroll area's content that take files and links
//! dragged from other apps, through OLE drag and drop. A host registers a drop target with
//! OLE once it has a drop area, and the target finds the innermost area under the drag, in
//! the host under the pointer or the hosts around it.

use std::cell::Cell;
use std::fmt;
use std::ptr;
use std::rc::Rc;

use windows::Win32::Foundation::{HGLOBAL, HWND, S_OK};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, TYMED_HGLOBAL};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, IDropTarget, RegisterDragDrop,
    ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::core::{Result, w};

use nifty_runtime::DropHandler;

use crate::areas::Areas;
use crate::window::WindowState;
use target::DropTarget;

/// The index `DragQueryFileW` takes to answer how many files a drop holds.
const FILE_COUNT: u32 = u32::MAX;
/// The index of an `HGLOBAL`'s data that is no particular item.
const ALL_ITEMS: i32 = -1;
/// Bytes per UTF-16 code unit.
const UNIT_BYTES: usize = 2;

/// A host's drop areas, and whether it is registered with OLE to take drops.
#[derive(Default)]
pub(crate) struct DropAreas {
    areas: Areas<Rc<DropHandler>>,
    registered: Cell<bool>,
}

impl fmt::Debug for DropAreas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropAreas")
            .field("registered", &self.registered)
            .finish_non_exhaustive()
    }
}

impl DropAreas {
    /// Adds an area that gives what is dropped on it to `on_drop`, registering `host`,
    /// a host of `window`, with OLE the first time.
    pub(crate) fn add(
        &self,
        window: &Rc<WindowState>,
        host: HWND,
        on_drop: DropHandler,
    ) -> Result<u32> {
        if !self.registered.get() {
            let target: IDropTarget = DropTarget {
                window: Rc::downgrade(window),
                carries_drops: Cell::new(false),
            }
            .into();
            // SAFETY: `host` is a live window of this thread, which OLE is initialized on
            // by the backend's `ComApartment`; `revoke` unregisters it before it goes.
            unsafe { RegisterDragDrop(host, &target) }?;
            self.registered.set(true);
        }
        Ok(self.areas.add(Rc::new(on_drop)))
    }

    pub(crate) fn areas(&self) -> &Areas<Rc<DropHandler>> {
        &self.areas
    }

    /// Unregisters `host` from OLE, if it was registered; called as `host` is destroyed.
    pub(crate) fn revoke(&self, host: HWND) {
        if self.registered.replace(false) {
            // SAFETY: `host` is the live window `add` registered.
            let revoked = unsafe { RevokeDragDrop(host) };
            debug_assert!(revoked.is_ok(), "a registered window can be revoked");
        }
    }
}

/// The OLE drop target, apart from the rest because the code `implement` generates for it
/// inlines always and casts references to pointers, which the workspace's lints forbid.
#[expect(
    clippy::inline_always,
    clippy::ref_as_ptr,
    reason = "the code the `implement` macro generates"
)]
mod target {
    use std::cell::Cell;
    use std::rc::{Rc, Weak};

    use windows::Win32::Foundation::{POINT, POINTL};
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT, IDropTarget, IDropTarget_Impl};
    use windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS;
    use windows::Win32::UI::WindowsAndMessaging::{GetParent, WindowFromPoint};
    use windows::core::{Ref, Result, implement};

    use nifty_runtime::DropHandler;

    use super::{carries_drops, dropped, set_effect};
    use crate::convert::screen_to_content;
    use crate::scroll::{Host, scroll_area};
    use crate::window::WindowState;

    /// What OLE asks about a drag over one of a window's hosts.
    #[implement(IDropTarget)]
    pub(super) struct DropTarget {
        pub(super) window: Weak<WindowState>,
        /// Whether the drag carries files or links, which only `DragEnter` is told.
        pub(super) carries_drops: Cell<bool>,
    }

    impl IDropTarget_Impl for DropTarget_Impl {
        fn DragEnter(
            &self,
            data: Ref<IDataObject>,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> Result<()> {
            self.carries_drops.set(carries_drops(data.ok()?));
            self.answer(*point, effect);
            Ok(())
        }

        fn DragOver(
            &self,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> Result<()> {
            self.answer(*point, effect);
            Ok(())
        }

        fn DragLeave(&self) -> Result<()> {
            self.carries_drops.set(false);
            Ok(())
        }

        fn Drop(
            &self,
            data: Ref<IDataObject>,
            _keys: MODIFIERKEYS_FLAGS,
            point: &POINTL,
            effect: *mut DROPEFFECT,
        ) -> Result<()> {
            self.carries_drops.set(false);
            let items = dropped(data.ok()?);
            let on_drop = self.on_drop_at(*point).filter(|_| !items.is_empty());
            set_effect(effect, on_drop.is_some());
            if let Some(on_drop) = on_drop {
                on_drop(items);
            }
            Ok(())
        }
    }

    impl DropTarget {
        /// Tells OLE whether a drop at `point` would be taken.
        fn answer(&self, point: POINTL, effect: *mut DROPEFFECT) {
            let takes = self.carries_drops.get() && self.on_drop_at(point).is_some();
            set_effect(effect, takes);
        }

        /// What takes a drop at `point`, on the screen: the innermost drop area there, in the
        /// host under it or the hosts around that, nearest first.
        fn on_drop_at(&self, point: POINTL) -> Option<Rc<DropHandler>> {
            let window = self.window.upgrade()?;
            let screen = POINT {
                x: point.x,
                y: point.y,
            };
            // SAFETY: `WindowFromPoint` accepts any point.
            let mut hwnd = unsafe { WindowFromPoint(screen) };
            loop {
                let host = if hwnd == window.hwnd() {
                    Some(Host::Window)
                } else {
                    scroll_area(hwnd).map(Host::Scroll)
                };
                if let Some(host) = host {
                    let content = screen_to_content(hwnd, host.offset(), screen);
                    if let Some(on_drop) = host.drop_areas(&window).areas().innermost_at(content) {
                        return Some(on_drop);
                    }
                }
                if hwnd == window.hwnd() {
                    return None;
                }
                // SAFETY: `hwnd` is a window under the pointer, inside the Nifty window.
                hwnd = unsafe { GetParent(hwnd) }.ok()?;
            }
        }
    }
}

/// Answers OLE through `effect` with a copy when the drop would be `taken`.
fn set_effect(effect: *mut DROPEFFECT, taken: bool) {
    // SAFETY: OLE passes a pointer to the effect it reads back, or none.
    if let Some(effect) = unsafe { effect.as_mut() } {
        *effect = if taken {
            DROPEFFECT_COPY
        } else {
            DROPEFFECT_NONE
        };
    }
}

/// Whether `data` holds files or a link.
fn carries_drops(data: &IDataObject) -> bool {
    [CF_HDROP.0, link_format()].into_iter().any(|format| {
        let format = in_global_memory(format);
        // SAFETY: `format` describes data in global memory, which `QueryGetData` only asks
        // about.
        let answer = unsafe { data.QueryGetData(&raw const format) };
        answer == S_OK
    })
}

/// The paths of the files `data` holds, or else the address of the link it holds.
fn dropped(data: &IDataObject) -> Vec<String> {
    read(data, CF_HDROP.0, file_paths)
        .or_else(|| read(data, link_format(), link_address))
        .unwrap_or_default()
}

/// What `parse` makes of `data` in `format`, or `None` when `data` has no such format.
fn read(
    data: &IDataObject,
    format: u16,
    parse: impl FnOnce(HGLOBAL) -> Vec<String>,
) -> Option<Vec<String>> {
    // SAFETY: the format asks for data in global memory, which `parse` only reads before
    // the medium is released.
    let format = in_global_memory(format);
    unsafe {
        let mut medium = data.GetData(&raw const format).ok()?;
        let items = parse(medium.u.hGlobal);
        ReleaseStgMedium(&raw mut medium);
        Some(items)
    }
}

/// The paths of the files in a dropped `CF_HDROP`.
fn file_paths(global: HGLOBAL) -> Vec<String> {
    let drop = HDROP(global.0);
    // SAFETY: `global` holds a `DROPFILES`, which `DragQueryFileW` reads.
    let count = unsafe { DragQueryFileW(drop, FILE_COUNT, None) };
    (0..count)
        .map(|index| {
            // SAFETY: as above; the buffer has room for the path and its terminator.
            unsafe {
                let len = DragQueryFileW(drop, index, None);
                let mut path = vec![0; usize::try_from(len).expect("paths fit in memory") + 1];
                let written = DragQueryFileW(drop, index, Some(&mut path));
                String::from_utf16_lossy(&path[..usize::try_from(written).expect("as above")])
            }
        })
        .collect()
}

/// The address of a dropped link, as browsers give it: NUL-terminated UTF-16.
fn link_address(global: HGLOBAL) -> Vec<String> {
    // SAFETY: `global` is live global memory, locked while it is read.
    unsafe {
        let units = GlobalLock(global).cast::<u16>().cast_const();
        if units.is_null() {
            return Vec::new();
        }
        let len = GlobalSize(global) / UNIT_BYTES;
        let units = std::slice::from_raw_parts(units, len);
        let end = units.iter().position(|&unit| unit == 0).unwrap_or(len);
        let address = String::from_utf16_lossy(&units[..end]);
        // It answers an error once the lock count is back at zero, which it is here.
        let _unlocked = GlobalUnlock(global);
        vec![address]
    }
}

/// The clipboard format browsers give a dragged link's address in.
fn link_format() -> u16 {
    // SAFETY: the name is a NUL-terminated constant.
    let format = unsafe { RegisterClipboardFormatW(w!("UniformResourceLocatorW")) };
    u16::try_from(format).expect("registered clipboard formats fit in 16 bits")
}

/// Data in `format` in global memory, for the whole of it.
fn in_global_memory(format: u16) -> FORMATETC {
    FORMATETC {
        cfFormat: format,
        ptd: ptr::null_mut(),
        dwAspect: DVASPECT_CONTENT.0,
        lindex: ALL_ITEMS,
        tymed: TYMED_HGLOBAL.0.cast_unsigned(),
    }
}
