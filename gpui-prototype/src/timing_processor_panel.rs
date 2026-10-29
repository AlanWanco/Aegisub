use std::collections::{BTreeMap, BTreeSet};

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::Workspace;

pub const NUMBERS: &[(&str, &str)] = &[
    ("lead_in", "提前开始 · 毫秒"),
    ("lead_out", "延后结束 · 毫秒"),
    ("adjacent_gap", "最大间隔 · 毫秒"),
    ("adjacent_overlap", "最大重叠 · 毫秒"),
    ("adjacent_bias", "衔接偏向 · 0–100"),
    ("before_start", "开始早于关键帧 · 毫秒"),
    ("after_start", "开始晚于关键帧 · 毫秒"),
    ("before_end", "结束早于关键帧 · 毫秒"),
    ("after_end", "结束晚于关键帧 · 毫秒"),
];

#[derive(Default, Deserialize)]
pub struct ProcessorState {
    #[serde(flatten)]
    pub values: BTreeMap<String, serde_json::Value>,
    pub styles: Vec<String>,
    pub can_keyframes: bool,
    pub changed: Option<usize>,
}

pub struct TimingProcessorPanel {
    pub inputs: BTreeMap<&'static str, Entity<InputState>>,
    pub options: BTreeMap<&'static str, bool>,
    pub styles: Vec<String>,
    pub selected_styles: BTreeSet<String>,
    pub loading: bool,
    pub initialized: bool,
    pub can_keyframes: bool,
    pub error: Option<String>,
    pending_values: BTreeMap<&'static str, String>,
    _events: Vec<Subscription>,
}

