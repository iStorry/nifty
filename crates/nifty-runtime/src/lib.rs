//! Platform-independent core of the Nifty runtime: a retained tree of native widgets with
//! flexbox layout. Each platform crate implements [`Backend`] with that OS's widgets.

mod backend;
mod geometry;
mod layout;
mod style;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
mod ui;

pub use backend::{
    Backend, ClickHandler, ResizeHandler, TextHandler, ToggleHandler, WakeHandler, Waker,
};
pub use geometry::{Frame, Point, Size};
pub use style::{
    Align, Axis, ButtonStyle, Justify, LabelStyle, StackStyle, TextAlign, TextColor, TextStyle,
};
pub use ui::{Node, Ui, WindowId};
