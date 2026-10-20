//! What an app's windows are made of, and the [`Logic`] that gives their dynamic parts
//! their values.

use std::fmt;
use std::rc::Rc;

use crate::geometry::Size;
use crate::state::StateSet;
use crate::style::{Backdrop, ButtonStyle, Icon, LabelStyle, StackStyle, ToggleStyle};

/// The program behind an app's UI. Templates hold its handles to values, bindings, actions
/// and regions, and the engine asks it to evaluate them against a frame of locals.
///
/// The interpreter implements it by walking checked code, and compiled apps by calling
/// compiled functions.
pub trait Logic: Sized + 'static {
    /// The locals visible to a piece of UI, such as loop variables and view parameters.
    type Frame: 'static;
    /// A value the UI shows, or reads when the user acts, such as a label's text or a nav
    /// item's value. Each is read as the type the checker gave it.
    type Value: Clone + fmt::Debug + 'static;
    /// A place a text field or checkbox shows, and writes the user's edits to.
    type Binding: Clone + fmt::Debug + 'static;
    /// What a button does when clicked.
    type Action: Clone + fmt::Debug + 'static;
    /// An `if`, `for` or view call: UI whose shape depends on state.
    type Region: fmt::Debug + 'static;
    type Error: 'static;

    /// # Errors
    /// When evaluating the text fails.
    fn text(&self, text: &Self::Value, frame: &Self::Frame) -> Result<String, Self::Error>;

    /// # Errors
    /// When evaluating the number fails.
    fn number(&self, number: &Self::Value, frame: &Self::Frame) -> Result<i64, Self::Error>;

    /// # Errors
    /// When evaluating the list fails.
    fn texts(&self, texts: &Self::Value, frame: &Self::Frame) -> Result<Vec<String>, Self::Error>;

    /// # Errors
    /// When reading the bound place fails.
    fn read_text(
        &self,
        binding: &Self::Binding,
        frame: &Self::Frame,
    ) -> Result<String, Self::Error>;

    /// # Errors
    /// When reading the bound place fails.
    fn read_checked(
        &self,
        binding: &Self::Binding,
        frame: &Self::Frame,
    ) -> Result<bool, Self::Error>;

    /// # Errors
    /// When reading the bound place fails.
    fn read_number(&self, binding: &Self::Binding, frame: &Self::Frame)
    -> Result<i64, Self::Error>;

    /// # Errors
    /// When writing the bound place fails.
    fn write_text(
        &self,
        binding: &Self::Binding,
        frame: &Self::Frame,
        text: &str,
    ) -> Result<(), Self::Error>;

    /// # Errors
    /// When writing the bound place fails.
    fn write_checked(
        &self,
        binding: &Self::Binding,
        frame: &Self::Frame,
        checked: bool,
    ) -> Result<(), Self::Error>;

    /// # Errors
    /// When writing the bound place fails.
    fn write_number(
        &self,
        binding: &Self::Binding,
        frame: &Self::Frame,
        number: i64,
    ) -> Result<(), Self::Error>;

    /// # Errors
    /// When the action fails part-way.
    fn run(&self, action: &Self::Action, frame: &Self::Frame) -> Result<(), Self::Error>;

    /// What `region` shows now, in order: lists of templates, each with its own frame.
    ///
    /// # Errors
    /// When evaluating what decides the region's contents fails.
    fn expand(
        &self,
        region: &Self::Region,
        frame: &Rc<Self::Frame>,
    ) -> Result<Vec<Shown<Self>>, Self::Error>;

    /// The states changed since the last call.
    fn take_changed(&self) -> StateSet;
}

pub type Templates<L> = Rc<[Template<L>]>;

/// One element of a window's UI, before it is shown.
pub enum Template<L: Logic> {
    /// Its children scroll up and down inside it when `scrolls` and they are too tall.
    Stack {
        style: StackStyle,
        children: Templates<L>,
        scrolls: bool,
    },
    /// Updated in place when `deps` change.
    Widget { widget: Widget<L>, deps: StateSet },
    /// Rebuilt in place when `deps` change.
    Region { region: L::Region, deps: StateSet },
}

