//! The contract each platform (`AppKit`, `Win32`, `GTK`) implements with its own native widgets.

use crate::geometry::{Frame, Size};

pub type ClickHandler = Box<dyn Fn()>;
pub type ResizeHandler = Box<dyn Fn(Size)>;

pub trait Backend: 'static {
    type Window;
    type Widget;

    /// The window starts hidden; `on_resize` receives the new content size after every resize.
    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> Self::Window;
    fn content_size(&self, window: &Self::Window) -> Size;
    fn show_window(&self, window: &Self::Window);

    fn create_label(&self, window: &Self::Window, text: &str) -> Self::Widget;
    fn create_button(
        &self,
        window: &Self::Window,
        title: &str,
        on_click: ClickHandler,
    ) -> Self::Widget;

    fn set_text(&self, widget: &Self::Widget, text: &str);
    /// The widget's natural size for its current content.
    fn measure(&self, widget: &Self::Widget) -> Size;
    /// Positions the widget relative to the window's content area.
    fn set_frame(&self, widget: &Self::Widget, frame: Frame);

    /// Runs the platform event loop until the app quits.
    fn run(&self);
}
