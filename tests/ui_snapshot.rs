// Added in the new-horizon-code-repo/gpui fork; not part of upstream GPUI.

use gpui::prelude::*;
use gpui::{
    AnimationExt as _, Context, IntoElement, ParentElement, Render, Styled, TestAppContext,
    UiNodeKind, Window, div, px, rgb,
};

struct Card;

impl Render for Card {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().bg(rgb(0x111214)).child(
            div()
                .w(px(120.))
                .h(px(20.))
                .text_size(px(10.))
                .text_color(rgb(0x7c7c86))
                .child("A long caption"),
        )
    }
}

#[gpui::test]
fn snapshot_records_elements_and_text(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Card);
    let snapshot = cx
        .update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            assert!(window.take_ui_snapshot().is_none());
            window.request_ui_snapshot();
            window.draw(cx).clear(cx);
            window.take_ui_snapshot()
        })
        .unwrap()
        .expect("a snapshot after the requested frame");

    let caption = snapshot
        .nodes
        .iter()
        .position(|node| {
            matches!(&node.kind, UiNodeKind::Element { .. }) && node.bounds.size.width == px(120.)
        })
        .expect("the caption box");
    assert!(snapshot.nodes[caption].source.is_some());
    assert_eq!(snapshot.nodes[caption].parent, Some(0));

    let (parent, text, font_size, runs) = snapshot
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            UiNodeKind::Text {
                text,
                font_size,
                runs,
                ..
            } => Some((node.parent, text.clone(), *font_size, runs.clone())),
            _ => None,
        })
        .expect("the caption text");
    assert_eq!(parent, Some(caption));
    assert_eq!(text.as_ref(), "A long caption");
    assert_eq!(font_size, px(10.));
    assert_eq!(runs[0].1, rgb(0x7c7c86).into());
}

#[gpui::test]
fn frames_without_a_request_record_nothing(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Card);
    let snapshot = cx
        .update_window(window.into(), |_, window, cx| {
            window.draw(cx).clear(cx);
            window.draw(cx).clear(cx);
            (window.is_recording_ui_snapshot(), window.take_ui_snapshot())
        })
        .unwrap();
    assert!(!snapshot.0);
    assert!(snapshot.1.is_none());
}

struct Controls;

impl Render for Controls {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(
                div()
                    .id("icon-button")
                    .size(px(24.))
                    .hover(|s| s.bg(rgb(0x333333)))
                    .on_click(|_, _, _| {})
                    .child(div().size(px(16.))),
            )
            .child(div().id("dot").size(px(8.)).with_animation(
                "pulse",
                gpui::Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                |el, t| el.opacity(t),
            ))
    }
}

#[gpui::test]
fn snapshot_records_state_styles_input_and_animations(cx: &mut TestAppContext) {
    let window = cx.add_window(|_, _| Controls);
    let snapshot = cx
        .update_window(window.into(), |_, window, cx| {
            window.request_ui_snapshot();
            window.draw(cx).clear(cx);
            window.take_ui_snapshot()
        })
        .unwrap()
        .unwrap();

    let button = snapshot
        .nodes
        .iter()
        .find_map(|node| match &node.kind {
            UiNodeKind::Element {
                states,
                interaction,
                ..
            } if interaction.clickable => Some((states.clone(), interaction.clone())),
            _ => None,
        })
        .expect("the clickable button");
    assert!(button.0.hover.is_some());
    assert!(!button.1.tooltip && !button.1.focusable);

    let (index, duration, oneshot, easing) = snapshot
        .nodes
        .iter()
        .enumerate()
        .find_map(|(i, node)| match &node.kind {
            UiNodeKind::Animation {
                duration,
                oneshot,
                easing,
            } => Some((i, *duration, *oneshot, easing.clone())),
            _ => None,
        })
        .expect("the animation wrapper");
    assert_eq!(duration, std::time::Duration::from_millis(1200));
    assert!(!oneshot);
    assert_eq!(easing.len(), 11);
    assert!(snapshot.nodes.iter().any(|node| node.parent == Some(index)));
}
