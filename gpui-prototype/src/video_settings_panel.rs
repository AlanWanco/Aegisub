use std::collections::BTreeMap;

use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;

use crate::{Workspace, format_time};

#[derive(Clone, Copy, PartialEq)]
pub enum VideoPage {
    Dummy,
    Jump,
    Aspect,
    Details,
}

#[derive(Default, Deserialize)]
#[serde(default)]
pub struct VideoSettings {
    pub loaded: bool,
    pub path: String,
    pub width: i64,
    pub height: i64,
    pub frames: i64,
    pub fps: f64,
    pub duration: i64,
    pub decoder: String,
    pub actual_matrix: String,
    pub forced_matrix: String,
    pub actual_range: String,
    pub forced_range: String,
    pub frame: i64,
    pub time: i64,
    pub aspect: f64,
    pub dummy_fps: String,
    pub dummy_width: i64,
    pub dummy_height: i64,
    pub dummy_length: i64,
    pub dummy_color: String,
    pub dummy_pattern: bool,
}

pub struct VideoSettingsPanel {
    pub state: VideoSettings,
    pub page: VideoPage,
    pub inputs: BTreeMap<&'static str, Entity<InputState>>,
    pub pattern: bool,
    pub initialized: bool,
    pub loading: bool,
    pub error: Option<String>,
    pending_values: BTreeMap<&'static str, String>,
    _events: Vec<Subscription>,
}

