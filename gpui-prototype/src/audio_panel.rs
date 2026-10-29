use std::{cell::RefCell, rc::Rc, sync::Arc};

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::{
    Icon, StyledExt, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::{
    Bounds, Context, Image, IntoElement, MouseButton, ObjectFit, Pixels, Styled, StyledImage,
    canvas, div, img, prelude::*, px, rgb,
};
use serde::Deserialize;
use serde_json::json;

use crate::{Workspace, format_time};

#[derive(Clone, Debug, Deserialize)]
pub struct Syllable {
    pub text: String,
    pub start: i64,
    pub duration: i64,
    pub tag: String,
    pub characters: usize,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct AudioState {
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub epoch: u64,
    #[serde(default)]
    pub render_revision: u64,
    #[serde(default)]
    pub snap_distance: i64,
    #[serde(default)]
    pub drag_sensitivity: i64,
    #[serde(default)]
    pub duration: i64,
    #[serde(default)]
    pub decoded: i64,
    #[serde(default)]
    pub playing: bool,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub start: i64,
    #[serde(default)]
    pub end: i64,
    #[serde(default)]
    #[allow(dead_code)] // Rendered by the full audio page only.
    pub rate: f64,
    #[serde(default)]
    pub volume: i64,
    #[serde(default)]
    pub autocommit: bool,
    #[serde(default)]
    pub autonext: bool,
    #[serde(default)]
    pub autoscroll: bool,
    #[serde(default)]
    pub snap: bool,
    #[serde(default)]
    pub spectrum: bool,
    #[serde(default)]
    pub karaoke: bool,
    #[serde(default)]
    pub pending_splits: bool,
    #[serde(default)]
    pub syllables: Vec<Syllable>,
    #[serde(default)]
    pub markers: Vec<i64>,
}

#[derive(Deserialize)]
pub struct AudioImage {
    #[serde(flatten)]
    pub state: AudioState,
    pub image: Option<String>,
    #[serde(default)]
    pub view_start: f64,
}

pub struct AudioPanel {
    pub mode: &'static str,
    pub view_start: i64,
    pub span: i64,
    pub gain: i64,
    pub image: Option<Arc<Image>>,
    pub loading: bool,
    pub requested: Option<(u64, i64, i64, &'static str, i64, i64, u64)>,
    pub bounds: Rc<RefCell<Bounds<Pixels>>>,
    pub gesture: Option<u64>,
    pub dragging: bool,
    pub pending_ms: Option<i64>,
    pub drag_loading: bool,
    pub finish_drag: bool,
}

impl AudioPanel {
    pub fn new() -> Self {
        Self {
            mode: "waveform",
            view_start: 0,
            span: 12000,
            gain: 800,
            image: None,
            loading: false,
            requested: None,
            bounds: Rc::new(RefCell::new(Bounds::default())),
            gesture: None,
            dragging: false,
            pending_ms: None,
            drag_loading: false,
            finish_drag: false,
        }
    }

    pub fn time_at(&self, x: Pixels, state: &AudioState) -> i64 {
        let bounds = *self.bounds.borrow();
        let fraction =
            (f32::from(x - bounds.left()) / f32::from(bounds.size.width).max(1.)).clamp(0., 1.);
        (self.view_start + (self.span as f32 * fraction).round() as i64)
            .clamp(0, state.duration.max(0))
    }

    pub fn render(
        &self,
        state: &AudioState,
        video: bool,
        appearance: &crate::Appearance,
        height: f32,
        cx: &mut Context<Workspace>,
    ) -> impl IntoElement {
        self.render_inner(state, video, appearance, cx, true, height)
    }

    pub fn render_compact(
        &self,
        state: &AudioState,
        video: bool,
        appearance: &crate::Appearance,
        height: f32,
        cx: &mut Context<Workspace>,
    ) -> impl IntoElement {
        self.render_inner(state, video, appearance, cx, false, height)
    }

    fn render_inner(
        &self,
        state: &AudioState,
        video: bool,
        appearance: &crate::Appearance,
        cx: &mut Context<Workspace>,
        full_controls: bool,
        graph_height: f32,
    ) -> impl IntoElement {
        // Three groups with different weights: file actions, playback and
        // commit (one primary), then the view/option toggles as quiet pills.
        let separator = Theme::global(cx).border;
        // File, playback and commit actions live in the page toolbar; the row
        // under the waveform only holds view and behaviour options.
        let toolbar = crate::page::toolbar([
            Button::new("open-audio")
                .icon(IconName::FolderOpen)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("打开音频")
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/open"}), cx)
                }))
                .into_any_element(),
            Button::new("audio-from-video")
                .icon(IconName::Video)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("从视频载入")
                .disabled(!video)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/open/video"}), cx)
                }))
                .into_any_element(),
            div()
                .w(px(1.))
                .h(px(18.))
                .mx_1()
                .bg(separator)
                .into_any_element(),
            Button::new("audio-play")
                .icon(IconName::Play)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("播放选区")
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/play/selection"}), cx)
                }))
                .into_any_element(),
            Button::new("audio-stop")
                .icon(IconName::Pause)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("停止")
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/stop"}), cx)
                }))
                .into_any_element(),
            Button::new("audio-save-clip")
                .icon(IconName::ExternalLink)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("导出音频剪辑…")
                .ghost()
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/save/clip"}), cx)
                }))
                .into_any_element(),
            div()
                .w(px(1.))
                .h(px(18.))
                .mx_1()
                .bg(separator)
                .into_any_element(),
            Button::new("audio-commit")
                .icon(IconName::Check)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("提交并前进")
                .primary()
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/commit/next"}), cx)
                }))
                .into_any_element(),
            Button::new("audio-commit-stay")
                .icon(IconName::Check)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("提交")
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/commit/stay"}), cx)
                }))
                .into_any_element(),
            Button::new("audio-revert")
                .icon(IconName::Undo2)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("还原时间")
                .ghost()
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"edit/revert"}), cx)
                }))
                .into_any_element(),
            div().flex_1().into_any_element(),
            Button::new("close-audio")
                .icon(IconName::Close)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("关闭音频")
                .ghost()
                .disabled(!state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/close"}), cx)
                }))
                .into_any_element(),
        ]);
        let mut controls = div().h_flex().flex_wrap().gap_2().items_center();
        controls = controls.child(crate::page::segmented(
            cx,
            [
                ("waveform", "波形", self.mode == "waveform", false),
                ("spectrum", "频谱", self.mode == "spectrum", false),
            ]
            .into_iter()
            .map(|(id, label, selected, disabled)| {
                (format!("audio-mode-{id}"), label, selected, disabled)
            })
            .collect(),
            |view, index, _, cx| {
                let mode = if index == 0 { "waveform" } else { "spectrum" };
                view.audio.mode = mode;
                if view.snapshot.audio.spectrum != (mode == "spectrum") {
                    view.invoke(json!({"op":"command", "name":"audio/opt/spectrum"}), cx);
                }
                view.refresh_audio(cx);
                cx.notify();
            },
        ));
        for (id, label, command, active) in [
            (
                "audio-auto-commit",
                "自动提交",
                "audio/opt/autocommit",
                state.autocommit,
            ),
            (
                "audio-auto-next",
                "提交后前进",
                "audio/opt/autonext",
                state.autonext,
            ),
            (
                "audio-auto-scroll",
                "跟随当前句",
                "audio/opt/autoscroll",
                state.autoscroll,
            ),
        ] {
            controls = controls.child(
                crate::page::toggle_chip(cx, id, label, active, false).on_click(cx.listener(
                    move |view, _, _, cx| view.invoke(json!({"op":"command", "name":command}), cx),
                )),
            );
        }
        controls = controls.child(
            crate::page::toggle_chip(cx, "audio-snap", "吸附", state.snap, false).on_click(
                cx.listener(|view, _, _, cx| {
                    view.invoke(
                        json!({"op":"audio_action", "action":"snap", "enabled":!view.snapshot.audio.snap}),
                        cx,
                    )
                }),
            ),
        );
        controls = controls.child(
            crate::page::toggle_chip(cx, "audio-karaoke-mode", "卡拉 OK", state.karaoke, !state.loaded)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(
                        json!({"op":"audio_action", "action":"karaoke", "enabled":!view.snapshot.audio.karaoke}),
                        cx,
                    )
                })),
        );
        let bounds = self.bounds.clone();
        // The rendered waveform image is drawn for a dark display, so keep the
        // dark track once audio exists and follow the theme while it is empty.
        let graph_bg: gpui_kit::Hsla = if state.loaded {
            rgb(0x101519).into()
        } else {
            Theme::global(cx).muted
        };
        let mut graph = div()
            .id("audio-graph")
            .relative()
            .w_full()
            .h(px(graph_height))
            .flex_shrink_0()
            .rounded(gpui_kit::px(crate::workspace_theme::RADIUS_MD))
            .bg(graph_bg)
            .overflow_hidden()
            .child(
                canvas(
                    move |region, _, _| {
                        *bounds.borrow_mut() = region;
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            );
        if let Some(image) = &self.image {
            graph = graph.child(
                img(image.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            );
        } else {
            graph = graph.child(
                div()
                    .p_4()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(if state.loaded {
                        "正在生成音频画面…"
                    } else {
                        "载入音频后显示波形；视频有音轨时可从视频载入"
                    }),
            );
        }
        let width = f32::from(self.bounds.borrow().size.width).max(1.);
        let left = ((state.start - self.view_start) as f32 / self.span as f32).clamp(0., 1.);
        let right = ((state.end - self.view_start) as f32 / self.span as f32).clamp(0., 1.);
        if state.loaded && right >= left {
            graph = graph.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(width * left))
                    .w(px(width * (right - left)))
                    .bg(gpui_kit::rgba(0x65b8ff26))
                    .border_l_2()
                    .border_color(appearance.color(
                        "Colour/Audio Display/Line boundary Start",
                        rgb(0x7ec2ff).into(),
                    ))
                    .child(div().absolute().right_0().top_0().bottom_0().w(px(2.)).bg(
                        appearance.color(
                            "Colour/Audio Display/Line boundary End",
                            rgb(0x7ec2ff).into(),
                        ),
                    )),
            );
        }
        for &marker in &state.markers {
            if marker < self.view_start || marker > self.view_start + self.span {
                continue;
            }
            let left = (marker - self.view_start) as f32 / self.span as f32 * width;
            graph = graph.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(left))
                    .w(px(1.))
                    .bg(appearance.color(
                        "Colour/Audio Display/Line Boundary Inactive Line",
                        gpui_kit::rgba(0xe5c78b88).into(),
                    )),
            );
        }
        if state.playing {
            let position =
                ((state.position - self.view_start) as f32 / self.span as f32).clamp(0., 1.);
            graph = graph.child(
                div()
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .left(px(width * position))
                    .w(px(2.))
                    .bg(appearance.color("Colour/Audio Display/Play Cursor", rgb(0xffffff).into())),
            );
        }
        let graph = graph
            .on_any_mouse_down(
                cx.listener(|view, event: &gpui_kit::MouseDownEvent, window, cx| {
                    if !view.snapshot.audio.loaded
                        || !matches!(event.button, MouseButton::Left | MouseButton::Right)
                    {
                        return;
                    }
                    view.key_focus = crate::KeyFocus::Audio;
                    view.focus_handle.focus(window, cx);
                    let ms = view.audio.time_at(event.position.x, &view.snapshot.audio);
                    view.begin_audio_drag(
                        ms,
                        event.button == MouseButton::Right,
                        event.modifiers.control || event.modifiers.platform,
                        event.modifiers.alt,
                        cx,
                    );
                }),
            )
            .on_mouse_move(
                cx.listener(|view, event: &gpui_kit::MouseMoveEvent, _, cx| {
                    if !view.audio.dragging {
                        return;
                    }
                    view.audio.pending_ms =
                        Some(view.audio.time_at(event.position.x, &view.snapshot.audio));
                    view.flush_audio_drag(cx);
                }),
            )
            .capture_any_mouse_up(cx.listener(|view, event: &gpui_kit::MouseUpEvent, _, cx| {
                if !view.audio.dragging {
                    return;
                }
                view.audio.pending_ms =
                    Some(view.audio.time_at(event.position.x, &view.snapshot.audio));
                view.audio.finish_drag = true;
                view.flush_audio_drag(cx);
            }))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, event: &gpui_kit::MouseUpEvent, _, cx| {
                    if !view.audio.dragging {
                        return;
                    }
                    view.audio.pending_ms =
                        Some(view.audio.time_at(event.position.x, &view.snapshot.audio));
                    view.audio.finish_drag = true;
                    view.flush_audio_drag(cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Right,
                cx.listener(|view, event: &gpui_kit::MouseUpEvent, _, cx| {
                    if !view.audio.dragging {
                        return;
                    }
                    view.audio.pending_ms =
                        Some(view.audio.time_at(event.position.x, &view.snapshot.audio));
                    view.audio.finish_drag = true;
                    view.flush_audio_drag(cx);
                }),
            );
        // Navigation, zoom and gain are icon buttons with tooltips; the zoom
        // presets below are a segmented control so the current level is visible.
        let icon_button = |id: &'static str, icon: IconName, label: &'static str| {
            Button::new(id)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .icon(icon)
                .accessibility_label(label)
                .tooltip(label)
                .ghost()
                .disabled(!state.loaded)
        };
        let viewport = div()
            .h_flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .child(
                icon_button("audio-left", IconName::ChevronLeft, "向前").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.audio.view_start =
                            (view.audio.view_start - view.audio.span / 2).max(0);
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                icon_button("audio-right", IconName::ChevronRight, "向后").on_click(cx.listener(
                    |view, _, _, cx| {
                        let duration = view.snapshot.audio.duration;
                        view.audio.view_start = (view.audio.view_start + view.audio.span / 2)
                            .min((duration - view.audio.span).max(0));
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                div()
                    .w(px(1.))
                    .h(px(18.))
                    .mx_1()
                    .bg(Theme::global(cx).border),
            )
            .child(
                icon_button("audio-zoom-out", IconName::ZoomOut, "缩小波形").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.audio.span = (view.audio.span * 2).min(600000);
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                icon_button("audio-zoom-in", IconName::ZoomIn, "放大波形").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.audio.span = (view.audio.span / 2).max(250);
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                Button::new("audio-fit-line")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Crosshair)
                    .label("定位当前句")
                    .tooltip("缩放到当前句的时间范围")
                    .ghost()
                    .disabled(!state.loaded)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.audio.view_start = (view.snapshot.audio.start - 1000).max(0);
                        view.audio.span = (view.snapshot.audio.end - view.snapshot.audio.start
                            + 2000)
                            .clamp(1000, 600000);
                        view.refresh_audio(cx);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w(px(1.))
                    .h(px(18.))
                    .mx_1()
                    .bg(Theme::global(cx).border),
            )
            .child(
                icon_button("audio-gain-down", IconName::Volume1, "增益 −").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.audio.gain = (view.audio.gain / 2).max(10);
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(format!("{}%", self.gain)),
            )
            .child(
                icon_button("audio-gain-up", IconName::Volume2, "增益 +").on_click(cx.listener(
                    |view, _, _, cx| {
                        view.audio.gain = (view.audio.gain * 2).min(1000);
                        view.refresh_audio(cx);
                        cx.notify();
                    },
                )),
            );
        // Playback rate is a segmented control so the active speed is visible at
        // a glance; volume uses icon buttons plus its current value.
        let p = Theme::global(cx);
        let mut rates = div()
            .h_flex()
            .items_center()
            .gap_0p5()
            .p_0p5()
            .rounded_full()
            .bg(p.muted);
        for (id, label, rate) in [
            ("audio-rate-50", "0.5×", 50),
            ("audio-rate-100", "1×", 100),
            ("audio-rate-150", "1.5×", 150),
            ("audio-rate-200", "2×", 200),
        ] {
            rates = rates.child(
                Button::new(id)
                    .icon(IconName::Play)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .label(label)
                    .ghost()
                    .when((state.rate - rate as f64).abs() < 0.5, |button| {
                        button.primary()
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.invoke(
                            json!({"op":"audio_action", "action":"rate", "value":rate}),
                            cx,
                        )
                    })),
            );
        }
        let sound = div()
            .h_flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(rates)
            .child(div().w(px(1.)).h(px(18.)).mx_1().bg(p.border))
            .child(
                Button::new("audio-volume-down")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Minus).size(px(15.)))
                    .accessibility_label("音量 −")
                    .tooltip("音量 −10%")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        let volume = (view.snapshot.audio.volume - 10).clamp(0, 200);
                        view.invoke(
                            json!({"op":"audio_action", "action":"volume", "value":volume}),
                            cx,
                        );
                    })),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(p.muted_foreground)
                    .child(format!("音量 {}%", state.volume)),
            )
            .child(
                Button::new("audio-volume-up")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Plus).size(px(15.)))
                    .accessibility_label("音量 +")
                    .tooltip("音量 +10%")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        let volume = (view.snapshot.audio.volume + 10).clamp(0, 200);
                        view.invoke(
                            json!({"op":"audio_action", "action":"volume", "value":volume}),
                            cx,
                        );
                    })),
            );
        let mut karaoke = div().v_flex().gap_2();
        if state.karaoke {
            let mut syllables = div().flex().flex_wrap().gap_2();
            for (index, syllable) in state.syllables.iter().enumerate() {
                let mut cell = div()
                    .v_flex()
                    .p_2()
                    .border_1()
                    .border_color(Theme::global(cx).border)
                    .gap_1()
                    .child(div().text_sm().child(syllable.text.clone()))
                    .child(div().text_xs().child(format!(
                        "{} · {} ms · {}",
                        format_time(syllable.start),
                        syllable.duration,
                        syllable.tag
                    )));
                let mut splits = div().h_flex().gap_1();
                for character in 1..syllable.characters {
                    splits = splits.child(Button::new(format!("split-{index}-{character}")).icon(IconName::Scissors).rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.)).label(format!("{character}｜"))
                        .on_click(cx.listener(move |view, _, _, cx| view.invoke(json!({"op":"audio_action", "action":"karaoke_split", "syllable":index, "character":character}), cx))));
                }
                if index > 0 {
                    splits = splits.child(Button::new(format!("merge-{index}")).icon(IconName::Plus).rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.)).label("合并前一节")
                        .on_click(cx.listener(move |view, _, _, cx| view.invoke(json!({"op":"audio_action", "action":"karaoke_merge", "syllable":index}), cx))));
                }
                cell = cell.child(splits);
                syllables = syllables.child(cell);
            }
            let mut actions = div()
                .h_flex()
                .gap_2()
                .child(
                    Button::new("accept-karaoke-splits")
                        .icon(IconName::Scissors)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("应用音节拆分")
                        .primary()
                        .disabled(!state.pending_splits)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"audio_action", "action":"karaoke_accept"}), cx)
                        })),
                )
                .child(
                    Button::new("cancel-karaoke-splits")
                        .icon(IconName::Undo2)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("还原音节")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"audio_action", "action":"karaoke_cancel"}), cx)
                        })),
                );
            for (id, tag) in [
                ("karaoke-k", "\\k"),
                ("karaoke-kf", "\\kf"),
                ("karaoke-ko", "\\ko"),
            ] {
                actions = actions.child(
                    Button::new(id)
                        .icon(IconName::Asterisk)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label(tag)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.invoke(
                                json!({"op":"audio_action", "action":"karaoke_tag", "tag":tag}),
                                cx,
                            )
                        })),
                );
            }
            karaoke = karaoke.child(actions).child(syllables);
        }
        let load_audio = if video {
            Button::new("audio-load-preview")
                .icon(IconName::Video)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("从视频载入")
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/open/video"}), cx)
                }))
        } else {
            Button::new("open-audio")
                .icon(IconName::FolderOpen)
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .label("打开音频")
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"audio/open"}), cx)
                }))
        };
        let hint = if state.loaded {
            format!(
                "{} → {} · {}",
                format_time(state.start),
                format_time(state.end),
                format_time(state.duration)
            )
        } else {
            "加载视频音轨后可使用波形调时和卡拉 OK 工具".to_owned()
        };
        let mut title_row = div().h_flex().items_center().gap_2();
        if !full_controls {
            title_row = title_row
                .child(Icon::new(IconName::AudioLines).size(px(14.)))
                .child(
                    div()
                        .flex_shrink_0()
                        .text_sm()
                        .font_semibold()
                        .child("音频波形"),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(11.))
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(hint),
                );
        } else {
            title_row = title_row.child(div().flex_1());
        }
        let palette = crate::workspace_theme::Palette::new(Theme::global(cx).is_dark());
        let mut panel = div()
            .v_flex()
            .w_full()
            .min_w_0()
            .gap_3()
            // The waveform strip is a card like every other workspace pane, so the
            // workspace reads as one set of surfaces instead of a bare canvas.
            .rounded(gpui_kit::px(crate::workspace_theme::RADIUS_LG))
            .border_1()
            .border_color(palette.border)
            .bg(palette.surface)
            .shadow(crate::page::card_shadow(Theme::global(cx).is_dark()))
            .p_3()
            .child(title_row.when(!full_controls && !state.loaded, |row| row.child(load_audio)))
            .child(graph);
        if full_controls {
            let divider = Theme::global(cx).border;
            panel = panel
                .child(div().v_flex().gap_2().child(viewport).child(sound).child(karaoke))
                .child(
                    div()
                        .id("audio-page-resize-handle")
                        .aria_label("调整音频波形高度")
                        .role(gpui_kit::Role::Splitter)
                        .h(px(6.))
                        .w_full()
                        .flex_shrink_0()
                        .h_flex()
                        .items_center()
                        .justify_center()
                        .cursor_row_resize()
                        .child(div().w(px(48.)).h(px(3.)).rounded(px(2.)).bg(divider))
                        .on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            cx.listener(|view, event: &gpui_kit::MouseDownEvent, _, cx| {
                                view.layout_drag = Some(crate::WorkspaceResizeDrag::AudioPage {
                                    start_y: event.position.y,
                                    start_height: view.audio_page_height,
                                });
                                cx.stop_propagation();
                                cx.notify();
                            }),
                        ),
                )
                .child(controls)
                .child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child("左键设置开始并拖动结束，右键设置结束；Ctrl/⌘ 拖动重合标记，Alt 拖动整句。时间修改遵循原有自动提交与吸附设置。"),
                )
