//! `AppKit` backend: every Nifty widget is a real `NSView` subclass from the system.

mod classes;
mod convert;
mod waker;
mod widgets;

use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSMenu, NSMenuItem, NSWindow,
    NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSString};

use nifty_runtime::{
    Backend, ButtonStyle, ClickHandler, Frame, ResizeHandler, Size, TextHandler, TextStyle,
    ToggleHandler, WakeHandler,
};

use classes::{AppDelegate, FlippedView, WindowDelegate};
use convert::{to_ns_rect, to_ns_size, to_size};
pub use waker::MacWaker;
pub use widgets::MacWidget;

#[derive(Debug)]
pub struct MacBackend {
    mtm: MainThreadMarker,
    app: Retained<NSApplication>,
    _delegate: Retained<AppDelegate>,
}

impl MacBackend {
    /// # Panics
    /// When called off the main thread, which `AppKit` forbids.
    #[must_use]
    pub fn new() -> Self {
        let mtm = MainThreadMarker::new().expect("AppKit apps must start on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        install_main_menu(&app, mtm);

        let delegate = AppDelegate::new(mtm);
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Self {
            mtm,
            app,
            _delegate: delegate,
        }
    }
}

impl Default for MacBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// The app menu provides Quit; the Edit menu is what makes copy, paste and undo work in
/// text fields, since `AppKit` routes those shortcuts through menu items.
fn install_main_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let menu_bar = NSMenu::new(mtm);
    add_submenu(&menu_bar, "", &[("Quit", sel!(terminate:), "q")], mtm);
    add_submenu(
        &menu_bar,
        "Edit",
        &[
            ("Undo", sel!(undo:), "z"),
            ("Redo", sel!(redo:), "Z"),
            ("Cut", sel!(cut:), "x"),
            ("Copy", sel!(copy:), "c"),
            ("Paste", sel!(paste:), "v"),
            ("Select All", sel!(selectAll:), "a"),
        ],
        mtm,
    );
    app.setMainMenu(Some(&menu_bar));
}

fn add_submenu(menu_bar: &NSMenu, title: &str, items: &[(&str, Sel, &str)], mtm: MainThreadMarker) {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str(title));
    for (item_title, action, key) in items {
        // SAFETY: each action is a standard responder-chain selector `AppKit` implements.
        unsafe {
            menu.addItemWithTitle_action_keyEquivalent(
                &NSString::from_str(item_title),
                Some(*action),
                &NSString::from_str(key),
            );
        }
    }
    let item = NSMenuItem::new(mtm);
    item.setSubmenu(Some(&menu));
    menu_bar.addItem(&item);
}

#[derive(Debug)]
pub struct MacWindow {
    window: Retained<NSWindow>,
    content: Retained<FlippedView>,
    _delegate: Retained<WindowDelegate>,
}

impl Backend for MacBackend {
    type Window = MacWindow;
    type Widget = MacWidget;
    type Waker = MacWaker;

    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> MacWindow {
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable;
        // SAFETY: `setReleasedWhenClosed(false)` below; `MacWindow` owns the window instead.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(self.mtm),
                NSRect::new(NSPoint::ZERO, to_ns_size(size)),
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: the window is retained by `MacWindow` for as long as it is used.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(title));

        let content = FlippedView::new(self.mtm);
        window.setContentView(Some(&content));
        let delegate = WindowDelegate::new(self.mtm, Retained::clone(&content), on_resize);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        MacWindow {
            window,
            content,
            _delegate: delegate,
        }
    }

    fn content_size(&self, window: &MacWindow) -> Size {
        to_size(window.content.frame().size)
    }

    fn show_window(&self, window: &MacWindow) {
        window.window.center();
        window.window.makeKeyAndOrderFront(None);
    }

    fn set_title(&self, window: &MacWindow, title: &str) {
        window.window.setTitle(&NSString::from_str(title));
    }

    fn set_content_size(&self, window: &MacWindow, size: Size) {
        window.window.setContentSize(to_ns_size(size));
    }

    fn close_window(&self, window: MacWindow) {
        window.window.close();
    }

    fn create_label(&self, window: &MacWindow, text: &str, style: TextStyle) -> MacWidget {
        add_to(window, MacWidget::label(self.mtm, text, style))
    }

    fn create_button(
        &self,
        window: &MacWindow,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> MacWidget {
        add_to(window, MacWidget::button(self.mtm, title, style, on_click))
    }

    fn create_text_field(
        &self,
        window: &MacWindow,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> MacWidget {
        add_to(
            window,
            MacWidget::text_field(self.mtm, text, placeholder, on_change),
        )
    }

    fn create_checkbox(
        &self,
        window: &MacWindow,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> MacWidget {
        add_to(
            window,
            MacWidget::checkbox(self.mtm, title, checked, on_toggle),
        )
    }

    fn create_divider(&self, window: &MacWindow) -> MacWidget {
        add_to(window, MacWidget::divider(self.mtm))
    }

    fn set_text(&self, widget: &MacWidget, text: &str) {
        widget.set_text(text);
    }

    fn set_checked(&self, widget: &MacWidget, checked: bool) {
        widget.set_checked(checked);
    }

    fn measure(&self, widget: &MacWidget) -> Size {
        widget.measure()
    }

    fn set_frame(&self, widget: &MacWidget, frame: Frame) {
        widget.view().setFrame(to_ns_rect(frame));
    }

    fn remove_widget(&self, widget: MacWidget) {
        widget.remove();
    }

    fn waker(&self, on_wake: WakeHandler) -> MacWaker {
        MacWaker::new(self.mtm, on_wake)
    }

    fn run(&self) {
        self.app.run();
    }
}

fn add_to(window: &MacWindow, widget: MacWidget) -> MacWidget {
    window.content.addSubview(widget.view());
    widget
}
