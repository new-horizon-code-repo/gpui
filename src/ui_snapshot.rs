// Added in the new-horizon-code-repo/gpui fork; not part of upstream GPUI.

//! A record of what one frame painted, for design-review and testing tools.
//!
//! Call [`Window::request_ui_snapshot`]; the next frame paints every view
//! without cache reuse and records each element and text run it paints.
//! [`Window::take_ui_snapshot`] then returns the result.

use crate::{Bounds, ContentMask, Hsla, Pixels, SharedString, Size, Style, TextStyle, Window};
use std::panic::Location;

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

    pub(crate) fn enter_ui_element(
        &mut self,
        source: Option<&'static Location<'static>>,
        id: Option<SharedString>,
        bounds: Bounds<Pixels>,
        style: &Style,
        content_size: Size<Pixels>,
    ) -> bool {
        let kind = UiNodeKind::Element {
            style: Box::new(style.clone()),
            content_size,
        };
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
