//! The contract each platform (`AppKit`, `Win32`, `GTK`) implements with its own native widgets.

use crate::geometry::{Frame, Size};
use crate::style::{ButtonStyle, TextStyle};

pub type ClickHandler = Box<dyn Fn()>;
pub type TextHandler = Box<dyn Fn(&str)>;
pub type ToggleHandler = Box<dyn Fn(bool)>;
pub type ResizeHandler = Box<dyn Fn(Size)>;

pub trait Backend: 'static {
    type Window;
    type Widget;

    /// The window starts hidden; `on_resize` receives the new content size after every resize.
    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> Self::Window;
    fn content_size(&self, window: &Self::Window) -> Size;
    fn show_window(&self, window: &Self::Window);

    fn create_label(&self, window: &Self::Window, text: &str, style: TextStyle) -> Self::Widget;
    fn create_button(
        &self,
        window: &Self::Window,
        title: &str,
        style: ButtonStyle,
        on_click: ClickHandler,
    ) -> Self::Widget;
    /// `on_change` runs after each edit the user makes, not when `set_text` changes the value.
    fn create_text_field(
        &self,
        window: &Self::Window,
        text: &str,
        placeholder: &str,
        on_change: TextHandler,
    ) -> Self::Widget;
    /// `on_toggle` runs when the user toggles it, not when `set_checked` changes it.
    fn create_checkbox(
        &self,
        window: &Self::Window,
        title: &str,
        checked: bool,
        on_toggle: ToggleHandler,
    ) -> Self::Widget;
    /// A thin line separating content; it is drawn along its longer side.
    fn create_divider(&self, window: &Self::Window) -> Self::Widget;

    /// Sets a label's text, a button's title or a text field's value.
    fn set_text(&self, widget: &Self::Widget, text: &str);
    fn set_checked(&self, widget: &Self::Widget, checked: bool);
    /// The widget's natural size for its current content.
    fn measure(&self, widget: &Self::Widget) -> Size;
    /// Positions the widget relative to the window's content area.
    fn set_frame(&self, widget: &Self::Widget, frame: Frame);
    /// Takes the widget off screen. It may be the widget whose handler is running right now,
    /// so the backend must keep that handler alive until the handler returns.
    fn remove_widget(&self, widget: Self::Widget);

    /// Runs the platform event loop until the app quits.
    fn run(&self);
}
