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
use serde_json::Value;

use crate::{Snapshot, Workspace};

#[derive(Default, Deserialize)]
pub struct SearchReply {
    #[serde(default)]
    pub search_defaults: Value,
    #[serde(default)]
    pub selection_defaults: Value,
    #[serde(default)]
    pub history: Vec<String>,
    pub count: Option<usize>,
    #[serde(default)]
    pub rows: Vec<i64>,
    pub found: Option<bool>,
    pub replaced: Option<bool>,
    pub row: Option<i64>,
}

pub struct SearchPanel {
    pub find: Entity<InputState>,
    pub replace: Entity<InputState>,
    pub field: i64,
    pub mode: i64,
    pub selection_action: i64,
    pub options: BTreeMap<&'static str, bool>,
    pub selection_mode: bool,
    pub initialized: bool,
    pub loading: bool,
    pub focus_pending: bool,
    pub error: Option<String>,
    pub message: String,
    pub history: Vec<String>,
    pending_find: Option<String>,
    pending_replace: Option<String>,
    _events: Vec<Subscription>,
}

impl SearchPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let find = cx.new(|cx| InputState::new(window, cx));
        let replace = cx.new(|cx| InputState::new(window, cx));
        let mut events = Vec::new();
        for (input, replacing) in [(&find, false), (&replace, true)] {
            events.push(cx.subscribe(input, move |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.find_panel.error = None;
                    cx.notify();
                }
                if matches!(event, InputEvent::PressEnter { .. }) {
                    view.search_request(
                        if view.find_panel.selection_mode {
                            "select"
                        } else if replacing {
                            "replace"
                        } else {
                            "find"
                        },
                        cx,
                    );
                }
            }));
        }
        Self {
            find,
            replace,
            field: 0,
            mode: 1,
            selection_action: 0,
            options: BTreeMap::from([
                ("match_case", false),
                ("use_regex", false),
                ("ignore_comments", false),
                ("skip_tags", false),
                ("only_selection", false),
                ("invert", false),
                ("comments", false),
                ("dialogue", true),
            ]),
            selection_mode: false,
            initialized: false,
            loading: false,
            focus_pending: false,
            error: None,
            message: String::new(),
            history: vec![],
            pending_find: None,
            pending_replace: None,
            _events: events,
        }
    }

    pub fn receive(&mut self, reply: SearchReply) {
        if !self.initialized {
            let defaults = if self.selection_mode {
                &reply.selection_defaults
            } else {
                &reply.search_defaults
            };
            self.field = defaults["field"].as_i64().unwrap_or(0).clamp(0, 3);
            self.mode = defaults["mode"].as_i64().unwrap_or(1).clamp(0, 2);
            self.selection_action = defaults["selection_action"]
                .as_i64()
                .unwrap_or(0)
                .clamp(0, 3);
            for (key, value) in &mut self.options {
                if let Some(option) = defaults[*key].as_bool() {
                    *value = option;
                }
            }
            self.pending_find = Some(defaults["find"].as_str().unwrap_or("").into());
            self.pending_replace = Some(defaults["replace_with"].as_str().unwrap_or("").into());
            self.initialized = true;
        }
        self.history = reply.history;
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(value) = self.pending_find.take() {
            self.find
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
        if let Some(value) = self.pending_replace.take() {
            self.replace
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
        if self.focus_pending {
            self.focus_pending = false;
            self.find.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    pub fn render(&self, snapshot: &Snapshot, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut panel = div()
            .id("search-panel")
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_4()
            .overflow_y_scrollbar()
            .child(
                div()
                    .h_flex()
                    .gap_2()
                    .child(
                        Button::new("search-mode-find")
                            .label("查找替换")
                            .small()
                            .when(!self.selection_mode, |button| button.primary())
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.open_search(false, cx);
                            })),
                    )
                    .child(
                        Button::new("search-mode-select")
                            .label("条件选行")
                            .small()
                            .when(self.selection_mode, |button| button.primary())
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.open_search(true, cx);
                            })),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .gap_1()
                    .w(px(500.))
                    .max_w_full()
                    .child("查找内容")
                    .child(Input::new(&self.find).disabled(disabled)),
            );
        if !self.selection_mode {
            panel = panel.child(
                div()
                    .v_flex()
                    .gap_1()
                    .w(px(500.))
                    .max_w_full()
                    .child("替换为")
                    .child(Input::new(&self.replace).disabled(disabled)),
            );
        }
        let mut field = div().h_flex().flex_wrap().gap_2();
        for (value, label) in [(0, "字幕文本"), (1, "样式"), (2, "角色"), (3, "效果")] {
            field = field.child(
                Button::new(format!("search-field-{value}"))
                    .label(label)
                    .small()
                    .when(self.field == value, |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.find_panel.field = value;
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(div().v_flex().gap_1().child("检索字段").child(field));
        let mut options = div().h_flex().flex_wrap().gap_2();
        let keys = if self.selection_mode {
            vec![
                ("match_case", "区分大小写"),
                ("invert", "选择不匹配项"),
                ("dialogue", "对白"),
                ("comments", "注释"),
            ]
        } else {
            vec![
                ("match_case", "区分大小写"),
                ("use_regex", "正则表达式"),
                ("ignore_comments", "跳过注释"),
                ("skip_tags", "跳过 ASS 标签"),
                ("only_selection", "仅选中行"),
            ]
        };
        for (key, label) in keys {
            options = options.child(
                Button::new(format!("search-option-{key}"))
                    .label(label)
                    .small()
                    .when(self.options[key], |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let value = view.find_panel.options[key];
                        view.find_panel.options.insert(key, !value);
                        if matches!(key, "comments" | "dialogue") && value {
                            let other = if key == "comments" {
                                "dialogue"
                            } else {
                                "comments"
                            };
                            view.find_panel.options.insert(other, true);
                        }
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(options);
        if self.selection_mode {
            let mut modes = div().h_flex().flex_wrap().gap_2();
            for (mode, label) in [(0, "完全匹配"), (1, "包含"), (2, "正则表达式")] {
                modes = modes.child(
                    Button::new(format!("selection-match-mode-{mode}"))
                        .label(label)
                        .small()
                        .when(self.mode == mode, |button| button.primary())
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.find_panel.mode = mode;
                            cx.notify();
                        })),
                );
            }
            let mut actions = div().h_flex().flex_wrap().gap_2();
            for (action, label) in [
                (0, "设为选区"),
                (1, "加入选区"),
                (2, "从选区移除"),
                (3, "与选区相交"),
            ] {
                actions = actions.child(
                    Button::new(format!("selection-action-{action}"))
                        .label(label)
                        .small()
                        .when(self.selection_action == action, |button| button.primary())
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.find_panel.selection_action = action;
                            cx.notify();
                        })),
                );
            }
            panel = panel
                .child(div().v_flex().gap_1().child("匹配方式").child(modes))
                .child(div().v_flex().gap_1().child("选择操作").child(actions));
        }
        let mut actions = div().h_flex().flex_wrap().gap_2();
        let buttons = if self.selection_mode {
            vec![("preview", "预览匹配"), ("select", "应用选择")]
        } else {
            vec![
                ("find", "查找下一个"),
                ("replace", "替换当前并查找"),
                ("replace_all", "全部替换"),
            ]
        };
        for (action, label) in buttons {
            actions = actions.child(
                Button::new(format!("search-{action}"))
                    .label(label)
                    .when(action == "find" || action == "select", |button| {
                        button.primary()
                    })
                    .disabled(
                        disabled || (!self.selection_mode && self.find.read(cx).value().is_empty()),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| view.search_request(action, cx))),
            );
        }
        actions = actions.child(
            Button::new("search-back-to-grid")
                .label("回到字幕")
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, window, cx| {
                    view.active_panel = crate::Panel::Subtitles;
                    view.focus_handle.focus(window, cx);
                    cx.notify();
                })),
        );
        panel = panel.child(actions).child(
            div()
                .text_sm()
                .child(format!("当前选中 {} 行", snapshot.selected.len())),
        );
        if !self.message.is_empty() {
            panel = panel.child(div().text_sm().child(self.message.clone()));
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        if self.loading {
            panel = panel.child("正在检索…");
        }
        if !self.history.is_empty() {
            let mut history = div().h_flex().flex_wrap().gap_2();
            for (index, text) in self.history.iter().enumerate() {
                let value = text.clone();
                history = history.child(
                    Button::new(format!("search-history-{index}"))
                        .label(text.clone())
                        .small()
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.find_panel.pending_find = Some(value.clone());
                            cx.notify();
                        })),
                );
            }
            panel = panel.child(div().v_flex().gap_2().child("最近查找").child(history));
        }
        panel
    }
}
