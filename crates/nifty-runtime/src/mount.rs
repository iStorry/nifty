//! Turns a [`Window`] description into a live native window.

use std::cell::RefCell;
use std::rc::Rc;

use taffy::NodeId;

use crate::backend::Backend;
use crate::layout::Layout;
use crate::reactive::Effect;
use crate::view::{Text, View, Window};

/// Opens `window` on `backend` and runs the event loop until the app quits.
pub fn run<B: Backend>(backend: B, window: Window) {
    let backend = Rc::new(backend);
    let _window = mount(&backend, window);
    backend.run();
}

/// Owns everything that must stay alive for the window to keep working.
struct MountedWindow<B: Backend> {
    _native: B::Window,
    _text_effects: Vec<Effect>,
}

fn mount<B: Backend>(backend: &Rc<B>, window: Window) -> MountedWindow<B> {
    let layout = Rc::new(RefCell::new(Layout::new(Rc::clone(backend))));
    let native = backend.open_window(
        &window.title,
        window.size,
        Box::new({
            let layout = Rc::clone(&layout);
            move |size| layout.borrow_mut().resize(size)
        }),
    );

    let text_bindings = {
        let mut layout = layout.borrow_mut();
        let mut builder = Builder {
            backend: backend.as_ref(),
            window: &native,
            layout: &mut layout,
            text_bindings: Vec::new(),
        };
        let content = builder.build(window.content);
        builder.layout.set_content(content);
        builder.text_bindings
    };

    let text_effects = text_bindings
        .into_iter()
        .map(|binding| binding.into_effect(&layout))
        .collect();
    layout.borrow_mut().resize(backend.content_size(&native));
    backend.show_window(&native);

    MountedWindow {
        _native: native,
        _text_effects: text_effects,
    }
}

struct TextBinding {
    node: NodeId,
    text: Box<dyn Fn() -> String>,
}

impl TextBinding {
    fn into_effect<B: Backend>(self, layout: &Rc<RefCell<Layout<B>>>) -> Effect {
        let layout = Rc::clone(layout);
        Effect::new(move || {
            let text = (self.text)();
            layout.borrow_mut().set_text(self.node, &text);
        })
    }
}

struct Builder<'a, B: Backend> {
    backend: &'a B,
    window: &'a B::Window,
    layout: &'a mut Layout<B>,
    text_bindings: Vec<TextBinding>,
}

impl<B: Backend> Builder<'_, B> {
    fn build(&mut self, view: View) -> NodeId {
        match view {
            View::Label(label) => {
                let widget = self
                    .backend
                    .create_label(self.window, label.text.static_value());
                self.add_widget(widget, label.text)
            }
            View::Button(button) => {
                let widget = self.backend.create_button(
                    self.window,
                    button.title.static_value(),
                    button.on_click,
                );
                self.add_widget(widget, button.title)
            }
            View::Stack(mut stack) => {
                let children: Vec<NodeId> = std::mem::take(&mut stack.children)
                    .into_iter()
                    .map(|child| self.build(child))
                    .collect();
                self.layout.add_stack(&stack, &children)
            }
        }
    }

    fn add_widget(&mut self, widget: B::Widget, text: Text) -> NodeId {
        let node = self.layout.add_widget(widget);
        if let Text::Dynamic(text) = text {
            self.text_bindings.push(TextBinding { node, text });
        }
        node
    }
}

