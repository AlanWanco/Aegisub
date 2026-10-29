use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::{
    StyledExt, Theme,
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

#[derive(Default, Deserialize)]
pub struct SpellState {
    pub available: bool,
    pub language: String,
    pub languages: Vec<String>,
    pub skip_comments: bool,
    pub skip_uppercase: bool,
    pub complete: bool,
    pub found: bool,
    pub changed: usize,
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub row: usize,
    #[serde(default)]
    pub start: usize,
    #[serde(default)]
    pub length: usize,
    #[serde(default)]
    pub word: String,
    #[serde(default)]
    pub line_text: String,
    #[serde(default)]
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub can_add: bool,
    #[serde(default)]
    pub can_remove: bool,
    #[serde(default)]
    pub dictionary_word: String,
}

pub struct SpellingPanel {
    pub state: SpellState,
    pub replacement: Entity<InputState>,
    pub loading: bool,
    pub error: Option<String>,
    pub pending: Option<String>,
    pub confirm_remove: bool,
    pub dictionary_checked: bool,
    pub dictionary_word: String,
    _events: Subscription,
}

impl SpellingPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let replacement =
            cx.new(|cx| InputState::new(window, cx).placeholder("选择建议或输入替换文本"));
        let events = cx.subscribe(&replacement, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.spelling.confirm_remove = false;
                view.spelling.state.can_remove = false;
                view.spelling.dictionary_checked = false;
                cx.notify();
            }
            if matches!(event, InputEvent::PressEnter { .. }) && view.spelling.state.found {
                view.spelling_request(json!({"action":"replace"}), cx);
            }
        });
        Self {
            state: SpellState::default(),
            replacement,
            loading: false,
            error: None,
            pending: None,
            confirm_remove: false,
            dictionary_checked: false,
            dictionary_word: String::new(),
            _events: events,
        }
    }

    pub fn receive(&mut self, state: SpellState, refresh_replacement: bool) {
        if refresh_replacement
            || state.word != self.state.word
            || state.id != self.state.id
            || state.start != self.state.start
        {
            self.pending = Some(
                state
                    .suggestions
                    .first()
                    .cloned()
                    .unwrap_or_else(|| state.word.clone()),
            );
        }
        self.state = state;
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(text) = self.pending.take() {
            self.replacement
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let selected = self.state.language.clone();
        let languages = self.state.languages.clone();
        let weak = cx.weak_entity();
        let mut toolbar = div()
            .h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                Button::new("spelling-start").rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                    .icon(IconName::Check)
                    .label("从当前行检查")
                    .primary()

                    .disabled(self.loading || !self.state.available)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.spelling_request(json!({"action":"start"}), cx)
                    })),
            )
            .child(
                Button::new("spelling-language").rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                    .icon(IconName::Languages)
                    .label(if selected.is_empty() {
                        "选择语言".into()
                    } else {
                        format!("语言 {selected}")
                    })

                    .disabled(self.loading || languages.is_empty())
                    .dropdown_menu(move |mut menu, _, _| {
                        for language in &languages {
                            let weak = weak.clone();
                            let value = language.clone();
                            menu = menu.item(
                                PopupMenuItem::new(value.clone())
                                    .checked(value == selected)
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |view, cx| {
                                            view.spelling_request(
                                                json!({"action":"configure","language":value,"skip_comments":view.spelling.state.skip_comments,"skip_uppercase":view.spelling.state.skip_uppercase}),
                                                cx,
                                            )
                                        });
                                    }),
                            );
                        }
                        menu
                    }),
            );
        for (id, label, value) in [
            ("comments", "跳过注释", self.state.skip_comments),
            ("uppercase", "忽略全大写词", self.state.skip_uppercase),
        ] {
            toolbar = toolbar.child(
                Button::new(format!("spelling-option-{id}"))
                    .icon(IconName::Check)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .label(label)
                    .ghost()
                    .when(value, |button| button.primary())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        let mut request = json!({"action":"configure"});
                        request[if id == "comments" {
                            "skip_comments"
                        } else {
                            "skip_uppercase"
                        }] = json!(!value);
                        view.spelling_request(request, cx);
                    })),
            );
        }
        toolbar = toolbar.child(
            Button::new("spelling-undo")
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .icon(IconName::Undo2)
                .accessibility_label("撤销")
                .tooltip("撤销上一次修改")
                .ghost()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                })),
        );
        let p = crate::workspace_theme::Palette::current(cx);
        let status = if !self.state.available {
            "没有可用的拼写词典。请在设置中配置词典目录并安装对应词典。"
        } else if self.state.complete {
            "本次拼写检查已完成。"
        } else if !self.state.found {
            "选择语言后点击“从当前行检查”开始；标签、绘图和被过滤的行会跳过。"
        } else if self.loading {
            "正在检查拼写…"
        } else {
            "找到一处需要确认的拼写。"
        };
        let mut page = crate::page::page(cx, "spelling-panel")
            .child(toolbar)
            .child(crate::page::section(
                cx,
                "状态",
                None,
                crate::page::section_body()
                    .child(crate::page::hint(cx, status))
                    .when(self.loading, |body| {
                        body.child(crate::page::hint(cx, "正在检查拼写…"))
                    }),
            ));
        if let Some(error) = &self.error {
            page = page.child(crate::page::error(cx, error.clone()));
        }
        let candidate = self.replacement.read(cx).value().to_string();
        let can_remove =
            self.dictionary_checked && self.dictionary_word == candidate && self.state.can_remove;
        let mut panel = page;
        if self.state.found {
            let mut word_body = crate::page::section_body()
                .child(div().text_base().font_semibold().child(format!(
                    "第 {} 行 · {}",
                    self.state.row + 1,
                    self.state.word
                )))
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .text_sm()
                        .gap_1()
                        .child(
                            self.state
                                .line_text
                                .get(..self.state.start)
                                .unwrap_or_default()
                                .to_owned(),
                        )
                        .child(
                            div()
                                .font_semibold()
                                .text_color(Theme::global(cx).danger)
                                .child(self.state.word.clone()),
                        )
                        .child(
                            self.state
                                .line_text
                                .get(self.state.start.saturating_add(self.state.length)..)
                                .unwrap_or_default()
                                .to_owned(),
                        ),
                )
                .child(crate::page::field(
                    cx,
                    "替换为",
                    Input::new(&self.replacement)
                        .w(px(380.))
                        .disabled(self.loading),
                ));
            let mut actions = div().h_flex().flex_wrap().gap_2();
            for (id, label, primary) in [
                ("replace", "替换并继续", true),
                ("replace_all", "替换本次全部同词", false),
                ("next", "忽略一次", false),
                ("ignore_all", "本次忽略同词", false),
                ("add", "加入个人词典", false),
            ] {
                actions = actions.child(
                    Button::new(format!("spelling-action-{id}"))
                        .icon(IconName::SpellCheck)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label(label)
                        .when(primary, |button| button.primary())
                        .disabled(self.loading || id == "add" && !self.state.can_add)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.spelling_request(json!({"action":id}), cx)
                        })),
                );
            }
            word_body = word_body
                .child(actions)
                .child(crate::page::field_label(cx, "建议"));
            let mut suggestions = div().h_flex().flex_wrap().gap_2();
            for (index, suggestion) in self.state.suggestions.iter().enumerate() {
                let value = suggestion.clone();
                suggestions = suggestions.child(
                    Button::new(format!("spelling-suggestion-{index}"))
                        .icon(IconName::Replace)
                        .rounded(gpui_kit::px(999.))
                        .small()
                        .px(gpui_kit::px(13.))
                        .py(gpui_kit::px(3.))
                        .label(value.clone())
                        .ghost()
                        .disabled(self.loading)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.spelling.pending = Some(value.clone());
                            view.spelling.confirm_remove = false;
                            cx.notify();
                        })),
                );
            }
            if self.state.suggestions.is_empty() {
                suggestions = suggestions.child(
                    div()
                        .text_sm()
                        .text_color(p.muted)
                        .child("没有建议，可以直接输入替换内容。"),
                );
            }
            word_body = word_body.child(suggestions);
            panel = panel.child(crate::page::section(cx, "当前单词", None, word_body));
        }
        panel.child(crate::page::section(
            cx,
            "个人词典",
            None,
            crate::page::section_body()
                .child(crate::page::hint(
                    cx,
                    "确认当前替换词是否已收录，或把它从个人词典移除。",
                ))
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .gap_2()
                        .items_center()
                        .child(
                            Button::new("spelling-dictionary-check")
                                .rounded(gpui_kit::px(999.))
                                .small()
                                .px(gpui_kit::px(13.))
                                .py(gpui_kit::px(3.))
                                .icon(IconName::BookOpen)
                                .label("检查替换词是否在个人词典")
                                .disabled(self.loading || candidate.is_empty())
                                .on_click(cx.listener(|view, _, _, cx| {
                                    let value =
                                        view.spelling.replacement.read(cx).value().to_string();
                                    view.spelling.dictionary_word = value.clone();
                                    view.spelling_request(
                                        json!({"action":"state","word":value}),
                                        cx,
                                    );
                                })),
                        )
                        .child(
                            Button::new("spelling-dictionary-remove")
                                .rounded(gpui_kit::px(999.))
                                .small()
                                .px(gpui_kit::px(13.))
                                .py(gpui_kit::px(3.))
                                .icon(IconName::Delete)
                                .label(if self.confirm_remove {
                                    "确认从个人词典移除"
                                } else {
                                    "从个人词典移除…"
                                })
                                .ghost()
                                .disabled(self.loading || !can_remove)
                                .on_click(cx.listener(|view, _, _, cx| {
                                    if view.spelling.confirm_remove {
                                        view.spelling_request(
                                    json!({"action":"remove","word":view.spelling.dictionary_word}),
                                    cx,
                                );
                                    } else {
                                        view.spelling.confirm_remove = true;
                                        cx.notify();
                                    }
                                })),
                        ),
                ),
        ))
    }
}
