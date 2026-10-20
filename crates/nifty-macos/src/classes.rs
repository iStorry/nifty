//! Objective-C classes Nifty registers to receive `AppKit` callbacks.

use std::cell::Cell;
use std::fmt;

use objc2::rc::{Retained, Weak};
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSControl, NSControlStateValue, NSControlStateValueOff,
    NSControlStateValueOn, NSControlTextEditingDelegate, NSEvent, NSMenu, NSMenuItem,
    NSPopUpButton, NSPreferredScrollerStyleDidChangeNotification, NSSlider, NSTextDelegate,
    NSTextField, NSTextFieldDelegate, NSTextView, NSTextViewDelegate, NSView, NSWindow,
    NSWindowDelegate,
};
use objc2_foundation::{NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol};

use nifty_runtime::{
    ClickHandler, CloseHandler, MenuHandler, NumberHandler, ResizeHandler, TextHandler,
    ToggleHandler,
};

use crate::context_menu;
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

        /// Widgets sit straight in this view, so a right-click on one that has no menu
        /// of its own comes here, to the menu of the stack under it.
        #[unsafe(method_id(menuForEvent:))]
        fn menu_for_event(&self, event: &NSEvent) -> Option<Retained<NSMenu>> {
            context_menu::menu_at(self, event)
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
    /// Whether the button it hears from shows as selected, for one that shows that.
    selected: Option<Cell<bool>>,
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
        /// An on-off button flips its own state when clicked, so it is put back to the
        /// selection the click may have changed, through `set_selected`.
        #[unsafe(method(click:))]
        fn click(&self, sender: Option<&AnyObject>) {
            let ivars = self.ivars();
            (ivars.on_click)();
            if let (Some(selected), Some(sender)) = (&ivars.selected, sender) {
                // SAFETY: only buttons send `click:`, and on-off buttons take a state.
                let () = unsafe { msg_send![sender, setState: control_state(selected.get())] };
            }
        }
    }
);

impl ClickTarget {
    pub(crate) fn action() -> Sel {
        sel!(click:)
    }

    pub(crate) fn new(mtm: MainThreadMarker, on_click: ClickHandler) -> Retained<Self> {
        Self::with_ivars(
            mtm,
            ClickTargetIvars {
                on_click,
                selected: None,
            },
        )
    }

    /// The target of a button that shows whether it is `selected`.
    pub(crate) fn selecting(
        mtm: MainThreadMarker,
        on_click: ClickHandler,
        selected: bool,
    ) -> Retained<Self> {
        Self::with_ivars(
            mtm,
            ClickTargetIvars {
                on_click,
                selected: Some(Cell::new(selected)),
            },
        )
    }

    pub(crate) fn set_selected(&self, selected: bool) {
        if let Some(shown) = &self.ivars().selected {
            shown.set(selected);
        }
    }

    fn with_ivars(mtm: MainThreadMarker, ivars: ClickTargetIvars) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

/// The state of an on-off control, ticked or selected when `on`.
pub(crate) fn control_state(on: bool) -> NSControlStateValue {
    if on {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
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
        fn toggle(&self, sender: &AnyObject) {
            // SAFETY: only checkboxes and switches send `toggle:`, and both have a state.
            let state: NSControlStateValue = unsafe { msg_send![sender, state] };
            (self.ivars().on_toggle)(state == NSControlStateValueOn);
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

pub(crate) struct MenuTargetIvars {
    on_select: MenuHandler,
}

impl fmt::Debug for MenuTargetIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MenuTargetIvars").finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `MenuTarget` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyMenuTarget"]
    #[ivars = MenuTargetIvars]
    #[derive(Debug)]
    pub(crate) struct MenuTarget;

    impl MenuTarget {
        /// Each item of the app's menus is numbered by its tag.
        #[unsafe(method(choose:))]
        fn choose(&self, sender: &NSMenuItem) {
            let item = usize::try_from(sender.tag()).expect("menu items are numbered from 0");
            (self.ivars().on_select)(item);
        }
    }
);

impl MenuTarget {
    pub(crate) fn action() -> Sel {
        sel!(choose:)
    }

    pub(crate) fn new(mtm: MainThreadMarker, on_select: MenuHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MenuTargetIvars { on_select });
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct NumberTargetIvars {
    on_change: NumberHandler,
    /// The whole number last shown, so dragging within one reports nothing new.
    last: Cell<i64>,
}

impl fmt::Debug for NumberTargetIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NumberTargetIvars")
            .field("last", &self.last)
            .finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `NumberTarget` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyNumberTarget"]
    #[ivars = NumberTargetIvars]
    #[derive(Debug)]
    pub(crate) struct NumberTarget;

    impl NumberTarget {
        /// Sliders slide smoothly, and report the whole number nearest the knob.
        #[unsafe(method(change:))]
        fn change(&self, sender: &NSSlider) {
            let value = whole(sender.doubleValue());
            let ivars = self.ivars();
            if ivars.last.replace(value) != value {
                (ivars.on_change)(value);
            }
        }
    }
);

impl NumberTarget {
    pub(crate) fn action() -> Sel {
        sel!(change:)
    }

    pub(crate) fn new(
        mtm: MainThreadMarker,
        value: i64,
        on_change: NumberHandler,
    ) -> Retained<Self> {
        let ivars = NumberTargetIvars {
            on_change,
            last: Cell::new(value),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }

    /// Records a value shown from code, which the user didn't pick.
    pub(crate) fn shown(&self, value: i64) {
        self.ivars().last.set(value);
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "a slider's value lies between bounds the checker keeps far inside i64"
)]
fn whole(value: f64) -> i64 {
    value.round() as i64
}

pub(crate) struct PickTargetIvars {
    on_pick: TextHandler,
}

impl fmt::Debug for PickTargetIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PickTargetIvars").finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `PickTarget` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyPickTarget"]
    #[ivars = PickTargetIvars]
    #[derive(Debug)]
    pub(crate) struct PickTarget;

    impl PickTarget {
        #[unsafe(method(pick:))]
        fn pick(&self, sender: &NSPopUpButton) {
            if let Some(title) = sender.titleOfSelectedItem() {
                (self.ivars().on_pick)(&title.to_string());
            }
        }
    }
);

impl PickTarget {
    pub(crate) fn action() -> Sel {
        sel!(pick:)
    }

