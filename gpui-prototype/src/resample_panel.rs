use std::collections::BTreeMap;

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::{
    StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
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
        let aspect = crate::page::segmented(
            cx,
            vec![
                (
                    "resample-aspect-0".into(),
                    "拉伸",
                    self.ar_mode == 0,
                    disabled,
                ),
                (
                    "resample-aspect-1".into(),
                    "补边",
                    self.ar_mode == 1,
                    disabled,
                ),
                (
                    "resample-aspect-2".into(),
                    "裁边",
                    self.ar_mode == 2,
                    disabled,
                ),
                (
                    "resample-aspect-3".into(),
                    "手动边缘",
                    self.ar_mode == 3,
                    disabled,
                ),
            ],
            |view, index, _, cx| {
                view.resample.ar_mode = index as i64;
                cx.notify();
            },
        );
        let mut color = div().h_flex().flex_wrap().items_start().gap_4();
        for (source, label, value) in [
            (true, "原色彩矩阵", &self.source_matrix),
            (false, "目标色彩矩阵", &self.dest_matrix),
        ] {
            let weak = cx.weak_entity();
            let selected = value.clone();
            color = color.child(
                div()
                    .v_flex()
                    .w(px(240.))
                    .min_w_0()
                    .gap_1()
                    .child(crate::page::field_label(cx, label))
                    .child(
                        crate::page::select_trigger(
                            cx,
                            if source {
                                "resample-source-matrix"
                            } else {
                                "resample-dest-matrix"
                            },
                            if value.is_empty() {
                                "不转换颜色".to_owned()
                            } else {
                                value.clone()
                            },
                            disabled,
                        )
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
                                    .on_click(
                                        move |_, _, cx| {
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
                                        },
                                    ),
                                );
                            }
                            menu
                        }),
                    ),
            );
        }
        let mut page = crate::page::page(cx, "resample-panel")
            .child(crate::page::toolbar([
                Button::new("resample-apply")
                    .icon(IconName::Check)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .label(if self.loading {
                        "处理中…"
                    } else {
                        "应用重采样"
                    })
                    .primary()
                    .disabled(disabled)
                    .on_click(cx.listener(|view, _, _, cx| view.resample_request("apply", cx)))
                    .into_any_element(),
                Button::new("resample-reset")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::RefreshCw)
                    .label("从当前脚本重新读取")
                    .disabled(self.loading)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.resample.initialized = false;
                        view.resample_request("state", cx);
                    }))
                    .into_any_element(),
                Button::new("resample-from-video")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Video)
                    .label("目标使用视频分辨率")
                    .disabled(disabled || self.state.video_x == 0)
                    .on_click(cx.listener(|view, _, window, cx| {
                        let state = &view.resample.state;
                        for (key, value) in [("dest_x", state.video_x), ("dest_y", state.video_y)] {
                            view.resample.inputs[key].update(cx, |input, cx| {
                                input.set_value(value.to_string(), window, cx)
                            });
                        }
                        if MATRICES.contains(&state.video_matrix.as_str()) {
                            view.resample.dest_matrix = state.video_matrix.clone();
                        }
                        cx.notify();
                    }))
                    .into_any_element(),
                Button::new("resample-undo")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Undo2)
                    .label("撤销")
                    .ghost()
                    .disabled(self.loading)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                    }))
                    .into_any_element(),
            ]))
            .child(crate::page::section(
                cx,
                "尺寸",
                None,
                crate::page::section_body()
                    .child(crate::page::field_row(FIELDS[..4].iter().map(
                        |&(key, label)| {
                            crate::page::field_cell(crate::page::field(
                                cx,
                                label,
                                Input::new(&self.inputs[key]).disabled(disabled),
                            ))
                        },
                    )))
                    .child(crate::page::hint(
                        cx,
                        format!(
                            "当前脚本：{} × {} · 视频：{} × {}",
                            self.state.source_x,
                            self.state.source_y,
                            self.state.video_x,
                            self.state.video_y
                        ),
                    )),
            ))
            .child(crate::page::section(
                cx,
                "宽高比",
                None,
                crate::page::section_body()
                    .child(aspect)
                    .when(self.ar_mode == 3, |body| {
                        body.child(crate::page::field_row(FIELDS[4..].iter().map(
                            |&(key, label)| {
                                crate::page::field_cell(crate::page::field(
                                    cx,
                                    label,
                                    Input::new(&self.inputs[key]).disabled(disabled),
                                ))
                            },
                        )))
                        .child(crate::page::hint(
                            cx,
                            "正值添加边缘，负值裁去边缘；单位为原脚本像素。",
                        ))
                    }),
            ))
            .child(crate::page::section(
                cx,
                "色彩矩阵",
                None,
                crate::page::section_body().child(color),
            ));
        if let Some(error) = &self.error {
            page = page.child(crate::page::error(cx, error.clone()));
        }
        page
    }
}