impl TimingProcessorPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let mut inputs = BTreeMap::new();
        let mut events = Vec::new();
        for &(key, _) in NUMBERS {
            let input = cx.new(|cx| InputState::new(window, cx));
            events.push(cx.subscribe(&input, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.processor.error = None;
                    cx.notify();
                }
            }));
            inputs.insert(key, input);
        }
        Self {
            inputs,
            options: BTreeMap::from([
                ("has_lead_in", false),
                ("has_lead_out", false),
                ("adjacent", false),
                ("keyframes", false),
                ("only_selection", false),
            ]),
            styles: vec![],
            selected_styles: BTreeSet::new(),
            loading: false,
            initialized: false,
            can_keyframes: false,
            error: None,
            pending_values: BTreeMap::new(),
            _events: events,
        }
    }

    pub fn receive(&mut self, state: ProcessorState) {
        if !self.initialized {
            for &(key, _) in NUMBERS {
                self.pending_values.insert(
                    key,
                    state
                        .values
                        .get(key)
                        .and_then(|value| value.as_i64())
                        .unwrap_or(0)
                        .to_string(),
                );
            }
            for (key, value) in &mut self.options {
                *value = state
                    .values
                    .get(*key)
                    .and_then(|value| value.as_bool())
                    .unwrap_or(false);
            }
            self.selected_styles = state.styles.iter().cloned().collect();
            self.initialized = true;
        }
        self.styles = state.styles;
        self.selected_styles
            .retain(|style| self.styles.contains(style));
        self.can_keyframes = state.can_keyframes;
        if !self.can_keyframes {
            self.options.insert("keyframes", false);
        }
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        for (key, value) in std::mem::take(&mut self.pending_values) {
            self.inputs[key].update(cx, |state, cx| state.set_value(value, window, cx));
        }
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut styles = div().h_flex().flex_wrap().gap_2();
        for (index, style) in self.styles.iter().enumerate() {
            let name = style.clone();
            let on = self.selected_styles.contains(style);
            styles = styles.child(
                crate::page::toggle_chip(
                    cx,
                    format!("processor-style-{index}"),
                    style.clone(),
                    on,
                    disabled,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    if !view.processor.selected_styles.remove(&name) {
                        view.processor.selected_styles.insert(name.clone());
                    }
                    cx.notify();
                })),
            );
        }
        let mut toggles = div().h_flex().flex_wrap().gap_2();
        for (key, label) in [
            ("only_selection", "仅选中行"),
            ("has_lead_in", "提前开始"),
            ("has_lead_out", "延后结束"),
            ("adjacent", "衔接相邻字幕"),
            ("keyframes", "吸附关键帧"),
        ] {
            let on = self.options[key];
            let off_limits = disabled || (key == "keyframes" && !self.can_keyframes);
            toggles = toggles.child(
                crate::page::toggle_chip(cx, format!("processor-{key}"), label, on, off_limits)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let value = view.processor.options[key];
                        view.processor.options.insert(key, !value);
                        cx.notify();
                    })),
            );
        }
        // A fixed-width wrap left the ninth field alone on its own line; lay the
        // thresholds out as a 3×3 grid instead.
        let mut numbers = div().v_flex().gap_3();
        for row in NUMBERS.chunks(3) {
            let mut line = div().h_flex().items_start().gap_3();
            for &(key, label) in row {
                let enabled = match key {
                    "lead_in" => self.options["has_lead_in"],
                    "lead_out" => self.options["has_lead_out"],
                    "adjacent_gap" | "adjacent_overlap" | "adjacent_bias" => {
                        self.options["adjacent"]
                    }
                    _ => self.options["keyframes"] && self.can_keyframes,
                };
                line = line.child(
                    div()
                        .v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(div().text_sm().child(label))
                        .child(Input::new(&self.inputs[key]).disabled(disabled || !enabled)),
                );
            }
            for _ in row.len()..3 {
                line = line.child(div().flex_1().min_w_0());
            }
            numbers = numbers.child(line);
        }
        let enabled = ["has_lead_in", "has_lead_out", "adjacent", "keyframes"]
            .iter()
            .any(|key| self.options[key]);
        let mut page = crate::page::page(cx, "timing-processor-panel")
            .child(crate::page::header(
                cx,
                IconName::Clock3,
                "时间后处理",
                "按开始时间处理所选样式，跳过注释；提前/延后会避开原本不重叠的字幕，整次操作可撤销。",
            ))
            .child(crate::page::toolbar([
                Button::new("processor-apply").icon(IconName::Check).rounded(gpui_kit::px(999.))
                    .label(if self.loading {
                        "处理中…"
                    } else {
                        "应用后处理"
                    })
                    .primary()
                    .disabled(disabled || self.selected_styles.is_empty() || !enabled)
                    .on_click(cx.listener(|view, _, _, cx| view.processor_request("apply", cx)))
                    .into_any_element(),
                Button::new("processor-undo").icon(IconName::Undo2).rounded(gpui_kit::px(999.))
                    .label("撤销")
                    .disabled(self.loading)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                    }))
                    .into_any_element(),
                Button::new("processor-open-keyframes").icon(IconName::Locate).rounded(gpui_kit::px(999.))
                    .label("关键帧…")
                    .disabled(self.can_keyframes)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"keyframe/open"}), cx)
                    }))
                    .into_any_element(),
            ]))
            // Two columns: which styles to process beside the thresholds,
            // so the page is not one long stack of full-width cards.
            .child(
                div()
                    .h_flex()
                    .items_start()
                    .w_full()
                    .min_w_0()
                    .gap_4()
                    .child(
                        div()
                            .w(px(330.))
                            .flex_shrink_0()
                            .min_w_0()
                            .child(crate::page::section_with_icon(
                                cx,
                                Some(IconName::Palette),
                                "处理哪些样式",
                                Some(
                                    Button::new("processor-refresh").icon(IconName::RefreshCw).rounded(gpui_kit::px(999.))
                                        .label("刷新参数")
                                        .ghost()
                                        .disabled(self.loading)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.processor.initialized = false;
                                            view.processor_request("state", cx);
                                        }))
                                        .into_any_element(),
                                ),
                                crate::page::section_body()
                                    .child(crate::page::field_label(cx, "已选样式"))
                                    .child(styles)
                                    .child(
                                        div()
                                            .h_flex()
                                            .gap_2()
                                            .child(
                                                Button::new("processor-all-styles").icon(IconName::Check).rounded(gpui_kit::px(999.))
                                                    .label("全选")
                                                    .ghost()
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.processor.selected_styles = view
                                                            .processor
                                                            .styles
                                                            .iter()
                                                            .cloned()
                                                            .collect();
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                Button::new("processor-no-styles").icon(IconName::CircleX).rounded(gpui_kit::px(999.))
                                                    .label("清空")
                                                    .ghost()
                                                    .disabled(disabled)
                                                    .on_click(cx.listener(|view, _, _, cx| {
                                                        view.processor.selected_styles.clear();
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )),
                    )
                    .child(div().flex_1().min_w_0().child(crate::page::section_with_icon(
                cx,
                Some(IconName::Clock3),
                "阈值",
                None,
                crate::page::section_body()
                    .child(toggles)
                    .when(!self.can_keyframes, |body| {
                        body.child(crate::page::hint(
                            cx,
                            "载入视频、时间码和关键帧后才能吸附关键帧。",
                        ))
                    })
                    .child(numbers)
                    .child(crate::page::hint(
                        cx,
                        "衔接偏向：0 只改后一句开始，100 只改前一句结束，50 取两者中间。",
                    )),
            ))),
            );
        if let Some(error) = &self.error {
            page = page.child(crate::page::error(cx, error.clone()));
        }
        page
    }
}
