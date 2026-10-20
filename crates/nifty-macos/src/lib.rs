//! `AppKit` backend: every Nifty widget is a real `NSView` subclass from the system.
#![cfg(target_os = "macos")]

mod areas;
mod classes;
mod context_menu;
mod convert;
mod dialog;
mod drop;
mod icons;
mod menus;
mod nav_item;
mod switch;
mod waker;
mod widgets;

use std::cell::RefCell;
use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSAutoresizingMaskOptions, NSBackingStoreType,
    NSView, NSVisualEffectMaterial, NSVisualEffectView, NSWindow, NSWindowOrderingMode,
    NSWindowStyleMask, NSWindowToolbarStyle,
};
use objc2_foundation::{NSPoint, NSRect, NSString};

use nifty_runtime::{
    Backdrop, Backend, ButtonStyle, ClickHandler, CloseHandler, DialogHandler, DropHandler,
    FileDialog, Frame, Icon, ImageSize, LabelStyle, Menu, MenuEntry, MenuHandler, NumberHandler,
    Parent, ResizeHandler, Size, Surface, TextHandler, ToggleHandler, ToggleStyle, WakeHandler,
};

use classes::{AppDelegate, FlippedView, MenuTarget, WindowDelegate};
use convert::{to_ns_size, to_points, to_size};
pub use waker::MacWaker;
pub use widgets::MacWidget;
use widgets::behind_window;

#[derive(Debug)]
pub struct MacBackend {
    mtm: MainThreadMarker,
    app: Retained<NSApplication>,
    _delegate: Retained<AppDelegate>,
    /// What the app's own menu items tell when chosen; menu items hold it weakly.
    menu_target: RefCell<Option<Retained<MenuTarget>>>,
}

impl MacBackend {
    /// # Panics
    /// When called off the main thread, which `AppKit` forbids.
    #[must_use]
    pub fn new() -> Self {
        let mtm = MainThreadMarker::new().expect("AppKit apps must start on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        menus::install(&app, mtm, &[], None);

        let delegate = AppDelegate::new(mtm);
        app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Self {
            mtm,
            app,
            _delegate: delegate,
            menu_target: RefCell::new(None),
        }
    }
}

impl Default for MacBackend {
    fn default() -> Self {
        Self::new()
    }
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

    fn open_window(
        &self,
        title: &str,
        size: Size,
        on_resize: ResizeHandler,
        on_close: CloseHandler,
    ) -> MacWindow {
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
        let delegate =
            WindowDelegate::new(self.mtm, Retained::clone(&content), on_resize, on_close);
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

    fn focus_window(&self, window: &MacWindow) {
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
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::button(self.mtm, title, (style, icon), selected, on_click),
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
        lines: u32,
        on_change: TextHandler,
        on_submit: Option<ClickHandler>,
    ) -> MacWidget {
        let field = if lines > 1 {
            MacWidget::text_area(self.mtm, text, lines, on_change)
        } else {
            MacWidget::text_field(self.mtm, text, placeholder, on_change, on_submit)
        };
        add_to(parent, field)
    }

    fn create_checkbox(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        checked: bool,
        style: ToggleStyle,
        on_toggle: ToggleHandler,
    ) -> MacWidget {
        let widget = match style {
            ToggleStyle::Checkbox => MacWidget::checkbox(self.mtm, title, checked, on_toggle),
            ToggleStyle::Switch => MacWidget::switch(self.mtm, title, checked, on_toggle),
        };
        add_to(parent, widget)
    }

    fn create_slider(
        &self,
        parent: Parent<'_, Self>,
        value: i64,
        min: i64,
        max: i64,
        on_change: NumberHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::slider(self.mtm, value, min, max, on_change),
        )
    }

    fn create_picker(
        &self,
        parent: Parent<'_, Self>,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::picker(self.mtm, options, selected, on_pick),
        )
    }

    fn create_divider(&self, parent: Parent<'_, Self>) -> MacWidget {
        add_to(parent, MacWidget::divider(self.mtm))
    }

    fn create_drop_target(&self, parent: Parent<'_, Self>, on_drop: DropHandler) -> MacWidget {
        drop::accept_drops(host_view(parent));
        add_to(parent, MacWidget::drop_target(self.mtm, on_drop))
    }

    fn create_context_menu(
        &self,
        parent: Parent<'_, Self>,
        entries: &[MenuEntry<usize>],
        on_select: MenuHandler,
    ) -> MacWidget {
        add_to(
            parent,
            MacWidget::context_menu(self.mtm, entries, on_select),
        )
    }

    fn create_image(
        &self,
        parent: Parent<'_, Self>,
        path: Option<&Path>,
        size: ImageSize,
    ) -> MacWidget {
        add_to(parent, MacWidget::image(self.mtm, path, size))
    }

    fn set_image(&self, widget: &MacWidget, path: Option<&Path>) {
        widget.set_image(path);
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

    fn set_options(&self, widget: &MacWidget, options: &[String]) {
        widget.set_options(options);
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

    fn set_menu_bar(&self, menus: &[Menu<usize>], on_select: MenuHandler) {
        let target = MenuTarget::new(self.mtm, on_select);
        menus::install(&self.app, self.mtm, menus, Some(&target));
        *self.menu_target.borrow_mut() = Some(target);
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
    host_view(parent).addSubview(widget.view());
    widget
}

/// The view widgets created in `parent` sit in.
fn host_view(parent: Parent<'_, MacBackend>) -> &NSView {
    match parent {
        Parent::Window(window) => &window.content,
        Parent::Scroll(scroll) => scroll.content_view(),
    }
}
