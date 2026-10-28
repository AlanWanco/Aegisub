use std::collections::BTreeMap;

use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::Workspace;

pub const FIELDS: &[(&str, &str)] = &[
    ("source_x", "原宽度"),
    ("source_y", "原高度"),
    ("dest_x", "目标宽度"),
    ("dest_y", "目标高度"),
    ("left", "左边缘偏移"),
    ("right", "右边缘偏移"),
    ("top", "上边缘偏移"),
    ("bottom", "下边缘偏移"),
];
const MATRICES: &[&str] = &[
    "", "TV.601", "PC.601", "TV.709", "PC.709", "TV.FCC", "PC.FCC", "TV.240M", "PC.240M",
];

#[derive(Default, Deserialize)]
pub struct ResampleState {
    pub source_x: i64,
    pub source_y: i64,
    pub source_matrix: String,
    #[serde(default)]
    pub video_x: i64,
    #[serde(default)]
    pub video_y: i64,
    #[serde(default)]
    pub video_matrix: String,
}

pub struct ResamplePanel {
    pub inputs: BTreeMap<&'static str, Entity<InputState>>,
    pub state: ResampleState,
    pub source_matrix: String,
    pub dest_matrix: String,
    pub ar_mode: i64,
    pub loading: bool,
    pub initialized: bool,
    pub error: Option<String>,
    pending_inputs: Option<BTreeMap<&'static str, String>>,
    _events: Vec<Subscription>,
}

