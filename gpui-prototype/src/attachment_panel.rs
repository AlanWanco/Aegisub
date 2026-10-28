use std::collections::BTreeSet;

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Icon, StyledExt, Theme,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};
use gpui_kit::{Context, IntoElement, div, prelude::*, px};
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
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| view.pick_attachments(kind, cx))),
            );
        }
        toolbar = toolbar
            .child(Button::new("attachment-extract").label("导出所选附件…").disabled(self.loading || self.selected.is_empty()).on_click(cx.listener(|view, _, _, cx| view.extract_attachments(cx))))
            .child(Button::new("attachment-delete").label(if self.confirm_delete { "确认删除所选附件" } else { "删除所选…" }).disabled(self.loading || self.selected.is_empty()).on_click(cx.listener(|view, _, _, cx| {
                if view.attachments.confirm_delete {
                    let items = view.attachments.selected_items();
                    view.attachment_request(json!({"action":"delete","items":items,"revision":view.attachments.state.revision}), cx);
                } else { view.attachments.confirm_delete = true; cx.notify(); }
            })))
            .child(Button::new("attachment-refresh").label("刷新").disabled(self.loading).on_click(cx.listener(|view, _, _, cx| view.attachment_request(json!({"action":"state"}), cx))))
            .child(Button::new("attachment-undo").label("撤销").disabled(self.loading).on_click(cx.listener(|view, _, _, cx| view.invoke(json!({"op":"command","name":"edit/undo"}), cx))));
        if self.confirm_delete {
            toolbar = toolbar.child(
                Button::new("attachment-cancel-delete")
                    .label("取消删除")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    })),
            );
        }
        let mut list = div().v_flex().w_full().min_w_0().gap_1();
        if self.state.items.is_empty() && !self.loading {
            list = list.child(
                div()
                    .v_flex()
                    .items_center()
                    .gap_2()
                    .py_8()
                    .child(
                        Icon::new(IconName::Paperclip)
                            .size(px(28.))
                            .text_color(Theme::global(cx).muted_foreground),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(Theme::global(cx).muted_foreground)
                            .child("当前字幕没有附件"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(Theme::global(cx).muted_foreground)
                            .child("可用上方按钮添加字体或图像，随 ASS 文件一起保存。"),
                    ),
            );
        }
        for attachment in &self.state.items {
            let index = attachment.index;
            let selected = self.selected.contains(&index);
            list = list.child(
                Button::new(format!("attachment-row-{index}"))
                    .ghost()
                    .h(px(34.))
                    .w_full()
                    .accessibility_label(attachment.name.clone())
                    .when(selected, |button| button.primary())
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.attachments.selected.remove(&index) {
                            view.attachments.selected.insert(index);
                        }
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .h_flex()
                            .w_full()
                            .min_w_0()
                            .gap_3()
                            .items_center()
                            .child(
                                Icon::new(if attachment.group == "font" {
                                    IconName::CaseSensitive
                                } else {
                                    IconName::Frame
                                })
                                .size(px(14.))
                                .text_color(Theme::global(cx).muted_foreground),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .text_left()
                                    .child(attachment.name.clone()),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .text_xs()
                                    .text_color(Theme::global(cx).muted_foreground)
                                    .child(format!(
                                        "{} · {} 字节",
                                        if attachment.group == "font" {
                                            "字体"
                                        } else {
                                            "图像"
                                        },
                                        attachment.encoded_size
                                    )),
                            ),
                    ),
            );
        }
        if self.loading {
            list = list.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("正在处理附件…"),
            );
        }
        let mut panel = crate::page::page(cx, "attachment-panel")
            .child(crate::page::header(
                cx,
                IconName::Paperclip,
                "附件管理",
                "附件嵌入 ASS 文件，添加和删除可撤销；列表大小为 ASS 编码后的数据量。",
            ))
            .child(toolbar)
            .child(crate::page::section(
                cx,
                format!(
                    "附件 · {} 个，已选 {} 个",
                    self.state.items.len(),
                    self.selected.len()
                ),
                None,
                crate::page::section_body().child(list),
            ));
        if let Some(error) = &self.error {
            panel = panel.child(crate::page::error(cx, error.clone()));
        }
        panel
    }
}
