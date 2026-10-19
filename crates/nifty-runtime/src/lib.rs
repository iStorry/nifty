//! Platform-independent core of the nifty runtime: reactivity, the view tree and layout.
//! Each platform crate implements [`Backend`] with that OS's native widgets.

mod backend;
mod geometry;
mod layout;
mod mount;
mod reactive;
mod view;

pub use backend::{Backend, ClickHandler, ResizeHandler};
pub use geometry::{Frame, Point, Size};
pub use mount::run;
pub use reactive::{Effect, Signal};
pub use view::{
    Align, Axis, Button, IntoText, Justify, Label, Stack, Text, View, Window, button, column,
    label, row,
};
