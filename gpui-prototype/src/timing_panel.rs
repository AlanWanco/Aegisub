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
use serde_json::{Value, json};

use crate::{Snapshot, Workspace, format_time};

#[derive(Clone, Default, Deserialize)]
pub struct ShiftState {
    pub by_time: bool,
    pub backward: bool,
    pub time: i64,
    pub frames: i64,
    pub mode: i64,
    pub fields: i64,
    pub history: Vec<Value>,
    pub changed: Option<usize>,
}

pub struct TimingPanel {
    pub time: Entity<InputState>,
    pub frames: Entity<InputState>,
    pub by_time: bool,
    pub backward: bool,
    pub mode: i64,
    pub fields: i64,
    pub initialized: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub history: Vec<Value>,
    pub confirm_clear: bool,
    pending_inputs: Option<(String, String)>,
    _events: Vec<Subscription>,
}

impl TimingPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let time = cx.new(|cx| InputState::new(window, cx).default_value("0:00:00.00"));
        let frames = cx.new(|cx| InputState::new(window, cx).default_value("0"));
        let events = [&time, &frames]
            .into_iter()
            .map(|input| {
                cx.subscribe(input, |view, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::PressEnter { .. }) {
                        view.shift_times_request("apply", cx);
                    }
                    if matches!(event, InputEvent::Change) {
                        view.timing.error = None;
                        cx.notify();
                    }
                })
            })
            .collect();
        Self {
            time,
            frames,
            by_time: true,
            backward: false,
            mode: 0,
            fields: 0,
            initialized: false,
            loading: false,
            error: None,
            history: vec![],
            confirm_clear: false,
            pending_inputs: None,
            _events: events,
        }
    }

    pub fn receive(&mut self, state: ShiftState) {
        if !self.initialized {
            self.by_time = state.by_time;
            self.backward = state.backward;
            self.mode = state.mode;
            self.fields = state.fields;
            self.pending_inputs = Some((format_time(state.time), state.frames.to_string()));
            self.initialized = true;
        }
        self.history = state.history;
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        if let Some((time, frames)) = self.pending_inputs.take() {
            self.time
                .update(cx, |state, cx| state.set_value(time, window, cx));
            self.frames
                .update(cx, |state, cx| state.set_value(frames, window, cx));
        }
    }

    fn restore(&mut self, entry: &Value, cx: &App) {
        self.by_time = entry["is by time"].as_bool().unwrap_or(true);
        self.backward = entry["is backward"].as_bool().unwrap_or(false);
        self.mode = entry["mode"].as_i64().unwrap_or(0).clamp(0, 2);
        self.fields = entry["fields"].as_i64().unwrap_or(0).clamp(0, 2);
        let amount = entry["amount"].as_str().unwrap_or("0").to_owned();
        self.pending_inputs = Some(if self.by_time {
            (amount, self.frames.read(cx).value().to_string())
        } else {
            (self.time.read(cx).value().to_string(), amount)
        });
        self.error = None;
    }

    pub fn render(&self, snapshot: &Snapshot, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut units = div().h_flex().flex_wrap().gap_2();
        for (id, label, by_time) in [("time", "按时间", true), ("frames", "按帧", false)] {
            units = units.child(
                Button::new(format!("shift-unit-{id}"))
                    .label(label)
                    .small()
                    .when(self.by_time == by_time, |button| button.primary())
                    .disabled(disabled || (!by_time && !snapshot.timecodes))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.timing.by_time = by_time;
                        view.timing.error = None;
                        cx.notify();
                    })),
            );
        }
        let mut direction = div().h_flex().flex_wrap().gap_2();
        for (label, backward) in [("向后延迟", false), ("向前提前", true)] {
            direction = direction.child(
                Button::new(format!("shift-direction-{backward}"))
                    .label(label)
                    .small()
                    .when(self.backward == backward, |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.timing.backward = backward;
                        cx.notify();
                    })),
            );
        }
        let mut scope = div().h_flex().flex_wrap().gap_2();
        for (mode, label) in [(0, "全部行"), (1, "选中行"), (2, "首个选中行及以后")] {
            scope = scope.child(
                Button::new(format!("shift-mode-{mode}"))
                    .label(label)
                    .small()
                    .when(self.mode == mode, |button| button.primary())
                    .disabled(disabled || (mode > 0 && snapshot.selected.is_empty()))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.timing.mode = mode;
                        cx.notify();
                    })),
            );
        }
        let mut fields = div().h_flex().flex_wrap().gap_2();
        for (field, label) in [(0, "开始和结束"), (1, "只改开始"), (2, "只改结束")] {
            fields = fields.child(
                Button::new(format!("shift-fields-{field}"))
                    .label(label)
                    .small()
                    .when(self.fields == field, |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.timing.fields = field;
                        cx.notify();
                    })),
            );
        }
        let count = match self.mode {
            1 => snapshot.selected.len(),
            2 => snapshot
                .lines
                .iter()
                .position(|line| snapshot.selected.contains(&line.id))
                .map_or(0, |index| snapshot.lines.len() - index),
            _ => snapshot.lines.len(),
        };
        let mut form = div()
            .v_flex()
            .gap_3()
            .w_full()
            .max_w(px(780.))
            .child(div().text_lg().font_semibold().child("平移时间"))
            .child(units)
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .child(if self.by_time {
                        "平移时长"
                    } else {
                        "平移帧数"
                    })
                    .child(
                        Input::new(if self.by_time {
                            &self.time
                        } else {
                            &self.frames
                        })
                        .disabled(disabled)
                        .w(px(260.)),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(Theme::global(cx).muted_foreground)
                            .child(if self.by_time {
                                "格式：时:分:秒.百分秒，例如 0:00:01.50。"
                            } else {
                                "输入非负整数，使用项目当前的帧率或变帧率时间码。"
                            }),
                    ),
            )
            .child(div().v_flex().gap_1().child("方向").child(direction))
            .child(div().v_flex().gap_1().child("影响范围").child(scope))
            .child(div().v_flex().gap_1().child("修改时间字段").child(fields))
            .child(format!(
                "当前范围包含 {count} 行，使用字幕编辑器的撤销记录。"
            ));
        if !snapshot.timecodes {
            form = form.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("载入视频或时间码后可按帧平移。"),
            );
        }
        if let Some(error) = &self.error {
            form = form.child(
                div()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        form = form.child(
            div()
                .h_flex()
                .gap_2()
                .child(
                    Button::new("shift-apply")
                        .label(if self.loading {
                            "处理中…"
                        } else {
                            "应用平移"
                        })
                        .primary()
                        .disabled(disabled || count == 0 || (!self.by_time && !snapshot.timecodes))
                        .on_click(
                            cx.listener(|view, _, _, cx| view.shift_times_request("apply", cx)),
                        ),
                )
                .child(
                    Button::new("shift-undo")
                        .label("撤销")
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                        })),
                ),
        );
        let mut history = div()
            .v_flex()
            .gap_2()
            .child(div().text_lg().font_semibold().child("历史记录"))
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("点击记录恢复平移参数；影响范围使用当前文档的选择。"),
            );
        for (index, entry) in self.history.iter().enumerate() {
            let entry = entry.clone();
            let filename = entry["filename"]
                .as_str()
                .filter(|name| !name.is_empty())
                .unwrap_or("未命名字幕");
            let amount = entry["amount"].as_str().unwrap_or("?");
            let unit = if entry["is by time"] == true {
                ""
            } else {
                " 帧"
            };
            let direction = if entry["is backward"] == true {
                "提前"
            } else {
                "延迟"
            };
            let scope = match entry["mode"].as_i64() {
                Some(1) => "选中行",
                Some(2) => "选择及以后",
                _ => "全部行",
            };
            let fields = match entry["fields"].as_i64() {
                Some(1) => "开始",
                Some(2) => "结束",
                _ => "开始+结束",
            };
            history = history.child(
                Button::new(format!("shift-history-{index}"))
                    .label(format!(
                        "{filename} · {direction} {amount}{unit} · {scope} · {fields}"
                    ))
                    .small()
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.timing.restore(&entry, cx);
                        cx.notify();
                    })),
            );
        }
        if self.history.is_empty() {
            history = history.child(div().text_sm().child("尚无平移记录。"));
        } else {
            history = history.child(
                Button::new("shift-clear-history")
                    .label(if self.confirm_clear {
                        "确认清空历史记录"
                    } else {
                        "清空历史记录…"
                    })
                    .small()
                    .disabled(disabled)
                    .on_click(cx.listener(|view, _, _, cx| {
                        if view.timing.confirm_clear {
                            view.timing.confirm_clear = false;
                            view.shift_times_request("clear_history", cx);
                        } else {
                            view.timing.confirm_clear = true;
                            cx.notify();
                        }
                    })),
            );
        }
        div()
            .id("timing-panel")
            .flex_1()
            .min_h_0()
            .p_4()
            .v_flex()
            .gap_6()
            .overflow_y_scrollbar()
            .child(form)
            .child(history)
    }
}
