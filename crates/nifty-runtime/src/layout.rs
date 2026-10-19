//! Flexbox layout over native widgets: Taffy computes frames, the backend applies them.

use std::rc::Rc;

use taffy::{
    AlignItems, AvailableSpace, Display, FlexDirection, JustifyContent, LayoutInput, LayoutOutput,
    NodeId, Rect, Style, TaffyTree, TraversePartialTree, compute_leaf_layout,
    style_helpers::length,
};

use crate::backend::Backend;
use crate::geometry::{Frame, Point, Size};
use crate::view::{Align, Axis, Justify, Stack};

const VALID_NODE: &str = "layout nodes are never removed, so their ids stay valid";

type TaffySize = taffy::Size<f32>;

struct Leaf<W> {
    widget: W,
    natural: TaffySize,
}

pub(crate) struct Layout<B: Backend> {
    backend: Rc<B>,
    tree: TaffyTree<Leaf<B::Widget>>,
    root: NodeId,
    viewport: Option<Size>,
}

impl<B: Backend> Layout<B> {
    pub(crate) fn new(backend: Rc<B>) -> Self {
        let mut tree = TaffyTree::new();
        let root = tree.new_leaf(window_style(Size::ZERO)).expect(VALID_NODE);
        Self {
            backend,
            tree,
            root,
            viewport: None,
        }
    }

    pub(crate) fn add_widget(&mut self, widget: B::Widget) -> NodeId {
        let natural = to_taffy(self.backend.measure(&widget));
        self.tree
            .new_leaf_with_context(Style::default(), Leaf { widget, natural })
            .expect(VALID_NODE)
    }

    pub(crate) fn add_stack(&mut self, stack: &Stack, children: &[NodeId]) -> NodeId {
        self.tree
            .new_with_children(stack_style(stack), children)
            .expect(VALID_NODE)
    }

    /// Makes `content` fill the window.
    pub(crate) fn set_content(&mut self, content: NodeId) {
        let mut style = self.tree.style(content).expect(VALID_NODE).clone();
        style.flex_grow = 1.0;
        self.tree.set_style(content, style).expect(VALID_NODE);
        self.tree.add_child(self.root, content).expect(VALID_NODE);
    }

    pub(crate) fn resize(&mut self, viewport: Size) {
        self.viewport = Some(viewport);
        self.tree
            .set_style(self.root, window_style(viewport))
            .expect(VALID_NODE);
        self.relayout();
    }

    pub(crate) fn set_text(&mut self, node: NodeId, text: &str) {
        let leaf = self
            .tree
            .get_node_context_mut(node)
            .expect("text bindings only target widget nodes");
        self.backend.set_text(&leaf.widget, text);

        let natural = to_taffy(self.backend.measure(&leaf.widget));
        if natural == leaf.natural {
            return;
        }
        leaf.natural = natural;
        self.tree.mark_dirty(node).expect(VALID_NODE);
        self.relayout();
    }

    /// Does nothing until the window reports its first size.
    fn relayout(&mut self) {
        let Some(viewport) = self.viewport else {
            return;
        };
        let available = taffy::Size {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        };
        self.tree
            .compute_layout_with_measure(self.root, available, measure_leaf)
            .expect(VALID_NODE);
        self.place(self.root, Point::default());
    }

    /// Native widgets are flat children of the window, so frames are made window-relative here.
    fn place(&self, node: NodeId, parent_origin: Point) {
        let layout = self.tree.layout(node).expect(VALID_NODE);
        let origin = Point {
            x: parent_origin.x + layout.location.x,
            y: parent_origin.y + layout.location.y,
        };
        if let Some(leaf) = self.tree.get_node_context(node) {
            let size = Size::new(layout.size.width, layout.size.height);
            self.backend.set_frame(&leaf.widget, Frame { origin, size });
        }
        for child in self.tree.child_ids(node) {
            self.place(child, origin);
        }
    }
}

fn measure_leaf<W>(
    inputs: LayoutInput,
    _node: NodeId,
    leaf: Option<&mut Leaf<W>>,
    style: &Style,
) -> LayoutOutput {
    let natural = leaf.map_or(TaffySize::ZERO, |leaf| leaf.natural);
    compute_leaf_layout(
        inputs,
        style,
        |_, _| 0.0,
        |known, _available| known.unwrap_or(natural),
    )
}

fn window_style(viewport: Size) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: taffy::Size {
            width: length(viewport.width),
            height: length(viewport.height),
        },
        ..Style::default()
    }
}

fn stack_style(stack: &Stack) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: match stack.axis {
            Axis::Horizontal => FlexDirection::Row,
            Axis::Vertical => FlexDirection::Column,
        },
        gap: taffy::Size::length(stack.spacing),
        padding: Rect::length(stack.padding),
        align_items: match stack.align {
            Align::Stretch => AlignItems::STRETCH,
            Align::Start => AlignItems::START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::END,
        },
        justify_content: match stack.justify {
            Justify::Start => JustifyContent::START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        },
        ..Style::default()
    }
}

fn to_taffy(size: Size) -> TaffySize {
    TaffySize {
        width: size.width,
        height: size.height,
    }
}
