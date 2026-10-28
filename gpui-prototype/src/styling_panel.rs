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
use serde_json::json;

use crate::{Snapshot, Workspace, document_panels::DocumentDetails};

pub struct StylingPanel {
    pub input: Entity<InputState>,
    pub seek: bool,
    pub line_id: i64,
    pub pending_value: Option<String>,
    pub focus_pending: bool,
    _events: Subscription,
}

impl StylingPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder("输入或选择样式"));
        let events = cx.subscribe(&input, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                view.styling_action("commit", None, cx);
            }
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            input,
            seek: true,
            line_id: -1,
            pending_value: None,
            focus_pending: false,
            _events: events,
        }
    }

    pub fn sync(&mut self, snapshot: &Snapshot, window: &mut Window, cx: &mut App) {
        let active = snapshot
            .lines
            .iter()
            .find(|line| line.id == snapshot.active);
        if let Some(line) = active {
            if self.line_id != line.id || self.pending_value.is_some() {
                let value = self
                    .pending_value
                    .take()
                    .unwrap_or_else(|| line.style.clone());
                self.input
                    .update(cx, |state, cx| state.set_value(value, window, cx));
                self.line_id = line.id;
            }
        }
        if self.focus_pending {
            self.focus_pending = false;
            self.input.update(cx, |state, cx| state.focus(window, cx));
        }
    }

    pub fn render(
        &self,
        snapshot: &Snapshot,
        details: &DocumentDetails,
        cx: &mut Context<Workspace>,
    ) -> impl IntoElement {
        let active = snapshot
            .lines
            .iter()
            .find(|line| line.id == snapshot.active);
        let typed = self.input.read(cx).value().to_lowercase();
        let valid = details.styles.iter().any(|style| {
            style
                .get("name")
                .is_some_and(|name| name.eq_ignore_ascii_case(&typed))
        });
        let mut styles = div()
            .id("styling-style-list")
            .v_flex()
            .w(px(275.))
            .gap_1()
            .overflow_y_scrollbar();
        for style in &details.styles {
            let Some(name) = style.get("name") else {
                continue;
            };
            if !typed.is_empty() && !name.to_lowercase().starts_with(&typed) {
                continue;
            }
            let name = name.clone();
            styles = styles.child(
                Button::new(format!("styling-select-{name}"))
                    .label(name.clone())
                    .w_full()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.styling_action("preview", Some(name.clone()), cx)
                    })),
            );
        }
        let mut actions = div().h_flex().flex_wrap().gap_2();
        for (id, label, action) in [
            ("styling-prev", "上一行", "prev"),
            ("styling-next", "下一行", "next"),
            ("styling-preview", "应用并预览", "preview"),
            ("styling-commit", "应用并前进", "commit"),
        ] {
            actions = actions.child(
                Button::new(id)
                    .label(label)
                    .disabled(active.is_none())
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.styling_action(action, None, cx)),
                    ),
            );
        }
        actions = actions.child(
            Button::new("styling-seek")
                .label("定位当前行视频")
                .when(self.seek, |button| button.primary())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.styling.seek = !view.styling.seek;
                    cx.notify();
                })),
        );
        for (id, label, command, enabled) in [
            (
                "styling-play-video",
                "播放视频",
                "video/play/line",
                snapshot.video,
            ),
            (
                "styling-play-audio",
                "播放音频",
                "audio/play/selection",
                snapshot.audio.loaded,
            ),
            ("styling-undo", "撤销", "edit/undo", true),
        ] {
            actions = actions.child(Button::new(id).label(label).disabled(!enabled).on_click(
                cx.listener(move |view, _, _, cx| {
                    view.invoke(json!({"op":"command","name":command}), cx)
                }),
            ));
        }
        div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .child(div().text_lg().font_semibold().child("样式助手"))
            .child(
                div().text_sm().child(
                    active
                        .map(|line| {
                            format!(
                                "行 {} / {} · 当前样式 {}",
                                line.row + 1,
                                snapshot.lines.len(),
                                line.style
                            )
                        })
                        .unwrap_or_default(),
                ),
            )
            .child(
                div()
                    .p_3()
                    .border_1()
                    .border_color(Theme::global(cx).border)
                    .child(active.map(|line| line.text.clone()).unwrap_or_default()),
            )
            .child(actions)
            .child(Input::new(&self.input).w(px(320.)))
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(if valid {
                        "Enter 应用并前进；F8 预览，PageUp/PageDown 切行。"
                    } else {
                        "按输入前缀筛选样式，点击样式应用到当前行。"
                    }),
            )
            .child(styles.flex_1())
    }
}
