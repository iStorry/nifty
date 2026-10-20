//! Compiled code as the runtime engine's [`Logic`]: each template's dynamic parts are
//! compiled functions, which this calls with the app's context and the template's frame.

use std::fmt;
use std::rc::Rc;

use nifty_runtime::{Logic, Shown, StateSet, Templates};

use crate::context::{AppError, Context};
use crate::program::{ActionFn, ExpandFn, FrameDesc, ReadFn, WriteFn};
use crate::show::Sink;
use crate::value::{BOOL, STRING, Slot, TypeDesc, Value, release, retain, string_slot};

/// The locals visible to a piece of UI. A frame owns its values, released when it drops.
pub struct Frame {
    slots: Box<[Slot]>,
    types: &'static [&'static TypeDesc],
}

impl Frame {
    /// A frame for `desc`'s locals, all empty.
    pub(crate) fn empty(desc: &'static FrameDesc) -> Self {
        let types = desc.locals.items();
        Self {
            slots: vec![0; types.len()].into(),
            types,
        }
    }

    pub(crate) fn as_ptr(&self) -> *const Slot {
        self.slots.as_ptr()
    }

    pub(crate) fn as_mut_ptr(&mut self) -> *mut Slot {
        self.slots.as_mut_ptr()
    }
}

impl Clone for Frame {
    fn clone(&self) -> Self {
        for (slot, ty) in self.slots.iter().zip(self.types) {
            retain(*slot, ty);
        }
        Self {
            slots: self.slots.clone(),
            types: self.types,
        }
    }
}

impl Drop for Frame {
    fn drop(&mut self) {
        for (slot, ty) in self.slots.iter().zip(self.types) {
            release(*slot, ty);
        }
    }
}

impl fmt::Debug for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Frame")
            .field("locals", &self.slots.len())
            .finish_non_exhaustive()
    }
}

/// A text field's or checkbox's place: how to read it and how to write the user's edits.
#[derive(Clone, Copy, Debug)]
pub struct Binding {
    pub(crate) read: ReadFn,
    pub(crate) write: WriteFn,
}

/// An `if`, `for` or view call: compiled code picks which of `lists` it shows.
#[derive(Debug)]
pub struct Region {
    pub(crate) expand: ExpandFn,
    pub(crate) lists: Vec<Templates<Compiled>>,
}

#[derive(Debug)]
pub(crate) struct ViewTemplate {
    pub body: Templates<Compiled>,
    pub frame: &'static FrameDesc,
}

pub struct Compiled {
    context: Context,
    /// Indexed by view id.
    views: Vec<ViewTemplate>,
}

impl Compiled {
    pub(crate) fn new(context: Context, views: Vec<ViewTemplate>) -> Self {
        Self { context, views }
    }

    #[cfg(test)]
    pub(crate) fn context(&self) -> &Context {
        &self.context
    }

    /// Runs a compiled call, turning its `false` into the failure it recorded.
    fn call(&self, run: impl FnOnce(*const Context) -> bool) -> Result<(), AppError> {
        if run(&raw const self.context) {
            Ok(())
        } else {
            Err(self.context.take_error())
        }
    }

    /// The value a compiled read gives, owned by the caller.
    fn read(&self, read: ReadFn, frame: &Frame, ty: &'static TypeDesc) -> Result<Value, AppError> {
        let mut out = 0;
        // SAFETY: compiled reads take the context and their template's frame, and write an
        // owned value of the template's type to `out`.
        self.call(|context| unsafe { read(context, frame.as_ptr(), &raw mut out) })?;
        Ok(Value::owned(out, ty))
    }

    fn write(&self, binding: &Binding, frame: &Frame, value: Slot) -> Result<(), AppError> {
        // SAFETY: compiled writes take the context, their template's frame and an owned
        // value of the bound place's type.
        self.call(|context| unsafe { (binding.write)(context, frame.as_ptr(), value) })
    }
}

impl Logic for Compiled {
    type Frame = Frame;
    type Text = ReadFn;
    type Binding = Binding;
    type Action = ActionFn;
    type Region = Region;
    type Error = AppError;

    fn text(&self, text: &ReadFn, frame: &Frame) -> Result<String, AppError> {
        let value = self.read(*text, frame, &STRING)?;
        Ok(crate::value::as_str(value.slot()).to_owned())
    }

    fn read_text(&self, binding: &Binding, frame: &Frame) -> Result<String, AppError> {
        self.text(&binding.read, frame)
    }

    fn read_checked(&self, binding: &Binding, frame: &Frame) -> Result<bool, AppError> {
        Ok(self.read(binding.read, frame, &BOOL)?.slot() != 0)
    }

    fn write_text(&self, binding: &Binding, frame: &Frame, text: &str) -> Result<(), AppError> {
        self.write(binding, frame, string_slot(text))
    }

    fn write_checked(
        &self,
        binding: &Binding,
        frame: &Frame,
        checked: bool,
    ) -> Result<(), AppError> {
        self.write(binding, frame, Slot::from(checked))
    }

    fn run(&self, action: &ActionFn, frame: &Frame) -> Result<(), AppError> {
        let mut scratch = frame.clone();
        // SAFETY: compiled actions take the context and a frame of their locals to change.
        self.call(|context| unsafe { action(context, scratch.as_mut_ptr()) })
    }

    fn expand(&self, region: &Region, frame: &Rc<Frame>) -> Result<Vec<Shown<Self>>, AppError> {
        let mut sink = Sink::new(region, &self.views, frame);
        // SAFETY: compiled expansions take the context, their region's frame and a sink
        // they only pass to `nf_show_*`.
        self.call(|context| unsafe { (region.expand)(context, frame.as_ptr(), &raw mut sink) })?;
        Ok(sink.finish())
    }

    fn take_changed(&self) -> StateSet {
        self.context.take_changed()
    }
}

impl fmt::Debug for Compiled {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Compiled")
            .field("context", &self.context)
            .finish_non_exhaustive()
    }
}
