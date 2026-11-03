// Added in the new-horizon-code-repo/gpui fork; not part of upstream GPUI.

//! A record of what one frame painted, for design-review and testing tools.
//!
//! Call [`Window::request_ui_snapshot`]; the next frame paints every view
//! without cache reuse and records each element and text run it paints.
//! [`Window::take_ui_snapshot`] then returns the result.

use crate::{
    Bounds, ContentMask, Hsla, Pixels, SharedString, Size, Style, StyleRefinement, TextStyle,
    Window,
};
use std::panic::Location;
use std::time::Duration;

/// What one frame painted, in paint order (parents before children).
#[derive(Clone, Debug, Default)]
pub struct UiSnapshot {
    /// Painted nodes; `UiNode::parent` indexes into this list.
    pub nodes: Vec<UiNode>,
    /// Window viewport size in logical pixels.
    pub viewport: Size<Pixels>,
    /// Pixels per logical pixel.
    pub scale_factor: f32,
    /// Size of one rem, for resolving rem lengths in recorded styles.
    pub rem_size: Pixels,
}

/// One painted element or piece of text.
#[derive(Clone, Debug)]
pub struct UiNode {
    /// Index of the enclosing element, if any.
    pub parent: Option<usize>,
    /// Where the element was constructed (debug builds or the `inspector` feature).
    pub source: Option<&'static Location<'static>>,
    /// The element's global id path, when it or an ancestor has an id.
    pub id: Option<SharedString>,
    /// Window bounds in logical pixels.
    pub bounds: Bounds<Pixels>,
    /// The clip inherited from ancestors.
    pub clip: Bounds<Pixels>,
    /// Element or text details.
    pub kind: UiNodeKind,
}

/// The node's payload.
#[derive(Clone, Debug)]
pub enum UiNodeKind {
    /// A div, img, svg or other element built on `Interactivity`.
    Element {
        /// The resolved style for this frame, hover and active states included.
        style: Box<Style>,
        /// Size of the children, larger than `bounds` when the content overflows.
        content_size: Size<Pixels>,
        /// Styles the element switches to, whether or not they apply right now.
        states: UiStates,
        /// How the element takes input.
        interaction: UiInteraction,
    },
    /// A `with_animation` wrapper; the animated element is its child.
    Animation {
        /// Length of one run.
        duration: Duration,
        /// Whether it runs once rather than repeating.
        oneshot: bool,
        /// The easing curve sampled at t = 0, 0.1, ..., 1.
        easing: Vec<f32>,
    },
    /// Shaped text.
    Text {
        /// The text the element was given.
        text: SharedString,
        /// The text as painted, after truncation; lines joined with `\n`.
        shown: SharedString,
        /// The inherited text style the text was shaped with.
        style: Box<TextStyle>,
        /// Font size in logical pixels.
        font_size: Pixels,
        /// Line height in logical pixels.
        line_height: Pixels,
        /// Size of the shaped text.
        size: Size<Pixels>,
        /// Per-run colors, as `(byte length, color, background)`.
        runs: Vec<(u32, Hsla, Option<Hsla>)>,
    },
}

/// The state styles an element declares (`.hover(..)`, `.active(..)`, `.focus(..)`).
#[derive(Clone, Debug, Default)]
pub struct UiStates {
    /// `.hover(..)`.
    pub hover: Option<Box<StyleRefinement>>,
    /// `.active(..)`.
    pub active: Option<Box<StyleRefinement>>,
    /// `.focus(..)`.
    pub focus: Option<Box<StyleRefinement>>,
    /// `.focus_visible(..)`.
    pub focus_visible: Option<Box<StyleRefinement>>,
}

/// Input facts about an element.
#[derive(Clone, Debug, Default)]
pub struct UiInteraction {
    /// It has click or mouse-down handlers.
    pub clickable: bool,
    /// It tracks a focus handle.
    pub focusable: bool,
    /// Its focus handle is in the Tab order (`FocusHandle::tab_stop`).
    pub tab_stop: bool,
    /// It holds focus in this frame.
    pub focused: bool,
    /// The pointer is over it in this frame.
    pub hovered: bool,
    /// It runs app code on hover (`.on_hover(..)`), or joins a hover group.
    pub hover_listener: bool,
    /// It shows a tooltip.
    pub tooltip: bool,
    /// Its explicit tab index, if any.
    pub tab_index: Option<isize>,
    /// It sets a key context for its actions.
    pub key_context: bool,
    /// Number of action handlers it registers.
    pub actions: usize,
    /// Its accessible label, if set.
    pub label: Option<SharedString>,
}

