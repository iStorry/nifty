//! Context menus: a hidden view that marks the area a stack's menu covers, and the lookup
//! the views widgets sit in use to find the menu under a right-click. A right-click that no
//! control takes passes up the responder chain to those views.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSMenu, NSView};
use objc2_foundation::NSRect;

use nifty_runtime::{MenuEntry, MenuHandler};

use crate::classes::MenuTarget;
use crate::menus;

#[derive(Debug)]
pub(crate) struct MenuAreaIvars {
    menu: Retained<NSMenu>,
    /// Its items hold their target weakly, so the area keeps it.
    _target: Retained<MenuTarget>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements and `MenuArea` does not implement Drop.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyMenuArea"]
    #[ivars = MenuAreaIvars]
    #[derive(Debug)]
    pub(crate) struct MenuArea;
);

impl MenuArea {
    /// A hidden area, so it takes no clicks, with a menu of `entries`.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        entries: &[MenuEntry<usize>],
        on_select: MenuHandler,
    ) -> Retained<Self> {
        let target = MenuTarget::new(mtm, on_select);
        let menu = menus::context_menu(mtm, entries, &target);
        let this = Self::alloc(mtm).set_ivars(MenuAreaIvars {
            menu,
            _target: target,
        });
        // SAFETY: `init` is NSView's designated initializer and takes no arguments.
        let area: Retained<Self> = unsafe { msg_send![super(this), init] };
        area.setHidden(true);
        area
    }
}

/// The menu of the smallest area among `host`'s subviews under `event`, which is the
/// innermost stack's; of areas the same size, the one added last.
pub(crate) fn menu_at(host: &NSView, event: &NSEvent) -> Option<Retained<NSMenu>> {
    let point = host.convertPoint_fromView(event.locationInWindow(), None);
    let mut found: Option<(f64, Retained<NSMenu>)> = None;
    for subview in host.subviews() {
        let Some(area) = subview.downcast_ref::<MenuArea>() else {
            continue;
        };
        let frame = area.frame();
        if !contains(frame, point.x, point.y) {
            continue;
        }
        let size = frame.size.width * frame.size.height;
        if found.as_ref().is_none_or(|(smallest, _)| size <= *smallest) {
            found = Some((size, area.ivars().menu.clone()));
        }
    }
    found.map(|(_, menu)| menu)
}

fn contains(frame: NSRect, x: f64, y: f64) -> bool {
    let (left, top) = (frame.origin.x, frame.origin.y);
    (left..left + frame.size.width).contains(&x) && (top..top + frame.size.height).contains(&y)
}
