//! The contract each platform (`AppKit`, `Win32`, `GTK`) implements with its own native widgets.

use crate::dialog::{DialogHandler, FileDialog};
use crate::geometry::{Frame, Size};
use std::path::Path;

use crate::menu::Menu;
use crate::style::{Backdrop, ButtonStyle, Icon, ImageSize, LabelStyle, Surface, ToggleStyle};

pub type ClickHandler = Box<dyn Fn()>;
pub type TextHandler = Box<dyn Fn(&str)>;
pub type ToggleHandler = Box<dyn Fn(bool)>;
pub type NumberHandler = Box<dyn Fn(i64)>;
/// Runs with the number of the menu item the user chose.
pub type MenuHandler = Box<dyn Fn(usize)>;
pub type ResizeHandler = Box<dyn Fn(Size)>;
pub type WakeHandler = Box<dyn Fn()>;

/// Where a new widget goes: straight into a window's content area, or into the content of
/// a scroll area, which scrolls with it. Frames are relative to that parent's content.
pub enum Parent<'a, B: Backend + ?Sized> {
    Window(&'a B::Window),
    Scroll(&'a B::Widget),
}

impl<B: Backend + ?Sized> Clone for Parent<'_, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<B: Backend + ?Sized> Copy for Parent<'_, B> {}

impl<B: Backend + ?Sized> std::fmt::Debug for Parent<'_, B> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Window(_) => "Window",
            Self::Scroll(_) => "Scroll",
        })
    }
}

/// Asks the UI thread to run its wake handler; see [`Backend::waker`].
pub trait Waker: Send + Sync + 'static {
    fn wake(&self);
}

