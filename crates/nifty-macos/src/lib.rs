//! `AppKit` backend: every nifty widget is a real `NSView` subclass from the system.

mod classes;
mod convert;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSButton, NSMenu, NSMenuItem,
    NSTextField, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{NSPoint, NSRect, NSString, ns_string};

use nifty_runtime::{Backend, ClickHandler, Frame, ResizeHandler, Size};

use classes::{AppDelegate, ClickTarget, FlippedView, WindowDelegate};
use convert::{to_ns_rect, to_ns_size, to_size};

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

fn install_main_menu(app: &NSApplication, mtm: MainThreadMarker) {
    let app_menu = NSMenu::new(mtm);
    // SAFETY: `terminate:` is an NSApplication action reached through the responder chain.
    unsafe {
        app_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Quit"),
            Some(sel!(terminate:)),
            ns_string!("q"),
        );
    }
    let app_item = NSMenuItem::new(mtm);
    app_item.setSubmenu(Some(&app_menu));

    let menu_bar = NSMenu::new(mtm);
    menu_bar.addItem(&app_item);
    app.setMainMenu(Some(&menu_bar));
}

#[derive(Debug)]
pub struct MacWindow {
    window: Retained<NSWindow>,
    content: Retained<FlippedView>,
    _delegate: Retained<WindowDelegate>,
}

#[derive(Debug)]
pub struct MacWidget(WidgetKind);

#[derive(Debug)]
enum WidgetKind {
    Label(Retained<NSTextField>),
    Button {
        button: Retained<NSButton>,
        _target: Retained<ClickTarget>,
    },
}

impl MacWidget {
    fn view(&self) -> &NSView {
        match &self.0 {
            WidgetKind::Label(label) => label,
            WidgetKind::Button { button, .. } => button,
        }
    }
}

impl Backend for MacBackend {
    type Window = MacWindow;
    type Widget = MacWidget;

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

    fn create_label(&self, window: &MacWindow, text: &str) -> MacWidget {
        let label = NSTextField::labelWithString(&NSString::from_str(text), self.mtm);
        window.content.addSubview(&label);
        MacWidget(WidgetKind::Label(label))
    }

    fn create_button(&self, window: &MacWindow, title: &str, on_click: ClickHandler) -> MacWidget {
        let target = ClickTarget::new(self.mtm, on_click);
        // SAFETY: `ClickTarget` implements `click:`, and `MacWidget` keeps it alive because
        // AppKit holds button targets weakly.
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &NSString::from_str(title),
                Some(&target),
                Some(ClickTarget::action()),
                self.mtm,
            )
        };
        window.content.addSubview(&button);
        MacWidget(WidgetKind::Button {
            button,
            _target: target,
        })
    }

    fn set_text(&self, widget: &MacWidget, text: &str) {
        let text = NSString::from_str(text);
        match &widget.0 {
            WidgetKind::Label(label) => label.setStringValue(&text),
            WidgetKind::Button { button, .. } => button.setTitle(&text),
        }
    }

    fn measure(&self, widget: &MacWidget) -> Size {
        to_size(widget.view().fittingSize())
    }

    fn set_frame(&self, widget: &MacWidget, frame: Frame) {
        widget.view().setFrame(to_ns_rect(frame));
    }

    fn run(&self) {
        self.app.run();
    }
}