;
            crate::page::page(cx, "audio-panel")
                .child(toolbar)
                .child(panel)
                .into_any_element()
        } else {
            let compact_viewport = div()
                .h_flex()
                .gap_2()
                .items_center()
                .child(
                    Button::new("audio-gain-down")
                        .icon(IconName::Minus)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("波形增益")
                        .tooltip("降低波形增益（÷2）")
                        .disabled(!state.loaded)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.audio.gain = (view.audio.gain / 2).max(10);
                            view.refresh_audio(cx);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(format!("{}%", self.gain)),
                )
                .child(
                    Button::new("audio-gain-up")
                        .icon(IconName::Plus)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("波形增益")
                        .tooltip("提高波形增益（×2）")
                        .disabled(!state.loaded)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.audio.gain = (view.audio.gain * 2).min(1000);
                            view.refresh_audio(cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("audio-fit-line")
                        .icon(IconName::Locate)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("定位当前句")
                        .disabled(!state.loaded)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.audio.view_start = (view.snapshot.audio.start - 1000).max(0);
                            view.audio.span = (view.snapshot.audio.end - view.snapshot.audio.start
                                + 2000)
                                .clamp(1000, 600000);
                            view.refresh_audio(cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("audio-zoom-in")
                        .icon(IconName::ZoomIn)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("放大波形")
                        .disabled(!state.loaded)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.audio.span = (view.audio.span / 2).max(250);
                            view.refresh_audio(cx);
                            cx.notify();
                        })),
                )
                .child(
                    Button::new("audio-zoom-out")
                        .icon(IconName::ZoomOut)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label("缩小波形")
                        .disabled(!state.loaded)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.audio.span = (view.audio.span * 2).min(600000);
                            view.refresh_audio(cx);
                            cx.notify();
                        })),
                );
            panel = panel
                .child(compact_viewport)
                .child(sound.when(state.karaoke, |row| row.child(karaoke)));
            panel.into_any_element()
        }
    }
}