pub trait Backend: 'static {
    type Window;
    type Widget;
    type Waker: Waker;

    /// The window starts hidden; `on_resize` receives the new content size after every
    /// resize, and whenever something else changes how the content must be laid out, such
    /// as the width of scroll bars.
    fn open_window(&self, title: &str, size: Size, on_resize: ResizeHandler) -> Self::Window;
    fn content_size(&self, window: &Self::Window) -> Size;
    fn show_window(&self, window: &Self::Window);
    fn set_title(&self, window: &Self::Window, title: &str);
    /// Draws the window on `backdrop`, or as close to it as the system allows.
    fn set_backdrop(&self, window: &Self::Window, backdrop: Backdrop);
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
    fn create_surface(&self, parent: Parent<'_, Self>, surface: Surface) -> Self::Widget;
    /// An area whose content scrolls up and down when it is taller than the area. Widgets
    /// created with it as their [`Parent::Scroll`] are its content.
    fn create_scroll(&self, parent: Parent<'_, Self>) -> Self::Widget;
    /// How much of a scroll area's width its vertical scroll bar takes from the content, in
    /// points; `0` where scroll bars float over the content. It is asked again at every
    /// layout, since the user can change it while the app runs.
    fn scroll_bar_width(&self, window: &Self::Window) -> f32;
    /// Sets the size of a scroll area's content, which is as wide as the area less its
    /// scroll bar, and may be taller than the area.
    fn set_scroll_content(&self, scroll: &Self::Widget, size: Size);

    fn create_label(&self, parent: Parent<'_, Self>, text: &str, style: LabelStyle)
    -> Self::Widget;
    /// A button that shows whether it is selected when `selected` is `Some`; clicking
    /// it leaves that to `set_checked`.
    fn create_button(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        style: ButtonStyle,
        icon: Icon,
        selected: Option<bool>,
        on_click: ClickHandler,
    ) -> Self::Widget;
    /// A row of a sidebar list: an icon, a title and a badge such as a count, highlighted
    /// while `selected`. `on_click` runs when the user picks it; it stays as it is until
    /// `set_checked` changes it.
    fn create_nav_item(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        icon: Icon,
        badge: &str,
        selected: bool,
        on_click: ClickHandler,
    ) -> Self::Widget;
    /// A text field `lines` lines tall: one line, or more that wrap the text, take
    /// **Return** as a new line, and scroll; only a one-line field shows its placeholder.
    /// `on_change` runs after each edit the user makes, not when `set_text` changes the
    /// value, with lines ending in `\n`. A one-line field with `on_submit` runs it when
    /// the user presses **Return** in it, instead of pressing the window's primary button.
    fn create_text_field(
        &self,
        parent: Parent<'_, Self>,
        text: &str,
        placeholder: &str,
        lines: u32,
        on_change: TextHandler,
        on_submit: Option<ClickHandler>,
    ) -> Self::Widget;
    /// A checkbox ticks a box before its title, and a switch sits at the end of its title.
    /// `on_toggle` runs when the user toggles it, not when `set_checked` changes it.
    fn create_checkbox(
        &self,
        parent: Parent<'_, Self>,
        title: &str,
        checked: bool,
        style: ToggleStyle,
        on_toggle: ToggleHandler,
    ) -> Self::Widget;
    /// A slider for whole numbers from `min` to `max`, at `value`. `on_change` runs with
    /// each new value as the user drags it, not when `set_value` moves it.
    fn create_slider(
        &self,
        parent: Parent<'_, Self>,
        value: i64,
        min: i64,
        max: i64,
        on_change: NumberHandler,
    ) -> Self::Widget;
    /// A pop-up menu of `options`, showing the one equal to `selected`, or none when no
    /// option is. `on_pick` runs with the option the user picks, not when `set_text` or
    /// `set_options` change what it shows.
    fn create_picker(
        &self,
        parent: Parent<'_, Self>,
        options: &[String],
        selected: &str,
        on_pick: TextHandler,
    ) -> Self::Widget;
    /// The image in the file at `path`, as large as fits its frame, keeping its shape, and
    /// centred; it measures as `size.frame` of the image's own size, in points. With no
    /// path, or a file that can't be read as an image, it shows nothing.
    fn create_image(
        &self,
        parent: Parent<'_, Self>,
        path: Option<&Path>,
        size: ImageSize,
    ) -> Self::Widget;
    /// Shows the image at `path` instead, as `create_image` does.
    fn set_image(&self, widget: &Self::Widget, path: Option<&Path>);
    /// A thin line separating content; it is drawn along its longer side.
    fn create_divider(&self, parent: Parent<'_, Self>) -> Self::Widget;
    /// A progress bar filled to `percent`, from 0 to 100, or one showing that work is under
    /// way, animated, when there is no percentage.
    fn create_progress(&self, parent: Parent<'_, Self>, percent: Option<i64>) -> Self::Widget;

    /// Sets a label's text, a button's or nav item's title, a text field's value, or the
    /// option a picker shows.
    fn set_text(&self, widget: &Self::Widget, text: &str);
    /// Ticks a checkbox or switch, highlights a nav item as selected, or shows a button
    /// made with a selection as selected.
    fn set_checked(&self, widget: &Self::Widget, checked: bool);
    /// Sets a nav item's badge; empty hides it.
    fn set_badge(&self, widget: &Self::Widget, badge: &str);
    /// Fills a progress bar to `value` percent, from 0 to 100, or moves a slider to `value`.
    fn set_value(&self, widget: &Self::Widget, value: i64);
    /// Replaces a picker's options, keeping the one it shows if it is still there.
    fn set_options(&self, widget: &Self::Widget, options: &[String]);
    /// The widget's natural size for its current content.
    fn measure(&self, widget: &Self::Widget) -> Size;
    /// Positions the widget relative to its parent's content.
    fn set_frame(&self, widget: &Self::Widget, frame: Frame);
    /// Takes the widget off screen. It may be the widget whose handler is running right now,
    /// so the backend must keep that handler alive until the handler returns.
    fn remove_widget(&self, widget: Self::Widget);

    /// Gives the app `menus`, in place of any it had, alongside the system's own: in the
    /// menu bar on macOS, and in every window's menu bar on Windows, with their items'
    /// shortcuts working wherever the app has the keyboard. `on_select` runs with an
    /// item's number when the user chooses it or presses its shortcut.
    fn set_menu_bar(&self, menus: &[Menu<usize>], on_select: MenuHandler);

    /// `on_wake` runs on the UI thread soon after each `wake`, which any thread may call.
    fn waker(&self, on_wake: WakeHandler) -> Self::Waker;

    /// Shows `dialog` over the app's active window, or on its own when no window is active,
    /// and passes how it closed to `on_close` on the UI thread. It is only called between
    /// events, so it may run a modal loop and call `on_close` before returning.
    fn show_file_dialog(&self, dialog: &FileDialog, on_close: DialogHandler);

    /// Runs the platform event loop until the app quits.
    fn run(&self);
}
