//! Context menus: a hidden view that marks the area a stack's menu covers, and the lookup
//! the views widgets sit in use to find the menu under a right-click. A right-click that no
//! control takes passes up the responder chain to those views.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSEvent, NSMenu, NSView};

use nifty_runtime::{MenuEntry, MenuHandler};

use crate::areas;
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
    /// An area with a menu of `entries`.
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
        areas::hidden(unsafe { msg_send![super(this), init] })
    }
}

/// The menu of the innermost area under `event` among `host`'s subviews.
pub(crate) fn menu_at(host: &NSView, event: &NSEvent) -> Option<Retained<NSMenu>> {
    let point = host.convertPoint_fromView(event.locationInWindow(), None);
    areas::innermost::<MenuArea>(host, point).map(|area| area.ivars().menu.clone())
}
