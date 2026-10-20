//! Objective-C classes Nifty registers to receive `AppKit` callbacks.

use std::fmt;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSButton, NSControlStateValueOn,
    NSControlTextEditingDelegate, NSPreferredScrollerStyleDidChangeNotification, NSTextField,
    NSTextFieldDelegate, NSView, NSWindowDelegate,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol};

use nifty_runtime::{ClickHandler, ResizeHandler, TextHandler, ToggleHandler};

use crate::convert::to_size;

define_class!(
    // SAFETY: NSView has no subclassing requirements and `FlippedView` does not implement Drop.
    #[unsafe(super = NSView)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyFlippedView"]
    #[derive(Debug)]
    pub(crate) struct FlippedView;

    impl FlippedView {
        /// Puts the origin at the top-left, matching Nifty's layout coordinates.
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl FlippedView {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: `init` is NSView's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct ClickTargetIvars {
    on_click: ClickHandler,
}

impl fmt::Debug for ClickTargetIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClickTargetIvars").finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `ClickTarget` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyClickTarget"]
    #[ivars = ClickTargetIvars]
    #[derive(Debug)]
    pub(crate) struct ClickTarget;

    impl ClickTarget {
        #[unsafe(method(click:))]
        fn click(&self, _sender: Option<&AnyObject>) {
            (self.ivars().on_click)();
        }
    }
);

impl ClickTarget {
    pub(crate) fn action() -> Sel {
        sel!(click:)
    }

    pub(crate) fn new(mtm: MainThreadMarker, on_click: ClickHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ClickTargetIvars { on_click });
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct ToggleTargetIvars {
    on_toggle: ToggleHandler,
}

impl fmt::Debug for ToggleTargetIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ToggleTargetIvars").finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `ToggleTarget` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyToggleTarget"]
    #[ivars = ToggleTargetIvars]
    #[derive(Debug)]
    pub(crate) struct ToggleTarget;

    impl ToggleTarget {
        #[unsafe(method(toggle:))]
        fn toggle(&self, sender: &NSButton) {
            (self.ivars().on_toggle)(sender.state() == NSControlStateValueOn);
        }
    }
);

impl ToggleTarget {
    pub(crate) fn action() -> Sel {
        sel!(toggle:)
    }

    pub(crate) fn new(mtm: MainThreadMarker, on_toggle: ToggleHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ToggleTargetIvars { on_toggle });
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct TextFieldDelegateIvars {
    field: Weak<NSTextField>,
    on_change: TextHandler,
}

impl fmt::Debug for TextFieldDelegateIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextFieldDelegateIvars")
            .finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TextFieldDelegate` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyTextFieldDelegate"]
    #[ivars = TextFieldDelegateIvars]
    #[derive(Debug)]
    pub(crate) struct TextFieldDelegate;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for TextFieldDelegate {}

    // SAFETY: NSControlTextEditingDelegate has no safety requirements.
    unsafe impl NSControlTextEditingDelegate for TextFieldDelegate {
        #[unsafe(method(controlTextDidChange:))]
        fn control_text_did_change(&self, _notification: &NSNotification) {
            let ivars = self.ivars();
            if let Some(field) = ivars.field.load() {
                (ivars.on_change)(&field.stringValue().to_string());
            }
        }
    }

    // SAFETY: NSTextFieldDelegate has no safety requirements.
    unsafe impl NSTextFieldDelegate for TextFieldDelegate {}
);

impl TextFieldDelegate {
    /// Only edits made by the user notify the delegate; setting the value in code does not.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        field: &Retained<NSTextField>,
        on_change: TextHandler,
    ) -> Retained<Self> {
        let ivars = TextFieldDelegateIvars {
            field: Weak::from_retained(field),
            on_change,
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct WindowDelegateIvars {
    content: Retained<FlippedView>,
    on_resize: ResizeHandler,
}

impl fmt::Debug for WindowDelegateIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowDelegateIvars")
            .field("content", &self.content)
            .finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `WindowDelegate` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyWindowDelegate"]
    #[ivars = WindowDelegateIvars]
    #[derive(Debug)]
    pub(crate) struct WindowDelegate;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for WindowDelegate {}

    // SAFETY: NSWindowDelegate has no safety requirements.
    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowDidResize:))]
        fn window_did_resize(&self, _notification: &NSNotification) {
            self.report_size();
        }
    }

    impl WindowDelegate {
        /// Scroll bars that float over the content became ones that take a strip beside
        /// it, or the other way round, so the content is laid out again.
        #[unsafe(method(preferredScrollerStyleDidChange:))]
        fn preferred_scroller_style_did_change(&self, _notification: &NSNotification) {
            self.report_size();
        }
    }
);

impl WindowDelegate {
    pub(crate) fn new(
        mtm: MainThreadMarker,
        content: Retained<FlippedView>,
        on_resize: ResizeHandler,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(WindowDelegateIvars { content, on_resize });
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        let delegate: Retained<Self> = unsafe { msg_send![super(this), init] };
        // SAFETY: the delegate implements the selector, which takes the notification, and
        // the center stops posting to an observer once it is deallocated.
        unsafe {
            NSNotificationCenter::defaultCenter().addObserver_selector_name_object(
                &delegate,
                sel!(preferredScrollerStyleDidChange:),
                Some(NSPreferredScrollerStyleDidChangeNotification),
                None,
            );
        }
        delegate
    }

    fn report_size(&self) {
        let ivars = self.ivars();
        (ivars.on_resize)(to_size(ivars.content.frame().size));
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `AppDelegate` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyAppDelegate"]
    #[derive(Debug)]
    pub(crate) struct AppDelegate;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for AppDelegate {}

    // SAFETY: NSApplicationDelegate has no safety requirements.
    unsafe impl NSApplicationDelegate for AppDelegate {
        /// Apps launched from a terminal start in the background unless activated.
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            NSApplication::sharedApplication(self.mtm()).activate();
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn should_terminate_after_last_window_closed(&self, _sender: &NSApplication) -> bool {
            true
        }
    }
);

impl AppDelegate {
    pub(crate) fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}