#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::backend::{ClickHandler, ResizeHandler};
    use crate::geometry::{Frame, Point, Size};
    use crate::reactive::Signal;
    use crate::view::{Align, Justify, button, column, label};

    const CHAR_WIDTH: f32 = 10.0;
    const LINE_HEIGHT: f32 = 20.0;

    #[derive(Default)]
    struct FakeWidget {
        text: String,
        frame: Option<Frame>,
        on_click: Option<ClickHandler>,
    }

    #[derive(Default)]
    struct FakeBackend {
        size: Cell<Size>,
        on_resize: RefCell<Option<ResizeHandler>>,
        widgets: RefCell<Vec<FakeWidget>>,
    }

    impl FakeBackend {
        fn push(&self, widget: FakeWidget) -> usize {
            let mut widgets = self.widgets.borrow_mut();
            widgets.push(widget);
            widgets.len() - 1
        }

        fn frame(&self, widget: usize) -> Frame {
            self.widgets.borrow()[widget]
                .frame
                .expect("widget was laid out")
        }

        fn text(&self, widget: usize) -> String {
            self.widgets.borrow()[widget].text.clone()
        }

        fn click(&self, widget: usize) {
            let on_click = self.widgets.borrow_mut()[widget]
                .on_click
                .take()
                .expect("widget is a button");
            on_click();
            self.widgets.borrow_mut()[widget].on_click = Some(on_click);
        }

        fn resize(&self, size: Size) {
            let on_resize = self.on_resize.borrow();
            on_resize.as_ref().expect("window is open")(size);
        }
    }

    impl Backend for FakeBackend {
        type Window = ();
        type Widget = usize;

        fn open_window(&self, _title: &str, size: Size, on_resize: ResizeHandler) {
            self.size.set(size);
            self.on_resize.replace(Some(on_resize));
        }

        fn content_size(&self, (): &()) -> Size {
            self.size.get()
        }

        fn show_window(&self, (): &()) {}

        fn create_label(&self, (): &(), text: &str) -> usize {
            self.push(FakeWidget {
                text: text.to_owned(),
                ..FakeWidget::default()
            })
        }

        fn create_button(&self, (): &(), title: &str, on_click: ClickHandler) -> usize {
            self.push(FakeWidget {
                text: title.to_owned(),
                on_click: Some(on_click),
                ..FakeWidget::default()
            })
        }

        fn set_text(&self, widget: &usize, text: &str) {
            text.clone_into(&mut self.widgets.borrow_mut()[*widget].text);
        }

        fn measure(&self, widget: &usize) -> Size {
            let chars = u8::try_from(self.widgets.borrow()[*widget].text.len())
                .expect("test strings are short");
            Size::new(f32::from(chars) * CHAR_WIDTH, LINE_HEIGHT)
        }

        fn set_frame(&self, widget: &usize, frame: Frame) {
            self.widgets.borrow_mut()[*widget].frame = Some(frame);
        }

        fn run(&self) {}
    }

    fn frame(x: f32, y: f32, width: f32, height: f32) -> Frame {
        Frame {
            origin: Point { x, y },
            size: Size::new(width, height),
        }
    }

    #[test]
    fn centers_children_with_spacing() {
        let backend = Rc::new(FakeBackend::default());
        let content = column()
            .spacing(10.0)
            .align(Align::Center)
            .justify(Justify::Center)
            .child(label("abc"))
            .child(label("hello"));
        let _window = mount(&backend, Window::new("test", content).size(200.0, 100.0));

        assert_eq!(backend.frame(0), frame(85.0, 25.0, 30.0, 20.0));
        assert_eq!(backend.frame(1), frame(75.0, 55.0, 50.0, 20.0));
    }

    #[test]
    fn padding_offsets_children() {
        let backend = Rc::new(FakeBackend::default());
        let content = column().padding(8.0).child(label("ab"));
        let _window = mount(&backend, Window::new("test", content).size(100.0, 100.0));

        assert_eq!(backend.frame(0), frame(8.0, 8.0, 84.0, 20.0));
    }

    #[test]
    fn button_click_updates_bound_label_and_its_frame() {
        let backend = Rc::new(FakeBackend::default());
        let count = Signal::new(9);
        let content = column()
            .align(Align::Start)
            .child(label({
                let count = count.clone();
                move || format!("n={}", count.get())
            }))
            .child(button("+", move || count.update(|n| *n += 1)));
        let _window = mount(&backend, Window::new("test", content).size(200.0, 100.0));
        assert_eq!(backend.text(0), "n=9");
        assert_eq!(backend.frame(0).size.width, 30.0);

        backend.click(1);

        assert_eq!(backend.text(0), "n=10");
        assert_eq!(backend.frame(0).size.width, 40.0);
    }

    #[test]
    fn resize_relays_out() {
        let backend = Rc::new(FakeBackend::default());
        let content = column().justify(Justify::End).child(label("a"));
        let _window = mount(&backend, Window::new("test", content).size(100.0, 100.0));
        assert_eq!(backend.frame(0).origin.y, 80.0);

        backend.resize(Size::new(100.0, 300.0));

        assert_eq!(backend.frame(0).origin.y, 280.0);
    }
}
