//! Platform-independent core of the Nifty runtime: a retained tree of native widgets with
//! flexbox layout, and the engine that keeps an app's windows in step with its state. Each
//! platform crate implements [`Backend`] with that OS's widgets.

mod backend;
mod dialog;
mod failure;
mod geometry;
mod layout;
mod paths;
mod reactive;
mod state;
mod style;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod ui;

pub use backend::{
    Backend, ClickHandler, ResizeHandler, TextHandler, ToggleHandler, WakeHandler, Waker,
};
pub use dialog::{DialogHandler, DialogKind, Dialogs, FileDialog, Picked, ShowDialog};
pub use failure::{Failure, MAX_CALL_DEPTH};
pub use geometry::{Frame, Point, Size};
pub use paths::{NoHomeFolder, expand_home};
pub use reactive::{
    Engine, ErrorHandler, Logic, Shown, Template, Templates, Widget, WindowTemplate,
};
pub use state::{StateId, StateSet};
pub use style::{
    Align, Axis, ButtonStyle, Justify, LabelStyle, StackStyle, Surface, TextAlign, TextColor,
    TextStyle,
};
pub use ui::{Node, Ui, WindowId};
