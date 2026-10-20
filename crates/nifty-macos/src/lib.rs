//! `AppKit` backend: every Nifty widget is a real `NSView` subclass from the system.
#![cfg(target_os = "macos")]

mod classes;
mod convert;
mod dialog;
mod icons;
mod nav_item;
mod waker;
mod widgets;

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{ProtocolObject, Sel};
use objc2::{MainThreadMarker, MainThreadOnly, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
    NSMenu, NSMenuItem, NSView, NSVisualEffectMaterial, NSVisualEffectView, NSWindow,
    NSWindowOrderingMode, NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::{NSPoint, NSRect, NSString};

use nifty_runtime::{
    Backdrop, Backend, ButtonStyle, ClickHandler, DialogHandler, FileDialog, Frame, Icon,
    LabelStyle, Parent, ResizeHandler, Size, Surface, TextHandler, ToggleHandler, WakeHandler,
};

use classes::{AppDelegate, FlippedView, WindowDelegate};
use convert::{to_ns_size, to_points, to_size};
pub use waker::MacWaker;
pub use widgets::MacWidget;
use widgets::behind_window;

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
    /// Behind all the content of a translucent window; made the first time one asks.
    translucency: RefCell<Option<Retained<NSVisualEffectView>>>,
}

impl Backend for MacBackend {
    type Window = MacWindow;
    type Widget = MacWidget;
    type Waker = MacWaker;

    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> MacWindow {
        // The content runs under a transparent title bar, so surfaces such as a sidebar can
        // reach the top of the window the way Finder's does.
        let style = NSWindowStyleMask::Titled
            | NSWindowStyleMask::Closable
            | NSWindowStyleMask::Miniaturizable
            | NSWindowStyleMask::Resizable
            | NSWindowStyleMask::FullSizeContentView;
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
        window.setTitlebarAppearsTransparent(true);
        window.setToolbarStyle(NSWindowToolbarStyle::Unified);

        let content = FlippedView::new(self.mtm);
        window.setContentView(Some(&content));
        // `size` is the area below the title bar, which the content view also covers. The
        // window grows before it has a delegate, so the resize isn't reported while the
        // caller is still creating it.
        let inset = inset_under_title_bar(&window, &content);
        window.setContentSize(to_ns_size(Size::new(size.width, size.height + inset)));
        let delegate = WindowDelegate::new(self.mtm, Retained::clone(&content), on_resize);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));

        MacWindow {
            window,
            content,
            _delegate: delegate,
            translucency: RefCell::new(None),
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

    /// A Mica-like material doesn't exist on macOS, where a standard window is already the
    /// platform's own look, so only `Translucent` changes anything.
    fn set_backdrop(&self, window: &MacWindow, backdrop: Backdrop) {
        let translucent = backdrop == Backdrop::Translucent;
        let mut translucency = window.translucency.borrow_mut();
        if let Some(view) = &*translucency {
            view.setHidden(!translucent);
        } else if translucent {
            let view = behind_window(self.mtm, NSVisualEffectMaterial::UnderWindowBackground);
            view.setFrame(window.content.bounds());
            view.setAutoresizingMask(
                NSAutoresizingMaskOptions::ViewWidthSizable
                    | NSAutoresizingMaskOptions::ViewHeightSizable,
            );
            window.content.addSubview_positioned_relativeTo(
                &view,
                NSWindowOrderingMode::Below,
                None,
            );
            *translucency = Some(view);
        }
    }

    fn set_content_size(&self, window: &MacWindow, size: Size) {
        window.window.setContentSize(to_ns_size(size));
    }

    fn close_window(&self, window: MacWindow) {
        window.window.close();
    }

    fn title_bar_inset(&self, window: &MacWindow) -> f32 {
        inset_under_title_bar(&window.window, &window.content)
    }

    fn set_min_content_width(&self, window: &MacWindow, width: f32) {
        window
            .window
            .setContentMinSize(to_ns_size(Size::new(width, 0.0)));
    }

    fn create_surface(&self, parent: Parent<'_, Self>, surface: Surface) -> MacWidget {
        add_to(parent, MacWidget::surface(self.mtm, surface))
    }

    fn create_scroll(&self, parent: Parent<'_, Self>) -> MacWidget {
        add_to(parent, MacWidget::scroll(self.mtm))
    }

    fn scroll_bar_width(&self, _window: &MacWindow) -> f32 {
        MacWidget::scroll_bar_width(self.mtm)
    }

    fn set_scroll_content(&self, scroll: &MacWidget, size: Size) {
        scroll.set_scroll_content(size);
    }

    fn create_label(&self, parent: Parent<'_, Self>, text: &str, style: LabelStyle) -> MacWidget {
        add_to(parent, MacWidget::label(self.mtm, text, style))
    }

    fn create_button(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        style: ButtonStyle,
        icon: Icon,
        on_click: ClickHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::button(self.mtm, title, style, icon, on_click),
        )
    }

    fn create_nav_item(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::nav_item(self.mtm, title, icon, badge, selected, on_click),
        )
    }

    fn create_text_field(
        &self,
        parent: Parent<'_, Self>,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::text_field(self.mtm, text, placeholder, on_change),
        )
    }

    fn create_checkbox(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::checkbox(self.mtm, title, checked, on_toggle),
        )
    }

    fn create_divider(&self, parent: Parent<'_, Self>) -> MacWidget {
        add_to(parent, MacWidget::divider(self.mtm))
    }

    fn create_progress(&self, parent: Parent<'_, Self>, percent: Option<i64>) -> MacWidget {
        add_to(parent, MacWidget::progress(self.mtm, percent))
    }

    fn set_text(&self, widget: &MacWidget, text: &str) {
        widget.set_text(text);
    }

    fn set_checked(&self, widget: &MacWidget, checked: bool) {
        widget.set_checked(checked);
    }

    fn set_badge(&self, widget: &MacWidget, badge: &str) {
        widget.set_badge(badge);
    }

    fn set_value(&self, widget: &MacWidget, value: i64) {
        widget.set_value(value);
    }

    fn measure(&self, widget: &MacWidget) -> Size {
        widget.measure()
    }

    fn set_frame(&self, widget: &MacWidget, frame: Frame) {
        widget.set_frame(frame);
    }

    fn remove_widget(&self, widget: MacWidget) {
        widget.remove();
    }

    fn waker(&self, on_wake: WakeHandler) -> MacWaker {
        MacWaker::new(self.mtm, on_wake)
    }

    fn show_file_dialog(&self, dialog: &FileDialog, on_close: DialogHandler) {
        dialog::show(self.mtm, &self.app, dialog, on_close);
    }

    fn run(&self) {
        self.app.run();
    }
}

/// How much of `content` the title bar covers: the part outside the window's layout rect.
fn inset_under_title_bar(window: &NSWindow, content: &NSView) -> f32 {
    to_points(content.frame().size.height - window.contentLayoutRect().size.height)
}

fn add_to(parent: Parent<'_, MacBackend>, widget: MacWidget) -> MacWidget {
    let host: &NSView = match parent {
        Parent::Window(window) => &window.content,
        Parent::Scroll(scroll) => scroll.content_view(),
    };
    host.addSubview(widget.view());
    widget
}
