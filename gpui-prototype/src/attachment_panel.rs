use std::collections::BTreeSet;

use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};
use gpui_kit::{Context, IntoElement, div, prelude::*};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Workspace;

#[derive(Clone, PartialEq, Deserialize)]
pub struct Attachment {
    pub index: usize,
    pub name: String,
    pub filename: String,
    pub encoded_size: u64,
    pub group: String,
    pub safe_name: bool,
}

#[derive(Default, Deserialize)]
pub struct AttachmentState {
    pub items: Vec<Attachment>,
    pub revision: u64,
}

#[derive(Default)]
pub struct AttachmentPanel {
    pub state: AttachmentState,
    pub selected: BTreeSet<usize>,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_delete: bool,
}

impl AttachmentPanel {
    pub fn selected_items(&self) -> Vec<Value> {
        self.state
            .items
            .iter()
            .filter(|item| self.selected.contains(&item.index))
            .map(|item| json!({"index":item.index,"name":item.name}))
            .collect()
    }

    pub fn receive(&mut self, state: AttachmentState) {
        if state.items != self.state.items {
            self.selected.clear();
            self.confirm_delete = false;
        }
        self.state = state;
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let mut toolbar = div().h_flex().flex_wrap().gap_2();
        for (id, label, kind) in [
            ("font", "添加字体…", "font"),
            ("graphic", "添加图像…", "graphic"),
        ] {
            toolbar = toolbar.child(
                Button::new(format!("attachment-add-{id}"))
                    .label(label)
                    .small()
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| view.pick_attachments(kind, cx))),
            );
        }
        toolbar = toolbar
            .child(Button::new("attachment-extract").label("导出所选附件…").small().disabled(self.loading || self.selected.is_empty()).on_click(cx.listener(|view, _, _, cx| view.extract_attachments(cx))))
            .child(Button::new("attachment-delete").label(if self.confirm_delete { "确认删除所选附件" } else { "删除所选…" }).small().disabled(self.loading || self.selected.is_empty()).on_click(cx.listener(|view, _, _, cx| {
                if view.attachments.confirm_delete {
                    let items = view.attachments.selected_items();
                    view.attachment_request(json!({"action":"delete","items":items,"revision":view.attachments.state.revision}), cx);
                } else { view.attachments.confirm_delete = true; cx.notify(); }
            })))
            .child(Button::new("attachment-refresh").label("刷新").small().disabled(self.loading).on_click(cx.listener(|view, _, _, cx| view.attachment_request(json!({"action":"state"}), cx))))
            .child(Button::new("attachment-undo").label("撤销").small().disabled(self.loading).on_click(cx.listener(|view, _, _, cx| view.invoke(json!({"op":"command","name":"edit/undo"}), cx))));
        if self.confirm_delete {
            toolbar = toolbar.child(
                Button::new("attachment-cancel-delete")
                    .label("取消删除")
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    })),
            );
        }
        let mut panel = div()
            .id("attachment-panel")
            .v_flex()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_3()
            .overflow_y_scrollbar()
            .child(div().text_lg().font_semibold().child("附件管理"))
            .child(toolbar)
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("附件嵌入 ASS 文件，添加和删除可撤销。列表大小为 ASS 编码后的数据量。"),
            )
            .child(format!(
                "附件 {} 个 · 已选 {} 个",
                self.state.items.len(),
                self.selected.len()
            ));
        if self.state.items.is_empty() && !self.loading {
            panel = panel.child("当前字幕没有附件，可添加字体或图像。");
        }
        for attachment in &self.state.items {
            let index = attachment.index;
            let selected = self.selected.contains(&index);
            panel = panel.child(
                Button::new(format!("attachment-row-{index}"))
                    .label(format!(
                        "{} · {} · {} 字节",
                        attachment.name,
                        if attachment.group == "font" {
                            "字体"
                        } else {
                            "图像"
                        },
                        attachment.encoded_size
                    ))
                    .small()
                    .when(selected, |button| button.primary())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.attachments.selected.remove(&index) {
                            view.attachments.selected.insert(index);
                        }
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    })),
            );
        }
        if self.loading {
            panel = panel.child("正在处理附件…");
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        panel
    }
}