impl VideoSettingsPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let mut inputs = BTreeMap::new();
        let mut events = Vec::new();
        for key in [
            "fps", "width", "height", "length", "color", "frame", "time", "aspect",
        ] {
            let input = cx.new(|cx| InputState::new(window, cx));
            events.push(
                cx.subscribe(&input, move |view, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        view.video_settings.error = None;
                        cx.notify();
                    }
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        let action = match key {
                            "frame" => "jump_frame",
                            "time" => "jump_time",
                            "aspect" => "aspect",
                            _ => "dummy",
                        };
                        view.video_settings_request(action, cx);
                    }
                }),
            );
            inputs.insert(key, input);
        }
        Self {
            state: VideoSettings::default(),
            page: VideoPage::Details,
            inputs,
            pattern: false,
            initialized: false,
            loading: false,
            error: None,
            pending_values: BTreeMap::new(),
            _events: events,
        }
    }

    pub fn receive(&mut self, state: VideoSettings) {
        if !self.initialized {
            self.pending_values = BTreeMap::from([
                ("fps", state.dummy_fps.clone()),
                ("width", state.dummy_width.to_string()),
                ("height", state.dummy_height.to_string()),
                ("length", state.dummy_length.to_string()),
                ("color", state.dummy_color.clone()),
                ("frame", state.frame.to_string()),
                ("time", format_time(state.time)),
                ("aspect", format!("{:.6}", state.aspect)),
            ]);
            self.pattern = state.dummy_pattern;
            self.initialized = true;
        }
        self.state = state;
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        for (key, value) in std::mem::take(&mut self.pending_values) {
            self.inputs[key].update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut pages = div().h_flex().flex_wrap().gap_2();
        for (page, id, label) in [
            (VideoPage::Details, "details", "视频详情"),
            (VideoPage::Jump, "jump", "精确跳转"),
            (VideoPage::Aspect, "aspect", "显示比例"),
            (VideoPage::Dummy, "dummy", "空白视频"),
        ] {
            pages = pages.child(
                Button::new(format!("video-page-{id}"))
                    .label(label)
                    .when(self.page == page, |button| button.primary())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.video_settings.page = page;
                        cx.notify();
                    })),
            );
        }
        let mut panel = div()
            .id("video-settings-panel")
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_4()
            .overflow_y_scrollbar()
            .child(pages);
        let fields: Vec<_> = match self.page {
            VideoPage::Dummy => vec![
                ("width", "宽度 · 像素"),
                ("height", "高度 · 像素"),
                ("fps", "帧率 · 支持 24000/1001"),
                ("length", "持续帧数"),
                ("color", "颜色 · #RRGGBB"),
            ],
            VideoPage::Jump => vec![
                ("frame", "跳转帧号 · 从 0 开始"),
                ("time", "跳转时间 · 时:分:秒.百分秒"),
            ],
            VideoPage::Aspect => vec![("aspect", "比例 · 小数、16:9 或 853x480")],
            _ => vec![],
        };
        if self.page == VideoPage::Dummy {
            let mut resolutions = div().h_flex().flex_wrap().gap_2();
            for (width, height) in [
                (640, 480),
                (1280, 720),
                (1920, 1080),
                (2560, 1440),
                (3840, 2160),
                (1080, 1920),
            ] {
                resolutions = resolutions.child(
                    Button::new(format!("dummy-resolution-{width}-{height}"))
                        .label(format!("{width} × {height}"))
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, window, cx| {
                            for (key, value) in [("width", width), ("height", height)] {
                                view.video_settings.inputs[key].update(cx, |state, cx| {
                                    state.set_value(value.to_string(), window, cx)
                                });
                            }
                        })),
                );
            }
            panel = panel
                .child(div().text_lg().font_semibold().child("创建空白视频"))
                .child(resolutions);
        }
        let mut inputs = div().h_flex().flex_wrap().gap_3();
        for (key, label) in fields {
            inputs = inputs.child(
                div()
                    .v_flex()
                    .gap_1()
                    .w(px(290.))
                    .child(div().text_sm().child(label))
                    .child(Input::new(&self.inputs[key]).disabled(
                        disabled || (self.page != VideoPage::Dummy && !self.state.loaded),
                    )),
            );
        }
        panel = panel.child(inputs);
        match self.page {
            VideoPage::Dummy => {
                panel = panel
                    .child(
                        Button::new("dummy-pattern")
                            .label("棋盘格图案")
                            .when(self.pattern, |button| button.primary())
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.video_settings.pattern = !view.video_settings.pattern;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("dummy-video-create")
                            .label("载入空白视频")
                            .primary()
                            .disabled(disabled)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.video_settings_request("dummy", cx)
                            })),
                    );
            }
            VideoPage::Jump => {
                panel = panel.child(
                    div()
                        .h_flex()
                        .gap_2()
                        .child(
                            Button::new("jump-frame-apply")
                                .label("按帧跳转")
                                .primary()
                                .disabled(disabled || !self.state.loaded)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.video_settings_request("jump_frame", cx)
                                })),
                        )
                        .child(
                            Button::new("jump-time-apply")
                                .label("按时间跳转")
                                .disabled(disabled || !self.state.loaded)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.video_settings_request("jump_time", cx)
                                })),
                        ),
                );
            }
            VideoPage::Aspect => {
                panel = panel
                    .child(
                        Button::new("custom-aspect-apply")
                            .label("应用显示比例")
                            .primary()
                            .disabled(disabled || !self.state.loaded)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.video_settings_request("aspect", cx)
                            })),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(Theme::global(cx).muted_foreground)
                            .child("有效范围 0.5–5.0，调整视频预览的显示比例。"),
                    );
            }
            VideoPage::Details => {
                let state = &self.state;
                if state.loaded {
                    for (label, value) in [
                        ("文件", state.path.clone()),
                        ("分辨率", format!("{} × {}", state.width, state.height)),
                        ("帧率", format!("{:.6}", state.fps)),
                        (
                            "长度",
                            format!("{} 帧 · {}", state.frames, format_time(state.duration)),
                        ),
                        (
                            "当前帧",
                            format!("{} · {}", state.frame, format_time(state.time)),
                        ),
                        ("显示比例", format!("{:.6}", state.aspect)),
                        ("实际色彩矩阵", state.actual_matrix.clone()),
                        ("强制色彩矩阵", state.forced_matrix.clone()),
                        ("实际范围", state.actual_range.clone()),
                        ("强制范围", state.forced_range.clone()),
                        ("解码器", state.decoder.clone()),
                    ] {
                        panel = panel.child(div().text_sm().child(format!("{label}：{value}")));
                    }
                } else {
                    panel = panel.child("尚未载入视频。可打开视频文件或创建空白视频。");
                }
                panel = panel.child(
                    Button::new("refresh-video-details")
                        .label("刷新详情")
                        .disabled(self.loading)
                        .on_click(
                            cx.listener(|view, _, _, cx| view.video_settings_request("state", cx)),
                        ),
                );
            }
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        panel.child(
            Button::new("video-settings-back")
                .label("回到字幕预览")
                .on_click(cx.listener(|view, _, window, cx| {
                    view.active_panel = crate::Panel::Subtitles;
                    view.focus_handle.focus(window, cx);
                    cx.notify();
                })),
        )
    }
}
