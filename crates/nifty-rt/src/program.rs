//! The descriptor a compiled program hands the runtime: its states, windows, views and the
//! compiled functions behind every dynamic part of their UI.
//!
//! `nf build` emits these as data next to the machine code, laid out as the `#[repr(C)]`
//! types here say, so their layout is the contract between the two.

use nifty_runtime::{Axis, Backdrop, ButtonStyle, Icon, LabelStyle, StackStyle, TextStyle};

use crate::context::Context;
use crate::ffi::{Slice, Str};
use crate::show::Sink;
use crate::value::{Slot, TypeDesc};

/// Bumped whenever these layouts or the meaning of a runtime call change, so a program
/// built against another runtime is refused instead of misread.
pub const ABI_VERSION: u32 = 2;

/// Writes a state's starting value to `out`.
pub type InitFn = unsafe extern "C" fn(context: *const Context, out: *mut Slot) -> bool;
/// Writes the value a piece of UI shows, such as a string, an `int` or a `bool`, to `out`;
/// `frame` holds its locals.
pub type ReadFn =
    unsafe extern "C" fn(context: *const Context, frame: *const Slot, out: *mut Slot) -> bool;
/// Writes `value`, which it takes ownership of, to a bound place.
pub type WriteFn =
    unsafe extern "C" fn(context: *const Context, frame: *const Slot, value: Slot) -> bool;
/// Runs a button's action in a copy of its frame, which it may change.
pub type ActionFn = unsafe extern "C" fn(context: *const Context, frame: *mut Slot) -> bool;
/// Tells `sink` what a region shows now, through the `nf_show_*` calls.
pub type ExpandFn =
    unsafe extern "C" fn(context: *const Context, frame: *const Slot, sink: *mut Sink) -> bool;

/// Every function returns `false` after a runtime call recorded why it failed.
#[derive(Debug)]
#[repr(C)]
pub struct ProgramDesc {
    pub abi_version: u32,
    pub states: Slice<StateDesc>,
    pub windows: Slice<WindowDesc>,
    /// Indexed by view id.
    pub views: Slice<ViewDesc>,
    /// Indexed by function id, for errors that name a function.
    pub functions: Slice<Str>,
    /// Indexed by span id, for errors that point at code.
    pub spans: Slice<SpanDesc>,
}

#[derive(Debug)]
#[repr(C)]
pub struct StateDesc {
    pub ty: &'static TypeDesc,
    pub init: InitFn,
}

#[derive(Debug)]
#[repr(C)]
pub struct SpanDesc {
    pub file: Str,
    pub line: u32,
    pub column: u32,
}

/// The types of a frame's locals; a frame starts with every local empty.
#[derive(Debug)]
#[repr(C)]
pub struct FrameDesc {
    pub locals: Slice<&'static TypeDesc>,
}

#[derive(Debug)]
#[repr(C)]
pub struct WindowDesc {
    pub title: Str,
    pub width: f32,
    pub height: f32,
    pub backdrop: Backdrop,
    pub style: StackStyle,
    pub children: Slice<TemplateDesc>,
    pub frame: FrameDesc,
}

#[derive(Debug)]
#[repr(C)]
pub struct ViewDesc {
    pub body: Slice<TemplateDesc>,
    /// Its parameters are the first locals, in order.
    pub frame: FrameDesc,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TemplateKind {
    Stack,
    Label,
    Button,
    TextField,
    Checkbox,
    Spacer,
    Divider,
    Region,
    NavItem,
    Progress,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct BindingDesc {
    pub read: Option<ReadFn>,
    pub write: Option<WriteFn>,
}

#[derive(Debug)]
#[repr(C)]
pub struct RegionDesc {
    pub expand: Option<ExpandFn>,
    /// The template lists it can show, which `nf_show_same` and `nf_show_item` pick by
    /// index.
    pub lists: Slice<Slice<TemplateDesc>>,
}

/// One element of a window's UI. Which fields matter depends on `kind`:
///
/// - `Stack`: `stack` and `children`.
/// - `Label`: `label` and `text`.
/// - `Button`: `button`, `icon`, `text` as its title, and `action`.
/// - `TextField`: `binding` and `placeholder`.
/// - `Checkbox`: `binding`, and `text` as its title if it has one.
/// - `NavItem`: `text` as its title, `value`, `binding` as its selection, `icon`, and
///   `badge` if it has one.
/// - `Progress`: `value` as its percentage, if it has one.
/// - `Region`: `region`.
///
/// `deps` are the state ids whose changes require showing a widget again, or rebuilding a
/// region.
#[derive(Debug)]
#[repr(C)]
pub struct TemplateDesc {
    pub kind: TemplateKind,
    pub button: ButtonStyle,
    pub icon: Icon,
    pub deps: Slice<u32>,
    pub stack: StackStyle,
    pub label: LabelStyle,
    pub text: Option<ReadFn>,
    pub value: Option<ReadFn>,
    pub badge: Option<ReadFn>,
    pub binding: BindingDesc,
    pub action: Option<ActionFn>,
    pub placeholder: Str,
    pub children: Slice<TemplateDesc>,
    pub region: RegionDesc,
}

impl TemplateDesc {
    /// A template of `kind` with every other field empty, to build on.
    #[must_use]
    pub const fn empty(kind: TemplateKind) -> Self {
        Self {
            kind,
            button: ButtonStyle::Normal,
            icon: Icon::None,
            deps: Slice::EMPTY,
            stack: StackStyle::new(Axis::Vertical),
            label: LabelStyle::new(TextStyle::Body),
            text: None,
            value: None,
            badge: None,
            binding: BindingDesc {
                read: None,
                write: None,
            },
            action: None,
            placeholder: Str::EMPTY,
            children: Slice::EMPTY,
            region: RegionDesc {
                expand: None,
                lists: Slice::EMPTY,
            },
        }
    }
}
