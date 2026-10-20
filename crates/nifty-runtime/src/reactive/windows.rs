//! The app's windows: which are open, building a window's content when it opens, and
//! closing it when code or the user asks.

use std::rc::Rc;

use super::{Engine, Logic, Mounted, WindowAction, WindowRequest, WindowTemplate};
use crate::backend::Backend;
use crate::geometry::Size;
use crate::ui::{Node, WindowCloseHandler, WindowId};

pub(super) struct MountedWindow<L: Logic> {
    pub(super) window: WindowId,
    /// The size its template asked for, which a reload compares against.
    pub(super) declared_size: Size,
    pub(super) content: Node,
    pub(super) children: Vec<Mounted<L>>,
}

/// An open window of the app being replaced, and the size its template asked for.
#[derive(Clone, Copy)]
pub(super) struct ReusedWindow {
    window: WindowId,
    declared_size: Size,
}

impl ReusedWindow {
    pub(super) fn of<L: Logic>(mounted: &MountedWindow<L>) -> Self {
        Self {
            window: mounted.window,
            declared_size: mounted.declared_size,
        }
    }

    pub(super) fn window(self) -> WindowId {
        self.window
    }
}

/// A window whose content is built but not yet showing, and its template's position.
pub(super) struct PreparedWindow<L: Logic> {
    index: usize,
    mounted: MountedWindow<L>,
    reused: Option<ReusedWindow>,
}

impl<L: Logic, B: Backend> Engine<L, B> {
    /// Which windows to open, by template: those the replaced app had open, `reused`, and
    /// new ones that start open. When none would be open, those that start open are, so
    /// the app keeps running.
    pub(super) fn windows_to_open(&self, reused: &[Option<ReusedWindow>]) -> Vec<bool> {
        let starts_open = |template: &WindowTemplate<L>| template.starts_open;
        let opening: Vec<bool> = self
            .templates
            .iter()
            .enumerate()
            .map(|(index, template)| {
                reused
                    .get(index)
                    .map_or_else(|| starts_open(template), Option::is_some)
            })
            .collect();
        if opening.contains(&true) {
            opening
        } else {
            self.templates.iter().map(starts_open).collect()
        }
    }

    /// Builds the content of the window at `index`, in the window `reused` if given or a
    /// new one, without showing it yet.
    pub(super) fn prepare_window(
        &self,
        index: usize,
        reused: Option<ReusedWindow>,
    ) -> Result<PreparedWindow<L>, L::Error> {
        let template = &self.templates[index];
        let window = match reused {
            Some(reused) => reused.window,
            None => self.ui.create_window(&template.title, template.size),
        };
        match self.mount_window(window, template) {
            Ok(mounted) => Ok(PreparedWindow {
                index,
                mounted,
                reused,
            }),
            Err(error) => {
                if reused.is_none() {
                    self.ui.close(window);
                }
                Err(error)
            }
        }
    }

    /// Puts built content in its window and shows it.
    pub(super) fn show_prepared(&self, prepared: PreparedWindow<L>) {
        let PreparedWindow {
            index,
            mounted,
            reused,
        } = prepared;
        let template = &self.templates[index];
        self.ui.set_backdrop(mounted.window, template.backdrop);
        self.ui.set_content(mounted.window, mounted.content);
        match reused {
            Some(reused) => {
                self.ui.set_title(mounted.window, &template.title);
                if reused.declared_size != mounted.declared_size {
                    self.ui.set_size(mounted.window, mounted.declared_size);
                }
            }
            None => self.ui.show(mounted.window),
        }
        self.windows.borrow_mut()[index] = Some(mounted);
    }

    /// Throws away content that was built but never shown, closing windows opened for it.
    pub(super) fn discard(&self, prepared: Vec<PreparedWindow<L>>) {
        for PreparedWindow {
            mounted, reused, ..
        } in prepared
        {
            self.ui.remove(mounted.content);
            if reused.is_none() {
                self.ui.close(mounted.window);
            }
        }
    }

    /// Builds a window's content without placing it in the window yet.
    fn mount_window(
        &self,
        window: WindowId,
        template: &WindowTemplate<L>,
    ) -> Result<MountedWindow<L>, L::Error> {
        let content = self.ui.content(window, &template.style);
        match self.mount_list(&template.children, &template.frame, content, &mut 0) {
            Ok(children) => Ok(MountedWindow {
                window,
                declared_size: template.size,
                content,
                children,
            }),
            Err(error) => {
                self.ui.remove(content);
                Err(error)
            }
        }
    }

    /// Opens and closes windows as `requests` ask, the last request for a window deciding
    /// what happens to it. Windows open before others close, so the app, which quits with
    /// its last window, keeps running when code swaps one window for another.
    pub(super) fn apply_window_requests(&self, requests: &[WindowRequest]) -> Result<(), L::Error> {
        let mut decided: Vec<WindowRequest> = Vec::new();
        for request in requests {
            match decided
                .iter_mut()
                .find(|decided| decided.window == request.window)
            {
                Some(decided) => decided.action = request.action,
                None => decided.push(*request),
            }
        }
        for request in &decided {
            if request.action == WindowAction::Open {
                self.open_window(request.window)?;
            }
        }
        for request in &decided {
            if request.action == WindowAction::Close {
                self.close_window(request.window);
            }
        }
        Ok(())
    }

    /// Opens the window at `index`, or brings it to the front if it is open.
    fn open_window(&self, index: usize) -> Result<(), L::Error> {
        let open = self.windows.borrow()[index]
            .as_ref()
            .map(|mounted| mounted.window);
        if let Some(window) = open {
            self.ui.focus(window);
            return Ok(());
        }
        let prepared = self.prepare_window(index, None)?;
        self.show_prepared(prepared);
        Ok(())
    }

    /// Closes the window at `index`, if it is open.
    fn close_window(&self, index: usize) {
        let closed = self.windows.borrow_mut()[index].take();
        if let Some(mounted) = closed {
            self.ui.close(mounted.window);
        }
    }

    /// Closes a window the user asked to close, as code closing it would.
    pub(super) fn on_close(&self) -> WindowCloseHandler {
        let engine = self.this.clone();
        Rc::new(move |window| {
            let Some(engine) = engine.upgrade() else {
                return;
            };
            let index = engine.windows.borrow().iter().position(|mounted| {
                mounted
                    .as_ref()
                    .is_some_and(|mounted| mounted.window == window)
            });
            match index {
                Some(index) => engine.close_window(index),
                None => engine.ui.close(window),
            }
        })
    }
}
