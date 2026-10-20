//! The contract each platform (`AppKit`, `Win32`, `GTK`) implements with its own native widgets.

use crate::dialog::{DialogHandler, FileDialog};
use crate::geometry::{Frame, Size};
use crate::style::{ButtonStyle, LabelStyle, Surface};

pub type ClickHandler = Box<dyn Fn()>;
pub type TextHandler = Box<dyn Fn(&str)>;
pub type ToggleHandler = Box<dyn Fn(bool)>;
pub type ResizeHandler = Box<dyn Fn(Size)>;
pub type WakeHandler = Box<dyn Fn()>;

/// Asks the UI thread to run its wake handler; see [`Backend::waker`].
pub trait Waker: Send + Sync + 'static {
    fn wake(&self);
}

pub trait Backend: 'static {
    type Window;
    type Widget;
    type Waker: Waker;

    /// The window starts hidden; `on_resize` receives the new content size after every resize.
    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> Self::Window;
    fn content_size(&self, window: &Self::Window) -> Size;
    fn show_window(&self, window: &Self::Window);
    fn set_title(&self, window: &Self::Window, title: &str);
    /// Resizes the window's content area; its `on_resize` handler then reports the new size.
    fn set_content_size(&self, window: &Self::Window, size: Size);
    /// Closes the window, whose widgets have all been removed already.
    fn close_window(&self, window: Self::Window);
    /// How far the title bar reaches down over the content area, which is `0` where the
    /// title bar sits outside it.
    fn title_bar_inset(&self, window: &Self::Window) -> f32;
    /// Stops the user from making the content area narrower than `width`.
    fn set_min_content_width(&self, window: &Self::Window, width: f32);

    /// Draws `surface` behind the widgets created after it. Widgets stack in the order
    /// they are created, and a stack's surface is created before its children, so the
    /// surface is behind everything inside it. It is never [`Surface::None`].
    fn create_surface(&self, window: &Self::Window, surface: Surface) -> Self::Widget;

    fn create_label(&self, window: &Self::Window, text: &str, style: LabelStyle) -> Self::Widget;
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

    /// `on_wake` runs on the UI thread soon after each `wake`, which any thread may call.
    fn waker(&self, on_wake: WakeHandler) -> Self::Waker;

    /// Shows `dialog` over the app's active window, or on its own when no window is active,
    /// and passes how it closed to `on_close` on the UI thread. It is only called between
    /// events, so it may run a modal loop and call `on_close` before returning.
    fn show_file_dialog(&self, dialog: &FileDialog, on_close: DialogHandler);

    /// Runs the platform event loop until the app quits.
    fn run(&self);
}
