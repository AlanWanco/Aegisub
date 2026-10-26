use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
};
use gpui_kit::{Context, IntoElement, div, prelude::*};
use serde::Deserialize;

use crate::{Snapshot, Workspace};

#[derive(Default, Deserialize)]
pub struct PasteReply {
    #[serde(default)]
    pub fields: Vec<bool>,
    pub changed: Option<usize>,
}

#[derive(Default)]
pub struct PastePanel {
    pub fields: [bool; 11],
    pub initialized: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub message: String,
}

impl PastePanel {
    pub fn render(&self, snapshot: &Snapshot, cx: &mut Context<Workspace>) -> impl IntoElement {
        let disabled = self.loading || !self.initialized;
        let mut presets = div().h_flex().flex_wrap().gap_2();
        for (preset, label) in [
            (0, "全部字段"),
            (1, "清除选择"),
            (2, "仅时间"),
            (3, "仅文本"),
        ] {
            presets = presets.child(
                Button::new(format!("paste-over-preset-{preset}"))
                    .label(label)
                    .small()
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.paste.fields = [preset == 0; 11];
                        if preset == 2 {
                            view.paste.fields[2] = true;
                            view.paste.fields[3] = true;
                        }
                        if preset == 3 {
                            view.paste.fields[10] = true;
                        }
                        cx.notify();
                    })),
            );
        }
        let mut fields = div().h_flex().flex_wrap().gap_2();
        for (index, label) in [
            "注释/对白",
            "层",
            "开始时间",
            "结束时间",
            "样式",
            "角色",
            "左边距",
            "右边距",
            "垂直边距",
            "效果",
            "字幕文本",
        ]
        .into_iter()
        .enumerate()
        {
            fields = fields.child(
                Button::new(format!("paste-over-field-{index}"))
                    .label(label)
                    .small()
                    .when(self.fields[index], |button| button.primary())
                    .disabled(disabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.paste.fields[index] = !view.paste.fields[index];
                        cx.notify();
                    })),
            );
        }
        let mut panel = div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_4()
            .child(div().text_lg().font_semibold().child("选择字段覆盖粘贴"))
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(
                        "从剪贴板读取字幕行，覆盖勾选字段；其余字段保持当前值。整次操作可撤销。",
                    ),
            )
            .child(presets)
            .child(fields)
            .child(if snapshot.selected.len() > 1 {
                format!("按顺序覆盖选中的 {} 行。", snapshot.selected.len())
            } else {
                "从当前行向下逐行覆盖，直到剪贴板结束或文档末尾。".into()
            })
            .child(
                Button::new("paste-over-apply")
                    .label(if self.loading {
                        "处理中…"
                    } else {
                        "应用覆盖粘贴"
                    })
                    .primary()
                    .disabled(
                        disabled
                            || snapshot.selected.is_empty()
                            || !self.fields.iter().any(|field| *field),
                    )
                    .on_click(cx.listener(|view, _, _, cx| view.paste_request(true, cx))),
            )
            .child(
                Button::new("paste-over-back")
                    .label("返回字幕")
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.active_panel = crate::Panel::Subtitles;
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
            );
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        panel.child(self.message.clone())
    }
}