impl ResamplePanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let mut inputs = BTreeMap::new();
        let mut events = Vec::new();
        for &(key, _) in FIELDS {
            let input = cx.new(|cx| InputState::new(window, cx));
            events.push(cx.subscribe(&input, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.resample.error = None;
                    cx.notify();
                }
            }));
            inputs.insert(key, input);
        }
        Self {
            inputs,
            state: ResampleState::default(),
            source_matrix: String::new(),
            dest_matrix: String::new(),
            ar_mode: 0,
            loading: false,
            initialized: false,
            error: None,
            pending_inputs: None,
            _events: events,
        }
    }

    pub fn receive(&mut self, state: ResampleState) {
        if !self.initialized {
            let target_x = if state.video_x > 0 {
                state.video_x
            } else {
                state.source_x
            };
            let target_y = if state.video_y > 0 {
                state.video_y
            } else {
                state.source_y
            };
            self.pending_inputs = Some(BTreeMap::from([
                ("source_x", state.source_x.to_string()),
                ("source_y", state.source_y.to_string()),
                ("dest_x", target_x.to_string()),
                ("dest_y", target_y.to_string()),
                ("left", "0".into()),
                ("right", "0".into()),
                ("top", "0".into()),
                ("bottom", "0".into()),
            ]));
            self.source_matrix.clear();
            self.dest_matrix.clear();
            if !state.source_matrix.is_empty()
                && MATRICES.contains(&state.source_matrix.as_str())
                && MATRICES.contains(&state.video_matrix.as_str())
                && !state.video_matrix.is_empty()
                && state.video_matrix != state.source_matrix
            {
                self.source_matrix = state.source_matrix.clone();
                self.dest_matrix = state.video_matrix.clone();
            }
            self.initialized = true;
        }
        self.state = state;
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(fields) = self.pending_inputs.take() {
            for (key, value) in fields {
                self.inputs[key].update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut form = div()
            .id("resample-panel")
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_4()
            .p_4()
            .overflow_y_scrollbar()
            .child(div().text_lg().font_semibold().child("分辨率重采样"))
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("同步缩放样式、坐标、边距、绘图和裁剪；整次处理可撤销。"),
            )
            .child(
                div()
                    .h_flex()
                    .flex_wrap()
                    .gap_2()
                    .child(
                        Button::new("resample-reset")
                            .label("从当前脚本重新读取")
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.resample.initialized = false;
                                view.resample_request("state", cx);
                            })),
                    )
                    .child(
                        Button::new("resample-from-video")
                            .label("目标使用视频分辨率")
                            .disabled(disabled || self.state.video_x == 0)
                            .on_click(cx.listener(|view, _, window, cx| {
                                let state = &view.resample.state;
                                for (key, value) in
                                    [("dest_x", state.video_x), ("dest_y", state.video_y)]
                                {
                                    view.resample.inputs[key].update(cx, |input, cx| {
                                        input.set_value(value.to_string(), window, cx)
                                    });
                                }
                                if MATRICES.contains(&state.video_matrix.as_str()) {
                                    view.resample.dest_matrix = state.video_matrix.clone();
                                }
                                cx.notify();
                            })),
                    ),
            );
        let mut dimensions = div().h_flex().flex_wrap().gap_3();
        for &(key, label) in &FIELDS[..4] {
            dimensions = dimensions.child(
                div()
                    .v_flex()
                    .gap_1()
                    .w(px(180.))
                    .child(label)
                    .child(Input::new(&self.inputs[key]).disabled(disabled)),
            );
        }
        form = form.child(dimensions).child(div().text_sm().child(format!(
            "当前脚本：{} × {} · 视频：{} × {}",
            self.state.source_x, self.state.source_y, self.state.video_x, self.state.video_y
        )));
        let mut aspect = div().h_flex().flex_wrap().gap_2();
        for (mode, label) in [(0, "拉伸"), (1, "补边"), (2, "裁边"), (3, "手动边缘")] {
            aspect = aspect.child(
                Button::new(format!("resample-aspect-{mode}"))
                    .label(label)
                    .when(self.ar_mode == mode, |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.resample.ar_mode = mode;
                        cx.notify();
                    })),
            );
        }
        form = form.child(div().v_flex().gap_2().child("宽高比处理").child(aspect));
        if self.ar_mode == 3 {
            let mut margins = div().h_flex().flex_wrap().gap_3();
            for &(key, label) in &FIELDS[4..] {
                margins = margins.child(
                    div()
                        .v_flex()
                        .gap_1()
                        .w(px(180.))
                        .child(label)
                        .child(Input::new(&self.inputs[key]).disabled(disabled)),
                );
            }
            form = form.child(margins).child(
                div()
                    .text_sm()
                    .child("正值添加边缘，负值裁去边缘；单位为原脚本像素。"),
            );
        }
        let mut color = div().h_flex().flex_wrap().gap_3();
        for (source, label, value) in [
            (true, "原色彩矩阵", &self.source_matrix),
            (false, "目标色彩矩阵", &self.dest_matrix),
        ] {
            let weak = cx.weak_entity();
            let selected = value.clone();
            color = color.child(
                div().v_flex().gap_1().child(label).child(
                    Button::new(if source {
                        "resample-source-matrix"
                    } else {
                        "resample-dest-matrix"
                    })
                    .label(if value.is_empty() {
                        "不转换颜色".to_owned()
                    } else {
                        value.clone()
                    })
                    .disabled(disabled)
                    .dropdown_menu(move |mut menu, _, _| {
                        for &matrix in MATRICES {
                            let weak = weak.clone();
                            menu = menu.item(
                                PopupMenuItem::new(if matrix.is_empty() {
                                    "不转换颜色"
                                } else {
                                    matrix
                                })
                                .checked(selected == matrix)
                                .on_click(move |_, _, cx| {
                                    let _ = weak.update(cx, |view, cx| {
                                        if matrix.is_empty() {
                                            view.resample.source_matrix.clear();
                                            view.resample.dest_matrix.clear();
                                        } else if source {
                                            view.resample.source_matrix = matrix.into();
                                        } else {
                                            view.resample.dest_matrix = matrix.into();
                                        }
                                        cx.notify();
                                    });
                                }),
                            );
                        }
                        menu
                    }),
                ),
            );
        }
        form = form.child(color);
        if let Some(error) = &self.error {
            form = form.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        form.child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    Button::new("resample-apply")
                        .label(if self.loading {
                            "处理中…"
                        } else {
                            "应用重采样"
                        })
                        .primary()
                        .disabled(disabled)
                        .on_click(cx.listener(|view, _, _, cx| view.resample_request("apply", cx))),
                )
                .child(
                    Button::new("resample-undo")
                        .label("撤销")
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                        })),
                ),
        )
    }
}