#[derive(Default)]
pub(crate) struct UiSnapshotState {
    requested: bool,
    recording: Option<UiSnapshot>,
    stack: Vec<usize>,
    done: Option<UiSnapshot>,
}

impl Window {
    /// Records the next frame into a [`UiSnapshot`] and schedules that frame.
    pub fn request_ui_snapshot(&mut self) {
        self.ui_snapshot.requested = true;
        self.refresh();
    }

    /// Returns the last recorded snapshot, once its frame has been drawn.
    pub fn take_ui_snapshot(&mut self) -> Option<UiSnapshot> {
        self.ui_snapshot.done.take()
    }

    /// Whether the current frame is being recorded.
    pub fn is_recording_ui_snapshot(&self) -> bool {
        self.ui_snapshot.recording.is_some()
    }

    pub(crate) fn begin_ui_snapshot(&mut self) {
        if std::mem::take(&mut self.ui_snapshot.requested) {
            self.ui_snapshot.stack.clear();
            self.ui_snapshot.recording = Some(UiSnapshot {
                nodes: Vec::new(),
                viewport: self.viewport_size(),
                scale_factor: self.scale_factor(),
                rem_size: self.rem_size(),
            });
        }
    }

    pub(crate) fn end_ui_snapshot(&mut self) {
        if let Some(snapshot) = self.ui_snapshot.recording.take() {
            self.ui_snapshot.done = Some(snapshot);
        }
    }

    fn push_ui_node(
        &mut self,
        source: Option<&'static Location<'static>>,
        id: Option<SharedString>,
        bounds: Bounds<Pixels>,
        kind: UiNodeKind,
    ) -> Option<usize> {
        let ContentMask { bounds: clip } = self.content_mask();
        let parent = self.ui_snapshot.stack.last().copied();
        let recording = self.ui_snapshot.recording.as_mut()?;
        recording.nodes.push(UiNode {
            parent,
            source,
            id,
            bounds,
            clip,
            kind,
        });
        Some(recording.nodes.len() - 1)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn enter_ui_element(
        &mut self,
        source: Option<&'static Location<'static>>,
        id: Option<SharedString>,
        bounds: Bounds<Pixels>,
        style: &Style,
        content_size: Size<Pixels>,
        states: UiStates,
        interaction: UiInteraction,
    ) -> bool {
        let kind = UiNodeKind::Element {
            style: Box::new(style.clone()),
            content_size,
            states,
            interaction,
        };
        self.enter_ui_node(source, id, bounds, kind)
    }

    pub(crate) fn enter_ui_animation(
        &mut self,
        bounds: Bounds<Pixels>,
        duration: Duration,
        oneshot: bool,
        easing: &dyn Fn(f32) -> f32,
    ) -> bool {
        let easing = (0..=10).map(|i| easing(i as f32 / 10.0)).collect();
        let kind = UiNodeKind::Animation {
            duration,
            oneshot,
            easing,
        };
        self.enter_ui_node(None, None, bounds, kind)
    }

    fn enter_ui_node(
        &mut self,
        source: Option<&'static Location<'static>>,
        id: Option<SharedString>,
        bounds: Bounds<Pixels>,
        kind: UiNodeKind,
    ) -> bool {
        match self.push_ui_node(source, id, bounds, kind) {
            Some(index) => {
                self.ui_snapshot.stack.push(index);
                true
            }
            None => false,
        }
    }

    pub(crate) fn exit_ui_element(&mut self) {
        self.ui_snapshot.stack.pop();
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn record_ui_text(
        &mut self,
        text: SharedString,
        shown: SharedString,
        bounds: Bounds<Pixels>,
        font_size: Pixels,
        line_height: Pixels,
        size: Size<Pixels>,
        runs: Vec<(u32, Hsla, Option<Hsla>)>,
    ) {
        let kind = UiNodeKind::Text {
            text,
            shown,
            style: Box::new(self.text_style()),
            font_size,
            line_height,
            size,
            runs,
        };
        self.push_ui_node(None, None, bounds, kind);
    }
}
