//! Flexbox layout over native widgets: Taffy computes frames, the caller applies them.

use std::collections::{HashMap, HashSet};

use taffy::{
    AlignItems, AlignSelf, AvailableSpace, Dimension, Display, FlexDirection, JustifyContent,
    LayoutInput, LayoutOutput, LengthPercentage, LengthPercentageAuto, NodeId, Overflow, Rect,
    Style, TaffyTree, TraversePartialTree, compute_leaf_layout, style_helpers::length,
};

use crate::geometry::{Frame, Point, Size};
use crate::style::{Align, Axis, Justify, StackStyle};

const LIVE_NODE: &str = "layout nodes are only used until they are removed";

type TaffySize = taffy::Size<f32>;

/// Within this distance of the top of the content area, a surface counts as touching it.
const TOP_EDGE_TOLERANCE: f32 = 0.5;

/// Where layout puts a widget.
pub(crate) enum Placed<'a, W> {
    /// The widget's frame, relative to its parent's content.
    Frame(&'a W, Frame),
    /// The size of the content a scroll area scrolls; its children are placed in it.
    ScrollContent(&'a W, Size),
}

/// One tree for every window: each window's content hangs off its own root node.
/// A node's context is its natural size, present only on widget leaves.
pub(crate) struct Layout<W> {
    tree: TaffyTree<TaffySize>,
    widgets: HashMap<NodeId, W>,
    /// Stacks whose widget is the surface drawn behind their children.
    surfaces: HashSet<NodeId>,
    /// Stacks that scroll, and the scroll area that shows each one's children.
    scrolls: HashMap<NodeId, W>,
    /// Widgets that cover their stack's content, outside its layout.
    overlays: HashSet<NodeId>,
}

impl<W> Layout<W> {
    pub(crate) fn new() -> Self {
        Self {
            tree: TaffyTree::new(),
            widgets: HashMap::new(),
            surfaces: HashSet::new(),
            scrolls: HashMap::new(),
            overlays: HashSet::new(),
        }
    }

    pub(crate) fn add_root(&mut self) -> NodeId {
        self.tree
            .new_leaf(root_style(Some(Size::ZERO), 0.0))
            .expect(LIVE_NODE)
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

    /// A widget that covers its whole stack, padding included, and takes no part in how
    /// the stack lays out its other children: Taffy skips it, and `place` gives it the
    /// stack's content frame.
    pub(crate) fn add_overlay(&mut self, widget: W, natural: Size) -> NodeId {
        let style = Style {
            display: Display::None,
            ..Style::default()
        };
        let node = self.add_leaf(widget, natural, style);
        self.overlays.insert(node);
        node
    }

    /// Empty space that takes whatever room its stack has left.
    pub(crate) fn add_spacer(&mut self) -> NodeId {
        let style = Style {
            flex_grow: 1.0,
            ..Style::default()
        };
        self.tree.new_leaf(style).expect(LIVE_NODE)
    }

    /// A stack, with the surface widget drawn behind its children if it has one. Stacks
    /// size from their children, so a surface needs no natural size.
    pub(crate) fn add_stack(&mut self, style: &StackStyle, surface: Option<W>) -> NodeId {
        let node = self.tree.new_leaf(stack_style(style)).expect(LIVE_NODE);
        if let Some(surface) = surface {
            self.widgets.insert(node, surface);
            self.surfaces.insert(node);
        }
        node
    }

    /// A stack shown in `scroll`, whose children scroll up and down when they are taller
    /// than it, with its scroll bar taking `bar_width` beside them. It takes the space its
    /// parent has left, and adds nothing to its parent's size, so a parent sized by its
    /// content leaves it none.
    pub(crate) fn add_scroll(
        &mut self,
        style: &StackStyle,
        surface: Option<W>,
        scroll: W,
        bar_width: f32,
    ) -> NodeId {
        let node = self.add_stack(style, surface);
        let style = Style {
            overflow: taffy::Point {
                x: Overflow::Visible,
                y: Overflow::Scroll,
            },
            scrollbar_width: bar_width,
            flex_grow: 1.0,
            flex_basis: Dimension::length(0.0),
            min_size: taffy::Size {
                width: LengthPercentageAuto::auto(),
                height: LengthPercentageAuto::length(0.0),
            },
            ..stack_style(style)
        };
        self.tree.set_style(node, style).expect(LIVE_NODE);
        self.scrolls.insert(node, scroll);
        node
    }

    /// Leaves `width` beside the content of every scroll area in the tree under `root`, for
    /// its scroll bar.
    pub(crate) fn set_scroll_bar_width(&mut self, root: NodeId, width: f32) {
        if self.scrolls.contains_key(&root) {
            let mut style = self.tree.style(root).expect(LIVE_NODE).clone();
            style.scrollbar_width = width;
            self.tree.set_style(root, style).expect(LIVE_NODE);
        }
        let children: Vec<NodeId> = self.tree.child_ids(root).collect();
        for child in children {
            self.set_scroll_bar_width(child, width);
        }
    }

    /// The scroll area whose content `node` is in, counting a scroll's own children as in
    /// it, or `None` when it is straight in the window.
    pub(crate) fn scroll_around(&self, node: NodeId) -> Option<&W> {
        let mut current = Some(node);
        while let Some(node) = current {
            if let Some(scroll) = self.scrolls.get(&node) {
                return Some(scroll);
            }
            current = self.tree.parent(node);
        }
        None
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

    /// Makes `content` fill the window whose root is `root`, returning the content it
    /// replaces, which is now detached and still holds its widgets.
    pub(crate) fn set_content(&mut self, root: NodeId, content: NodeId) -> Option<NodeId> {
        let previous = self.tree.child_ids(root).next();
        let mut style = self.tree.style(content).expect(LIVE_NODE).clone();
        style.flex_grow = 1.0;
        self.tree.set_style(content, style).expect(LIVE_NODE);
        self.tree.set_children(root, &[content]).expect(LIVE_NODE);
        previous.filter(|&previous| previous != content)
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
        self.surfaces.remove(&node);
        self.overlays.remove(&node);
        removed.extend(self.widgets.remove(&node));
        removed.extend(self.scrolls.remove(&node));
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

    /// The narrowest the window rooted at `root` can be without clipping its content.
    pub(crate) fn min_width(&mut self, root: NodeId, title_bar_inset: f32) -> f32 {
        self.tree
            .set_style(root, root_style(None, title_bar_inset))
            .expect(LIVE_NODE);
        let min_content = taffy::Size {
            width: AvailableSpace::MinContent,
            height: AvailableSpace::MinContent,
        };
        self.tree
            .compute_layout_with_measure(root, min_content, measure_leaf)
            .expect(LIVE_NODE);
        self.tree.layout(root).expect(LIVE_NODE).size.width
    }

    /// Lays out the window rooted at `root` and reports where each widget goes. Content
    /// starts below the title bar; a surface whose top touches it reaches up under the
    /// title bar, while its children stay below.
    pub(crate) fn layout_window(
        &mut self,
        root: NodeId,
        viewport: Size,
        title_bar_inset: f32,
        mut apply: impl FnMut(Placed<'_, W>),
    ) {
        self.tree
            .set_style(root, root_style(Some(viewport), title_bar_inset))
            .expect(LIVE_NODE);
        let available = taffy::Size {
            width: AvailableSpace::Definite(viewport.width),
            height: AvailableSpace::Definite(viewport.height),
        };
        self.tree
            .compute_layout_with_measure(root, available, measure_leaf)
            .expect(LIVE_NODE);
        self.place(root, Point::default(), Some(title_bar_inset), &mut apply);
    }

    /// Native widgets are flat children of the window or of a scroll area's content, so
    /// frames are made relative to those here. Inside a scroll area there is no title bar
    /// to reach under.
    fn place(
        &self,
        node: NodeId,
        parent_origin: Point,
        title_bar_inset: Option<f32>,
        apply: &mut impl FnMut(Placed<'_, W>),
    ) {
        let layout = self.tree.layout(node).expect(LIVE_NODE);
        let origin = Point {
            x: parent_origin.x + layout.location.x,
            y: parent_origin.y + layout.location.y,
        };
        let frame = Frame {
            origin,
            size: Size::new(layout.size.width, layout.size.height),
        };
        if let Some(widget) = self.widgets.get(&node) {
            let reaches_title_bar = self.surfaces.contains(&node)
                && title_bar_inset
                    .is_some_and(|inset| (origin.y - inset).abs() < TOP_EDGE_TOLERANCE);
            let frame = if reaches_title_bar {
                under_title_bar(frame)
            } else {
                frame
            };
            apply(Placed::Frame(widget, frame));
        }
        let (content, title_bar_inset) = match self.scrolls.get(&node) {
            Some(scroll) => {
                apply(Placed::Frame(scroll, frame));
                let size = Size::new(
                    layout.size.width - layout.scrollbar_size.width,
                    layout.size.height + layout.scroll_height(),
                );
                apply(Placed::ScrollContent(scroll, size));
                let content = Frame {
                    origin: Point::default(),
                    size,
                };
                (content, None)
            }
            None => (frame, title_bar_inset),
        };
        for child in self.tree.child_ids(node) {
            match self
                .widgets
                .get(&child)
                .filter(|_| self.overlays.contains(&child))
            {
                Some(overlay) => apply(Placed::Frame(overlay, content)),
                None => self.place(child, content.origin, title_bar_inset, apply),
            }
        }
    }
}

/// `frame`, stretched up to the top of the window.
fn under_title_bar(frame: Frame) -> Frame {
    Frame {
        origin: Point {
            x: frame.origin.x,
            y: 0.0,
        },
        size: Size::new(frame.size.width, frame.size.height + frame.origin.y),
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

/// The window's root fills `viewport`, or fits its content when there is none, and keeps
/// content out from under the title bar.
fn root_style(viewport: Option<Size>, title_bar_inset: f32) -> Style {
    Style {
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        size: viewport.map_or(taffy::Size::auto(), |viewport| taffy::Size {
            width: length(viewport.width),
            height: length(viewport.height),
        }),
        padding: Rect {
            top: LengthPercentage::length(title_bar_inset),
            ..Rect::zero()
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
        flex_grow: if stack.grow { 1.0 } else { 0.0 },
        size: taffy::Size {
            width: stack_width(stack),
            height: Dimension::auto(),
        },
        max_size: taffy::Size {
            width: stack
                .width_limit()
                .map_or(LengthPercentageAuto::auto(), LengthPercentageAuto::length),
            height: LengthPercentageAuto::auto(),
        },
        ..Style::default()
    }
}

/// A fixed width, or the full width on offer when there is a limit, so a limited stack
/// fills its parent up to the limit and can be centred in it; otherwise the content's width.
fn stack_width(stack: &StackStyle) -> Dimension {
    match (stack.fixed_width(), stack.width_limit()) {
        (Some(width), _) => Dimension::length(width),
        (None, Some(_)) => Dimension::percent(1.0),
        (None, None) => Dimension::auto(),
    }
}

fn to_taffy(size: Size) -> TaffySize {
    TaffySize {
        width: size.width,
        height: size.height,
    }
}