    pub(crate) fn new(mtm: MainThreadMarker, on_pick: TextHandler) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PickTargetIvars { on_pick });
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct TextFieldDelegateIvars {
    field: Weak<NSTextField>,
    on_change: TextHandler,
    /// What Return runs in the field, in place of the window's default button.
    on_submit: Option<ClickHandler>,
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

        /// Return in a field with an action runs it, and answers that Return is handled,
        /// so the window's default button isn't pressed too.
        #[unsafe(method(control:textView:doCommandBySelector:))]
        fn do_command(&self, _control: &NSControl, _editor: &NSTextView, command: Sel) -> bool {
            match &self.ivars().on_submit {
                Some(on_submit) if command == sel!(insertNewline:) => {
                    on_submit();
                    true
                }
                _ => false,
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
        on_submit: Option<ClickHandler>,
    ) -> Retained<Self> {
        let ivars = TextFieldDelegateIvars {
            field: Weak::from_retained(field),
            on_change,
            on_submit,
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        // SAFETY: `init` is NSObject's designated initializer and takes no arguments.
        unsafe { msg_send![super(this), init] }
    }
}

pub(crate) struct TextViewDelegateIvars {
    text_view: Weak<NSTextView>,
    on_change: TextHandler,
}

impl fmt::Debug for TextViewDelegateIvars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextViewDelegateIvars")
            .finish_non_exhaustive()
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and `TextViewDelegate` does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "NiftyTextViewDelegate"]
    #[ivars = TextViewDelegateIvars]
    #[derive(Debug)]
    pub(crate) struct TextViewDelegate;

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for TextViewDelegate {}

    // SAFETY: NSTextDelegate has no safety requirements.
    unsafe impl NSTextDelegate for TextViewDelegate {
        #[unsafe(method(textDidChange:))]
        fn text_did_change(&self, _notification: &NSNotification) {
            let ivars = self.ivars();
            if let Some(text_view) = ivars.text_view.load() {
                (ivars.on_change)(&text_view.string().to_string());
            }
        }
    }

    // SAFETY: NSTextViewDelegate has no safety requirements.
    unsafe impl NSTextViewDelegate for TextViewDelegate {}
);

impl TextViewDelegate {
    /// Only edits made by the user notify the delegate; setting the text in code does not.
    pub(crate) fn new(
        mtm: MainThreadMarker,
        text_view: &Retained<NSTextView>,
        on_change: TextHandler,
    ) -> Retained<Self> {
        let ivars = TextViewDelegateIvars {
            text_view: Weak::from_retained(text_view),
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
    on_close: CloseHandler,
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

        /// The user asked to close the window; the app closes it, once it has taken its
        /// content down.
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            (self.ivars().on_close)();
            false
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
        on_close: CloseHandler,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(WindowDelegateIvars {
            content,
            on_resize,
            on_close,
        });
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
