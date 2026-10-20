//! How compiled code says what a region shows: it calls `nf_show_*` on the sink it was
//! given, once per template list to show, in order.

use std::rc::Rc;

use nifty_runtime::{Shown, Templates};

use crate::logic::{Compiled, Frame, Region, ViewTemplate};
use crate::value::Slot;

pub struct Sink<'a> {
    region: &'a Region,
    views: &'a [ViewTemplate],
    parent: &'a Rc<Frame>,
    shown: Vec<(Templates<Compiled>, Pending)>,
}

impl std::fmt::Debug for Sink<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sink")
            .field("shown", &self.shown.len())
            .finish_non_exhaustive()
    }
}

/// The frame a shown list gets. Its own frames stay open until the expansion returns, so
/// compiled code can fill in their locals.
enum Pending {
    Parent,
    Own(Frame),
}

impl<'a> Sink<'a> {
    pub(crate) fn new(
        region: &'a Region,
        views: &'a [ViewTemplate],
        parent: &'a Rc<Frame>,
    ) -> Self {
        Self {
            region,
            views,
            parent,
            shown: Vec::new(),
        }
    }

    pub(crate) fn finish(self) -> Vec<Shown<Compiled>> {
        self.shown
            .into_iter()
            .map(|(templates, frame)| Shown {
                templates,
                frame: match frame {
                    Pending::Parent => Rc::clone(self.parent),
                    Pending::Own(frame) => Rc::new(frame),
                },
            })
            .collect()
    }

    /// # Panics
    /// When the region has no list `list`, which only wrongly compiled code asks for.
    fn list(&self, list: u32) -> Templates<Compiled> {
        let list =
            self.region.lists.get(list as usize).unwrap_or_else(|| {
                panic!("compiled code showed list {list}, which its region lacks")
            });
        Rc::clone(list)
    }

    /// Shows `templates` with `frame`, returning its locals; they live on the heap, so they
    /// stay put while the frame moves into the list.
    fn show_own(&mut self, templates: Templates<Compiled>, mut frame: Frame) -> *mut Slot {
        let locals = frame.as_mut_ptr();
        self.shown.push((templates, Pending::Own(frame)));
        locals
    }
}

/// Shows the region's list `list` with the region's own frame, as an `if` does.
///
/// # Safety
/// `sink` is the sink the runtime passed into the compiled code calling.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_show_same(sink: *mut Sink<'_>, list: u32) {
    // SAFETY: by the caller's contract.
    let sink = unsafe { &mut *sink };
    let templates = sink.list(list);
    sink.shown.push((templates, Pending::Parent));
}

/// Shows the region's list `list` with a copy of the region's frame, and returns that
/// copy's locals for compiled code to fill in, as a `for` does for each item. They stay
/// valid until the expansion returns.
///
/// # Safety
/// `sink` is the sink the runtime passed into the compiled code calling.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_show_item(sink: *mut Sink<'_>, list: u32) -> *mut Slot {
    // SAFETY: by the caller's contract.
    let sink = unsafe { &mut *sink };
    let templates = sink.list(list);
    let frame = Frame::clone(sink.parent);
    sink.show_own(templates, frame)
}

/// Shows view `view`'s body with a new frame of its locals, all empty, and returns them
/// for compiled code to fill in with the arguments. They stay valid until the expansion
/// returns.
///
/// # Safety
/// `sink` is the sink the runtime passed into the compiled code calling.
///
/// # Panics
/// When there is no view `view`, which only wrongly compiled code asks for.
#[unsafe(no_mangle)]
pub(crate) unsafe extern "C" fn nf_show_view(sink: *mut Sink<'_>, view: u32) -> *mut Slot {
    // SAFETY: by the caller's contract.
    let sink = unsafe { &mut *sink };
    let view = sink
        .views
        .get(view as usize)
        .unwrap_or_else(|| panic!("compiled code showed view {view}, which doesn't exist"));
    let (templates, frame) = (Rc::clone(&view.body), Frame::empty(view.frame));
    sink.show_own(templates, frame)
}
