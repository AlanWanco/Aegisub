use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt, Theme,
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
        // Fields are multi-select toggles; grouping them by what they change and
        // marking the selected ones keeps eleven pills from reading as one blob.
        let groups: [(&str, &[(usize, &str)]); 3] = [
            ("时间", &[(2, "开始时间"), (3, "结束时间")]),
            (
                "内容",
                &[
                    (0, "注释/对白"),
                    (4, "样式"),
                    (5, "角色"),
                    (9, "效果"),
                    (10, "字幕文本"),
                ],
            ),
            (
                "排版",
                &[(1, "层"), (6, "左边距"), (7, "右边距"), (8, "垂直边距")],
            ),
        ];
        let mut field_groups = div().v_flex().w_full().min_w_0().gap_3();
        for (title, entries) in groups {
            let mut row = div().h_flex().flex_wrap().items_center().gap_2();
            for &(index, label) in entries {
                let on = self.fields[index];
                row = row.child(
                    Button::new(format!("paste-over-field-{index}"))
                        .label(if on {
                            format!("✓ {label}")
                        } else {
                            label.to_owned()
                        })
                        .when(on, |button| button.primary())
                        .disabled(disabled)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.paste.fields[index] = !view.paste.fields[index];
                            cx.notify();
                        })),
                );
            }
            field_groups = field_groups
                .child(crate::page::field_label(cx, title))
                .child(row);
        }
        let p = crate::workspace_theme::Palette::current(cx);
        let scope = if snapshot.selected.len() > 1 {
            format!("按顺序覆盖选中的 {} 行", snapshot.selected.len())
        } else {
            "从当前行向下逐行覆盖，直到剪贴板结束或文档末尾".to_owned()
        };
        let mut actions = div().h_flex().flex_wrap().items_center().gap_2();
        actions = actions
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
                    .ghost()
                    .on_click(cx.listener(|view, _, window, cx| {
                        view.active_panel = crate::Panel::Subtitles;
                        view.focus_handle.focus(window, cx);
                        cx.notify();
                    })),
            )
            .child(div().text_sm().text_color(p.muted).child(scope));
        let mut panel = crate::page::page(cx, "paste-panel")
            .child(crate::page::header(
                cx,
                IconName::ClipboardPaste,
                "覆盖粘贴",
                "从剪贴板读取字幕行，覆盖勾选的字段；其余字段保持当前值，整次操作可撤销。",
            ))
            .child(actions)
            .child(crate::page::section(
                cx,
                "预设",
                None,
                crate::page::section_body().child(presets),
            ))
            .child(crate::page::section(
                cx,
                "覆盖哪些字段",
                None,
                crate::page::section_body().child(field_groups),
            ));
        if let Some(error) = &self.error {
            panel = panel.child(crate::page::error(cx, error.clone()));
        }
        if !self.message.is_empty() {
            panel = panel.child(crate::page::hint(cx, self.message.clone()));
        }
        panel
    }
}
