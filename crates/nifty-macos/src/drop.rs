//! Drop targets: a hidden view that marks the area a stack's drop target covers, and the
//! dragging-destination work of the views widgets sit in, which take files and links once
//! one of their stacks has a drop target.

use std::fmt;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send};
use objc2_app_kit::{
    NSDragOperation, NSDraggingInfo, NSPasteboardItem, NSPasteboardTypeFileURL,
    NSPasteboardTypeURL, NSView,
};
use objc2_foundation::{NSArray, NSString, NSURL};

use nifty_runtime::DropHandler;

use crate::areas;

pub(crate) struct DropAreaIvars {
    on_drop: DropHandler,
}

impl fmt::Debug for DropAreaIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DropAreaIvars").finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `DropArea` does not implement Drop.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyDropArea"]
    #[ivars = DropAreaIvars]
    #[derive(Debug)]
    pub(crate) struct DropArea;
);

impl DropArea {
    /// An area that gives what is dropped on it to `on_drop`.
    pub(crate) fn new(mtm: MainThreadMarker, on_drop: DropHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DropAreaIvars { on_drop });
        // SAFETY: `init` is NSView's designated initializer and takes no arguments.
        areas::hidden(unsafe { msg_send![super(this), init] })
    }
}

/// Lets `host`, a view widgets sit in, take drags of files and links.
pub(crate) fn accept_drops(host: &NSView) {
    host.registerForDraggedTypes(&dropped_types());
}

/// What a drag over `view` would do: copy, when a drop area is under it and it carries
/// files or links.
pub(crate) fn drag_operation(
    view: &NSView,
    drag: &ProtocolObject<dyn NSDraggingInfo>,
) -> NSDragOperation {
    let carries_drops = drag
        .draggingPasteboard()
        .availableTypeFromArray(&dropped_types())
        .is_some();
    if carries_drops && area_under(view, drag).is_some() {
        NSDragOperation::Copy
    } else {
        NSDragOperation::None
    }
}

/// Gives the files and links of a drag dropped on `view` to the drop area under it,
/// answering whether one took them.
pub(crate) fn perform_drop(view: &NSView, drag: &ProtocolObject<dyn NSDraggingInfo>) -> bool {
    let Some(area) = area_under(view, drag) else {
        return false;
    };
    let items: Vec<String> = drag
        .draggingPasteboard()
        .pasteboardItems()
        .map(|items| items.iter().filter_map(|item| dropped(&item)).collect())
        .unwrap_or_default();
    if items.is_empty() {
        return false;
    }
    (area.ivars().on_drop)(items);
    true
}

/// The drop area under the drag in `view` or the views around it, nearest first, since a
/// scroll area's content sits in a view of its own inside the stack around it.
fn area_under(
    view: &NSView,
    drag: &ProtocolObject<dyn NSDraggingInfo>,
) -> Option<Retained<DropArea>> {
    let location = drag.draggingLocation();
    let mut host = Some(view.retain());
    while let Some(current) = host {
        let point = current.convertPoint_fromView(location, None);
        if let Some(area) = areas::innermost::<DropArea>(&current, point) {
            return Some(area);
        }
        // SAFETY: views are only moved between superviews on the main thread, which this is.
        host = unsafe { current.superview() };
    }
    None
}

/// A file's path, or a link's address.
fn dropped(item: &NSPasteboardItem) -> Option<String> {
    // SAFETY: AppKit defines the pasteboard types as immutable constants.
    let (file_url, url) = unsafe { (NSPasteboardTypeFileURL, NSPasteboardTypeURL) };
    if let Some(file) = item.stringForType(file_url) {
        return NSURL::URLWithString(&file)
            .and_then(|file| file.path())
            .map(|path| path.to_string());
    }
    item.stringForType(url).map(|link| link.to_string())
}

/// The pasteboard types a drop target takes: files, and links.
fn dropped_types() -> Retained<NSArray<NSString>> {
    // SAFETY: AppKit defines the pasteboard types as immutable constants.
    unsafe { NSArray::from_slice(&[NSPasteboardTypeFileURL, NSPasteboardTypeURL]) }
}
