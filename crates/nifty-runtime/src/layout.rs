//! Flexbox layout over native widgets: Taffy computes frames, the caller applies them.

use std::collections::HashMap;

use taffy::{
    AlignItems, AlignSelf, AvailableSpace, Display, FlexDirection, JustifyContent, LayoutInput,
    LayoutOutput, NodeId, Rect, Style, TaffyTree, TraversePartialTree, compute_leaf_layout,
    style_helpers::length,
};

use crate::geometry::{Frame, Point, Size};
use crate::style::{Align, Axis, Justify, StackStyle};

const LIVE_NODE: &str = "layout nodes are only used until they are removed";

type TaffySize = taffy::Size<f32>;

/// One tree for every window: each window's content hangs off its own root node.
/// A node's context is its natural size, present only on widget leaves.
pub(crate) struct Layout<W> {
    tree: TaffyTree<TaffySize>,
    widgets: HashMap<NodeId, W>,
}

impl<W> Layout<W> {
    pub(crate) fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            widgets: HashMap::new(),
        }
    }

    pub(crate) fn add_root(&mut self) -> NodeId {
        self.tree.new_leaf(root_style(Size::ZERO)).expect(LIVE_NODE)
    }

    pub(crate) fn add_widget(&mut self, widget: W, natural: Size) -> NodeId {
        self.add_leaf(widget, natural, Style::default())
    }

    /// A divider stretches across its parent's cross axis and stays one point thick.
    pub(crate) fn add_divider(&mut self, widget: W, natural: Size) -> NodeId {
        let style = Style {
            align_self: Some(AlignSelf::STRETCH),
            flex_shrink: 0.0,
            ..Style::default()
        };
        self.add_leaf(widget, natural, style)
    }

    /// Empty space that takes whatever room its stack has left.
    pub(crate) fn add_spacer(&mut self) -> NodeId {
        let style = Style {
            flex_grow: 1.0,
            ..Style::default()
        };
        self.tree.new_leaf(style).expect(LIVE_NODE)
    }

    pub(crate) fn add_stack(&mut self, style: &StackStyle) -> NodeId {
        self.tree.new_leaf(stack_style(style)).expect(LIVE_NODE)
    }

    fn add_leaf(&mut self, widget: W, natural: Size, style: Style) -> NodeId {
        let node = self
            .tree
            .new_leaf_with_context(style, to_taffy(natural))
            .expect(LIVE_NODE);
        self.widgets.insert(node, widget);
        node
    }

    pub(crate) fn insert(&mut self, parent: NodeId, index: usize, child: NodeId) {
        self.tree
            .insert_child_at_index(parent, index, child)
            .expect(LIVE_NODE);
    }

    /// Makes `content` fill the window whose root is `root`.
    pub(crate) fn set_content(&mut self, root: NodeId, content: NodeId) {
        let mut style = self.tree.style(content).expect(LIVE_NODE).clone();
        style.flex_grow = 1.0;
        self.tree.set_style(content, style).expect(LIVE_NODE);
        self.tree.set_children(root, &[content]).expect(LIVE_NODE);
    }

    /// Removes `node` and everything under it, handing back their widgets.
    pub(crate) fn remove(&mut self, node: NodeId) -> Vec<W> {
        let mut removed = Vec::new();
        self.remove_into(node, &mut removed);
        removed
    }

    fn remove_into(&mut self, node: NodeId, removed: &mut Vec<W>) {
        let children: Vec<NodeId> = self.tree.child_ids(node).collect();
        for child in children {
            self.remove_into(child, removed);
        }
        self.tree.remove(node).expect(LIVE_NODE);
        removed.extend(self.widgets.remove(&node));
    }

    pub(crate) fn widget(&self, node: NodeId) -> Option<&W> {
        self.widgets.get(&node)
    }

    /// Records a widget's new natural size; returns whether layout is now out of date.
    pub(crate) fn set_natural(&mut self, node: NodeId, natural: Size) -> bool {
        let natural = to_taffy(natural);
        let current = self
            .tree
            .get_node_context_mut(node)
            .expect("only widget nodes have a natural size");
        if *current == natural {
            return false;
        }
        *current = natural;
        self.tree.mark_dirty(node).expect(LIVE_NODE);
        true
    }

    /// Lays out the window rooted at `root` and reports each widget's frame.
    pub(crate) fn layout_window(
        &mut self,
        root: NodeId,
        viewport: Size,
        mut apply: impl FnMut(&W, Frame),
    ) {
        self.tree
            .set_style(root, root_style(viewport))
            .expect(LIVE_NODE);
        let available = taffy::Size {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        };
        self.tree
            .compute_layout_with_measure(root, available, measure_leaf)
            .expect(LIVE_NODE);
        self.place(root, Point::default(), &mut apply);
    }

    /// Native widgets are flat children of the window, so frames are made window-relative here.
    fn place(&self, node: NodeId, parent_origin: Point, apply: &mut impl FnMut(&W, Frame)) {
        let layout = self.tree.layout(node).expect(LIVE_NODE);
        let origin = Point {
            x: parent_origin.x + layout.location.x,
            y: parent_origin.y + layout.location.y,
        };
        if let Some(widget) = self.widgets.get(&node) {
            let size = Size::new(layout.size.width, layout.size.height);
            apply(widget, Frame { origin, size });
        }
        for child in self.tree.child_ids(node) {
            self.place(child, origin, apply);
        }
    }
}

fn measure_leaf(
    inputs: LayoutInput,
    _node: NodeId,
    natural: Option<&mut TaffySize>,
    style: &Style,
) -> LayoutOutput {
    let natural = natural.map_or(TaffySize::ZERO, |natural| *natural);
    compute_leaf_layout(
        inputs,
        style,
        |_, _| 0.0,
        |known, _available| known.unwrap_or(natural),
    )
}

fn root_style(viewport: Size) -> Style {
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

fn stack_style(stack: &StackStyle) -> Style {
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