pub enum Widget<L: Logic> {
    Label {
        text: L::Value,
        style: LabelStyle,
    },
    Button {
        title: L::Value,
        style: ButtonStyle,
        icon: Icon,
        action: L::Action,
    },
    TextField {
        value: L::Binding,
        placeholder: String,
    },
    Checkbox {
        checked: L::Binding,
        title: Option<L::Value>,
        style: ToggleStyle,
    },
    /// Shows the number at `value`, from `min` to `max`, and writes the user's pick there.
    Slider {
        value: L::Binding,
        min: i64,
        max: i64,
    },
    /// Shows `selection` among `options`, a list of text, and writes the user's pick there.
    Picker {
        selection: L::Binding,
        options: L::Value,
    },
    /// Selected while `selection` holds `value`; picking it writes `value` there.
    NavItem {
        title: L::Value,
        value: L::Value,
        selection: L::Binding,
        icon: Icon,
        badge: Option<L::Value>,
    },
    /// A progress bar at `value` percent, or busy without one.
    Progress {
        value: Option<L::Value>,
    },
    Spacer,
    Divider,
}

/// Part of what a region shows: `templates`, with `frame` as their locals.
pub struct Shown<L: Logic> {
    pub templates: Templates<L>,
    pub frame: Rc<L::Frame>,
}

/// A window and the column it shows its content in.
pub struct WindowTemplate<L: Logic> {
    pub title: String,
    pub size: Size,
    pub backdrop: Backdrop,
    pub style: StackStyle,
    pub children: Templates<L>,
    /// The locals its content starts with.
    pub frame: L::Frame,
}

impl<L: Logic> fmt::Debug for Template<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stack {
                style,
                children,
                scrolls,
            } => f
                .debug_struct("Stack")
                .field("style", style)
                .field("children", children)
                .field("scrolls", scrolls)
                .finish(),
            Self::Widget { widget, deps } => f
                .debug_struct("Widget")
                .field("widget", widget)
                .field("deps", deps)
                .finish(),
            Self::Region { region, deps } => f
                .debug_struct("Region")
                .field("region", region)
                .field("deps", deps)
                .finish(),
        }
    }
}

impl<L: Logic> fmt::Debug for Widget<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Label { text, style } => f
                .debug_struct("Label")
                .field("text", text)
                .field("style", style)
                .finish(),
            Self::Button {
                title,
                style,
                icon,
                action,
            } => f
                .debug_struct("Button")
                .field("title", title)
                .field("style", style)
                .field("icon", icon)
                .field("action", action)
                .finish(),
            Self::TextField { value, placeholder } => f
                .debug_struct("TextField")
                .field("value", value)
                .field("placeholder", placeholder)
                .finish(),
            Self::Checkbox {
                checked,
                title,
                style,
            } => f
                .debug_struct("Checkbox")
                .field("checked", checked)
                .field("title", title)
                .field("style", style)
                .finish(),
            Self::Slider { value, min, max } => f
                .debug_struct("Slider")
                .field("value", value)
                .field("min", min)
                .field("max", max)
                .finish(),
            Self::Picker { selection, options } => f
                .debug_struct("Picker")
                .field("selection", selection)
                .field("options", options)
                .finish(),
            Self::NavItem {
                title,
                value,
                selection,
                icon,
                badge,
            } => f
                .debug_struct("NavItem")
                .field("title", title)
                .field("value", value)
                .field("selection", selection)
                .field("icon", icon)
                .field("badge", badge)
                .finish(),
            Self::Progress { value } => f.debug_struct("Progress").field("value", value).finish(),
            Self::Spacer => f.write_str("Spacer"),
            Self::Divider => f.write_str("Divider"),
        }
    }
}

impl<L: Logic> fmt::Debug for Shown<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shown")
            .field("templates", &self.templates)
            .finish_non_exhaustive()
    }
}

impl<L: Logic> fmt::Debug for WindowTemplate<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WindowTemplate")
            .field("title", &self.title)
            .field("size", &self.size)
            .field("backdrop", &self.backdrop)
            .field("style", &self.style)
            .field("children", &self.children)
            .finish_non_exhaustive()
    }
}
