use std::collections::BTreeMap;

use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
    switch::Switch,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::Workspace;

#[derive(Clone, Deserialize)]
pub struct PreferenceOption {
    pub name: String,
    pub label: String,
    pub group: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub minimum: f64,
    pub maximum: f64,
    pub directory: bool,
    pub is_default: bool,
    pub choices: Vec<String>,
    pub note: String,
}

#[derive(Deserialize)]
pub struct PreferenceState {
    pub options: Vec<PreferenceOption>,
}

pub struct PreferencesPanel {
    pub options: Vec<PreferenceOption>,
    pub inputs: BTreeMap<String, Entity<InputState>>,
    pub drafts: BTreeMap<String, String>,
    pub search: Entity<InputState>,
    pub group: String,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_reset: bool,
    subscriptions: Vec<Subscription>,
}

impl PreferencesPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索偏好名称或路径"));
        let subscription = cx.subscribe(&search, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.preferences.confirm_reset = false;
                cx.notify();
            }
        });
        Self {
            options: vec![],
            inputs: BTreeMap::new(),
            drafts: BTreeMap::new(),
            search,
            group: "通用".into(),
            loading: false,
            error: None,
            confirm_reset: false,
            subscriptions: vec![subscription],
        }
    }

    pub fn receive(&mut self, state: PreferenceState, submitted: &serde_json::Value) {
        if let Some(fields) = submitted.as_object() {
            self.drafts.retain(|key, value| {
                fields.get(key).and_then(|field| field.as_str()) != Some(value)
            });
        }
        self.options = state.options;
    }

    pub fn sync(view: &mut Workspace, window: &mut Window, cx: &mut Context<Workspace>) {
        for option in view.preferences.options.clone() {
            if option.kind == "bool" || !option.choices.is_empty() {
                continue;
            }
            let value = view
                .preferences
                .drafts
                .get(&option.name)
                .unwrap_or(&option.value)
                .clone();
            if !view.preferences.inputs.contains_key(&option.name) {
                let input = cx.new(|cx| InputState::new(window, cx).default_value(value.clone()));
                let key = option.name.clone();
                let subscription =
                    cx.subscribe(&input, move |view, state, event: &InputEvent, cx| {
                        if matches!(event, InputEvent::Change) {
                            let value = state.read(cx).value().to_string();
                            if view
                                .preferences
                                .options
                                .iter()
                                .any(|option| option.name == key && option.value == value)
                            {
                                view.preferences.drafts.remove(&key);
                            } else {
                                view.preferences.drafts.insert(key.clone(), value);
                            }
                            view.preferences.error = None;
                            cx.notify();
                        }
                    });
                view.preferences.inputs.insert(option.name.clone(), input);
                view.preferences.subscriptions.push(subscription);
            }
            let input = &view.preferences.inputs[&option.name];
            if input.read(cx).value().as_ref() != value {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let query = self.search.read(cx).value().to_lowercase();
        let mut groups = div().h_flex().flex_wrap().gap_1();
        let mut names = vec!["全部".to_owned()];
        for option in &self.options {
            if !names.contains(&option.group) {
                names.push(option.group.clone());
            }
        }
        for (index, group) in names.into_iter().enumerate() {
            let value = group.clone();
            groups = groups.child(
                Button::new(format!("preference-group-{index}"))
                    .label(group.clone())
                    .small()
                    .when(self.group == group, |button| button.primary())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.preferences.group = value.clone();
                        view.preferences.confirm_reset = false;
                        cx.notify();
                    })),
            );
        }
        let mut options = div()
            .id("preference-option-list")
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_0()
            .overflow_y_scrollbar();
        let visible: Vec<_> = self
            .options
            .iter()
            .filter(|option| {
                (self.group == "全部" || self.group == option.group)
                    && (query.is_empty()
                        || format!("{} {}", option.label, option.name)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect();
        for option in &visible {
            let key = option.name.clone();
            let value = self.drafts.get(&key).unwrap_or(&option.value).clone();
            let changed = self.drafts.contains_key(&key);
            let is_default = option.is_default && (!changed || value == option.value);
            let mut description = div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_sm().font_semibold().child(option.label.clone()))
                        .when(changed, |label| {
                            label.child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::global(cx).primary)
                                    .child("未应用"),
                            )
                        }),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(format!(
                            "{}{}",
                            key,
                            if is_default { " · 默认" } else { "" }
                        )),
                );
            if !option.note.is_empty() {
                description = description.child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(option.note.clone()),
                );
            }
            let mut control = div()
                .h_flex()
                .flex_shrink_0()
                .items_center()
                .justify_end()
                .gap_2()
                .w(px(330.));
            if option.kind == "bool" {
                control = control.child(
                    Switch::new(format!("preference-bool-{key}"))
                        .checked(value == "1")
                        .small()
                        .accessibility_label(option.label.clone())
                        .disabled(self.loading)
                        .on_change(cx.listener(move |view, checked: &bool, _, cx| {
                            let value = if *checked { "1" } else { "0" };
                            if view
                                .preferences
                                .options
                                .iter()
                                .any(|option| option.name == key && option.value == value)
                            {
                                view.preferences.drafts.remove(&key);
                            } else {
                                view.preferences.drafts.insert(key.clone(), value.into());
                            }
                            view.preferences.confirm_reset = false;
                            cx.notify();
                        })),
                );
            } else if !option.choices.is_empty() {
                let choices = option.choices.clone();
                let integer = option.kind == "int";
                let loading = self.loading;
                let display = if integer {
                    value
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| choices.get(index))
                        .cloned()
                        .unwrap_or(value.clone())
                } else {
                    value.clone()
                };
                let weak = cx.weak_entity();
                control = control.child(
                    Button::new(format!("preference-choice-{key}"))
                        .label(if display.is_empty() {
                            "自动".into()
                        } else {
                            display
                        })
                        .small()
                        .w(px(280.))
                        .disabled(loading)
                        .dropdown_menu(move |mut menu, _, _| {
                            for (index, label) in choices.iter().enumerate() {
                                let proposed = if integer {
                                    index.to_string()
                                } else {
                                    label.clone()
                                };
                                let weak = weak.clone();
                                let key = key.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(if label.is_empty() {
                                        "自动".into()
                                    } else {
                                        label.clone()
                                    })
                                    .checked(value == proposed)
                                    .disabled(loading)
                                    .on_click(
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |view, cx| {
                                                if view.preferences.options.iter().any(|option| {
                                                    option.name == key && option.value == proposed
                                                }) {
                                                    view.preferences.drafts.remove(&key);
                                                } else {
                                                    view.preferences
                                                        .drafts
                                                        .insert(key.clone(), proposed.clone());
                                                }
                                                view.preferences.confirm_reset = false;
                                                cx.notify();
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        }),
                );
            } else if let Some(input) = self.inputs.get(&key) {
                let hint = if option.kind == "int" || option.kind == "double" {
                    format!("{}–{}", option.minimum, option.maximum)
                } else if option.kind == "color" {
                    "#RRGGBB / #RRGGBBAA".into()
                } else {
                    String::new()
                };
                control = control.child(
                    Input::new(input)
                        .small()
                        .disabled(self.loading)
                        .w(px(if option.directory { 205. } else { 280. }))
                        .aria_label(option.label.clone()),
                );
                if option.directory {
                    control = control.child(
                        Button::new(format!("preference-browse-{key}"))
                            .label("选择…")
                            .small()
                            .disabled(self.loading)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.pick_preference_directory(key.clone(), cx)
                            })),
                    );
                }
                if !hint.is_empty() {
                    description = description.child(
                        div()
                            .text_xs()
                            .text_color(Theme::global(cx).muted_foreground)
                            .child(hint),
                    );
                }
            }
            options = options.child(
                div()
                    .h_flex()
                    .items_center()
                    .gap_4()
                    .px_3()
                    .py_3()
                    .border_b_1()
                    .border_color(Theme::global(cx).border)
                    .child(description)
                    .child(control),
            );
        }

        let reset_names: Vec<_> = visible.iter().map(|option| option.name.clone()).collect();
        let mut toolbar = div()
            .h_flex()
            .flex_wrap()
            .gap_2()
            .items_center()
            .child(
                Button::new("preferences-apply")
                    .label("应用修改")
                    .primary()
                    .small()
                    .disabled(self.loading || self.drafts.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.preferences_request(
                            json!({"action":"apply","fields":view.preferences.drafts}),
                            cx,
                        )
                    })),
            )
            .child(
                Button::new("preferences-discard")
                    .label("放弃未应用修改")
                    .small()
                    .disabled(self.loading || self.drafts.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.preferences.drafts.clear();
                        view.preferences.error = None;
                        view.preferences.confirm_reset = false;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("preferences-reset-visible")
                    .label(if self.confirm_reset {
                        "确认恢复当前列表默认值"
                    } else {
                        "恢复当前列表默认值…"
                    })
                    .small()
                    .disabled(self.loading || reset_names.is_empty())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.preferences.confirm_reset {
                            view.preferences_request(
                                json!({"action":"reset","names":reset_names}),
                                cx,
                            );
                        } else {
                            view.preferences.confirm_reset = true;
                            cx.notify();
                        }
                    })),
            )
            .child(
                Button::new("preferences-hotkeys")
                    .label("快捷键…")
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.active_panel = crate::Panel::Hotkeys;
                        view.hotkey_request(json!({"action":"state"}), cx);
                    })),
            )
            .child(Input::new(&self.search).small().w(px(280.)));
        if self.confirm_reset {
            toolbar = toolbar.child(
                Button::new("preferences-reset-cancel")
                    .label("取消恢复")
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.preferences.confirm_reset = false;
                        cx.notify();
                    })),
            );
        }
        let mut panel = div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .child(div().text_lg().font_semibold().child("偏好设置"))
            .child(groups)
            .child(toolbar)
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(format!(
                        "当前显示 {} 项 · {} 项未应用 · 修改后点击“应用修改”保存",
                        visible.len(),
                        self.drafts.len()
                    )),
            );
        if self.loading {
            panel = panel.child("正在同步设置…");
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        panel.child(options)
    }
}
