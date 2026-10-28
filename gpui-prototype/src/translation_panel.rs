use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt, Theme,
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
        let mut options = div().h_flex().gap_2();
        options = options.child(
            Button::new("translation-seek")
                .label("定位视频预览")
                .when(self.seek, |button| button.primary())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.translation.seek = !view.translation.seek;
                    cx.notify();
                })),
        );
        options = options.child(
            Button::new("translation-skip-whitespace")
                .label("跳过空白块")
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
            options = options.child(Button::new(id).label(label).disabled(!enabled).on_click(
                cx.listener(move |view, _, _, cx| {
                    view.invoke(json!({"op":"command", "name":command}), cx)
                }),
            ));
        }
        let p = crate::workspace_theme::Palette::current(cx);
        let status = if self.state.available {
            format!(
                "行 {} / {} · 文本块 {} / {}{}",
                self.state.row + 1,
                self.state.line_count,
                self.state.position + 1,
                self.state.count,
                if self.state.exhausted {
                    " · 已到最后一块"
                } else {
                    ""
                }
            )
        } else {
            "当前脚本没有可翻译的文本块".to_owned()
        };
        crate::page::page(cx, "translation-assistant")
            .child(crate::page::header(
                cx,
                IconName::Languages,
                "翻译助手",
                "按块翻译：上一块 / 下一块切换，提交后自动前进；整行原文的标签与绘图保持不变。",
            ))
            .child(crate::page::toolbar([
                Button::new("translation-commit")
                    .label("提交并前进")
                    .primary()
                    .disabled(!self.state.available || self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.translation_action("commit", cx)))
                    .into_any_element(),
                Button::new("translation-preview")
                    .label("提交并预览")
                    .disabled(!self.state.available || self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.translation_action("preview", cx)))
                    .into_any_element(),
                Button::new("translation-prev")
                    .label("上一块")
                    .ghost()
                    .disabled(!self.state.available || self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.translation_action("prev", cx)))
                    .into_any_element(),
                Button::new("translation-next")
                    .label("下一块")
                    .ghost()
                    .disabled(!self.state.available || self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.translation_action("next", cx)))
                    .into_any_element(),
                Button::new("translation-original")
                    .label("插入原文")
                    .ghost()
                    .disabled(!self.state.available || self.loading)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.translation_action("insert_original", cx)
                    }))
                    .into_any_element(),
                Button::new("translation-refresh")
                    .label("从当前行刷新")
                    .ghost()
                    .disabled(self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.translation_action("start", cx)))
                    .into_any_element(),
                crate::page::badge(cx, status).into_any_element(),
            ]))
            .child(options)
            .child(crate::page::section(
                cx,
                "整行原文",
                None,
                crate::page::section_body()
                    .child(crate::page::hint(cx, "覆盖标签、注释块和绘图保持原样"))
                    .child(
                        div()
                            .p_3()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(p.border)
                            .text_sm()
                            .child(self.state.line_text.clone()),
                    ),
            ))
            .child(crate::page::section(
                cx,
                "当前文本块",
                None,
                crate::page::section_body().child(
                    div()
                        .p_3()
                        .rounded(px(8.))
                        .bg(p.selected)
                        .text_color(p.selected_text)
                        .child(self.state.text.clone()),
                ),
            ))
            .child(crate::page::section(
                cx,
                "译文",
                None,
                crate::page::section_body()
                    .child(Textarea::new(&self.editor).h(px(150.)).w_full())
                    .child(crate::page::hint(
                        cx,
                        "Enter 提交并前进，Shift+Enter 换行；F8 预览、PageUp/PageDown 切块。切块会清空当前译文；刷新可重新读取外部修改。",
                    )),
            ))
    }
}
