use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::Button,
    slider::{Slider, SliderEvent, SliderState},
};
use gpui_kit::{Context, Entity, IntoElement, Subscription, div, prelude::*, px};
use serde_json::json;

use crate::{Snapshot, Workspace, format_time};

pub struct VideoTimeline {
    pub slider: Entity<SliderState>,
    pub dragging: bool,
    pub seeking: bool,
    pub pending_frame: Option<i64>,
    _events: Subscription,
}

impl VideoTimeline {
    pub fn new(cx: &mut Context<Workspace>) -> Self {
        let slider = cx.new(|_| SliderState::new().min(0.).max(1.).step(0.000001));
        let events = cx.subscribe(&slider, |view, _, event: &SliderEvent, cx| {
            let value = match event {
                SliderEvent::Change(value) => {
                    view.timeline.dragging = true;
                    value.start()
                }
                SliderEvent::Release(value) => {
                    view.timeline.dragging = false;
                    value.start()
                }
            };
            view.timeline.pending_frame =
                Some((value * (view.snapshot.frame_count - 1).max(0) as f32).round() as i64);
            view.flush_video_seek(cx);
        });
        Self {
            slider,
            dragging: false,
            seeking: false,
            pending_frame: None,
            _events: events,
        }
    }

    pub fn render(&self, snapshot: &Snapshot, cx: &mut Context<Workspace>) -> impl IntoElement {
        let mut controls = div().h_flex().flex_wrap().gap_1().items_center();
        for (id, label, command) in [
            (
                "previous-keyframe",
                "上一关键帧",
                "video/frame/prev/keyframe",
            ),
            ("previous-frame", "上一帧", "video/frame/prev"),
            ("next-frame", "下一帧", "video/frame/next"),
            ("next-keyframe", "下一关键帧", "video/frame/next/keyframe"),
            ("line-start-frame", "句首", "video/jump/start"),
            ("line-end-frame", "句尾", "video/jump/end"),
        ] {
            let enabled = snapshot.video;
            controls = controls.child(
                Button::new(id)
                    .label(label)
                    .small()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":command}), cx)
                    })),
            );
        }
        let active = snapshot
            .lines
            .iter()
            .find(|line| line.id == snapshot.active);
        div()
            .v_flex()
            .flex_shrink_0()
            .min_h(px(68.))
            .bg(Theme::global(cx).background)
            .px_3()
            .py_2()
            .gap_2()
            .border_t_1()
            .border_color(Theme::global(cx).border)
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_3()
                    .child(div().text_xs().child(format!(
                        "{} / {} · 帧 {} / {} · {:.2}×",
                        format_time(snapshot.video_time),
                        format_time(snapshot.video_duration),
                        snapshot.frame,
                        (snapshot.frame_count - 1).max(0),
                        snapshot.playback_rate
                    )))
                    .child(Slider::new(&self.slider).disabled(!snapshot.video).flex_1()),
            )
            .child(
                controls.child(
                    div().text_xs().child(
                        active
                            .map(|line| format!("当前句帧 {}–{}", line.start_frame, line.end_frame))
                            .unwrap_or_default(),
                    ),
                ),
            )
    }
}
