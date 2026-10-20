//! Platform-independent core of the Nifty runtime: a retained tree of native widgets with
//! flexbox layout, and the engine that keeps an app's windows in step with its state. Each
//! platform crate implements [`Backend`] with that OS's widgets.

#[macro_use]
mod coded;

mod backend;
mod dialog;
mod failure;
mod geometry;
mod layout;
mod menu;
mod paths;
mod reactive;
mod state;
mod style;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod ui;

pub use backend::{
    Backend, ClickHandler, MenuHandler, NumberHandler, Parent, ResizeHandler, TextHandler,
    ToggleHandler, WakeHandler, Waker,
};
pub use dialog::{DialogHandler, DialogKind, Dialogs, FileDialog, Picked, ShowDialog};
pub use failure::{Failure, MAX_CALL_DEPTH};
pub use geometry::{Frame, Point, Size};
pub use menu::{ALT, Key, Menu, MenuEntry, SHIFT, Shortcut, ShortcutError, SystemCommand};
pub use paths::{NoHomeFolder, expand_home};
pub use reactive::{
    AppTemplate, Engine, ErrorHandler, Logic, MenuBar, Shown, Template, Templates, Widget,
    WindowTemplate,
};
pub use state::{StateId, StateSet};
pub use style::{
    Align, Axis, Backdrop, ButtonStyle, Icon, ImageSize, Justify, LabelStyle, StackStyle, Surface,
    TextAlign, TextColor, TextStyle, ToggleStyle,
};
pub use ui::{Node, Position, Ui, WindowId};
