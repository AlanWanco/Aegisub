use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{InputEvent, Textarea, TextareaState},
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::Workspace;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TranslationState {
    pub available: bool,
    #[serde(default)]
    pub exhausted: bool,
    #[serde(default)]
    pub boundary: bool,
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub row: usize,
    #[serde(default)]
    pub block: usize,
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub line_text: String,
    #[serde(default)]
    pub position: usize,
    #[serde(default)]
    pub count: usize,
    #[serde(default)]
    pub line_count: usize,
}

pub struct TranslationPanel {
    pub state: TranslationState,
    pub editor: Entity<TextareaState>,
    pub loading: bool,
    pub seek: bool,
    pub skip_whitespace: bool,
    pub pending_text: Option<String>,
    pub focus_pending: bool,
    _events: Subscription,
}

impl TranslationPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let editor = cx.new(|cx| TextareaState::new(window, cx).submit_on_enter(true));
        let events = cx.subscribe(&editor, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { shift: false, .. }) {
                view.translation_action("commit", cx);
            }
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            state: TranslationState::default(),
            editor,
            loading: false,
            seek: true,
            skip_whitespace: true,
            pending_text: None,
            focus_pending: false,
            _events: events,
        }
    }

    pub fn render(
        &self,
        video: bool,
        audio: bool,
        cx: &mut Context<Workspace>,
    ) -> impl IntoElement {
        let mut actions = div().h_flex().flex_wrap().gap_2();
        for (id, label, action) in [
            ("translation-prev", "上一块", "prev"),
            ("translation-next", "下一块", "next"),
            ("translation-preview", "提交并预览", "preview"),
            ("translation-commit", "提交并前进", "commit"),
            ("translation-original", "插入原文", "insert_original"),
        ] {
            actions = actions.child(
                Button::new(id)
                    .label(label)
                    .small()
                    .disabled(!self.state.available || self.loading)
                    .on_click(
                        cx.listener(move |view, _, _, cx| view.translation_action(action, cx)),
                    ),
            );
        }
        actions = actions.child(
            Button::new("translation-refresh")
                .label("从当前行刷新")
                .small()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| view.translation_action("start", cx))),
        );
        let mut options = div().h_flex().gap_2();
        options = options.child(
            Button::new("translation-seek")
                .label("定位视频预览")
                .small()
                .when(self.seek, |button| button.primary())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.translation.seek = !view.translation.seek;
                    cx.notify();
                })),
        );
        options = options.child(
            Button::new("translation-skip-whitespace")
                .label("跳过空白块")
                .small()
                .when(self.skip_whitespace, |button| button.primary())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.translation.skip_whitespace = !view.translation.skip_whitespace;
                    view.translation_action("start", cx);
                })),
        );
        for (id, label, command, enabled) in [
            (
                "translation-video",
                "播放当前行视频",
                "video/play/line",
                video,
            ),
            (
                "translation-audio",
                "播放当前行音频",
                "audio/play/selection",
                audio,
            ),
            ("translation-undo", "撤销", "edit/undo", true),
        ] {
            options = options.child(
                Button::new(id)
                    .label(label)
                    .small()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":command}), cx)
                    })),
            );
        }
        div().id("translation-assistant").v_flex().flex_1().min_h_0().p_4().gap_3()
            .child(div().text_lg().font_semibold().child("翻译助手"))
            .child(div().text_sm().child(if self.state.available {
                format!("行 {} / {} · 文本块 {} / {}{}", self.state.row + 1, self.state.line_count,
                    self.state.position + 1, self.state.count, if self.state.exhausted { " · 已到最后一块" } else { "" })
            } else { "当前脚本没有可翻译的文本块".into() }))
            .child(actions).child(options)
            .child(div().text_sm().text_color(Theme::global(cx).muted_foreground).child("整行原文 · 覆盖标签、注释块和绘图保持原样"))
            .child(div().p_3().border_1().border_color(Theme::global(cx).border).child(self.state.line_text.clone()))
            .child(div().text_sm().font_semibold().child("当前文本块"))
            .child(div().p_3().bg(Theme::global(cx).accent).text_color(Theme::global(cx).accent_foreground).child(self.state.text.clone()))
            .child(div().text_sm().font_semibold().child("译文"))
            .child(Textarea::new(&self.editor).h(px(150.)).w_full())
            .child(div().text_xs().text_color(Theme::global(cx).muted_foreground).child("Enter 提交并前进，Shift+Enter 换行；F8 预览、PageUp/PageDown 切块。切块会清空当前译文；刷新可重新读取外部修改。"))
    }
}
