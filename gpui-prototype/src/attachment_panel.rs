use std::collections::BTreeSet;

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Icon, StyledExt, Theme,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
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
        // One add action, then the selection actions, then the quiet utilities:
        // six identical pills made the destructive one look like the others.
        let mut toolbar = div()
            .h_flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                Button::new("attachment-add")
                    .label("添加附件…")
                    .primary()
                    .disabled(self.loading)
                    .dropdown_menu({
                        let weak = cx.weak_entity();
                        move |mut menu, _, _| {
                            for (label, kind) in
                                [("添加字体…", "font"), ("添加图像…", "graphic")]
                            {
                                let weak = weak.clone();
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, _, cx| {
                                        let _ = weak.update(cx, |view, cx| {
                                            view.pick_attachments(kind, cx)
                                        });
                                    },
                                ));
                            }
                            menu
                        }
                    })
                    .into_any_element(),
            )
            .child(
                Button::new("attachment-extract")
                    .label("导出所选…")
                    .disabled(self.loading || self.selected.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| view.extract_attachments(cx)))
                    .into_any_element(),
            )
            .child(
                Button::new("attachment-delete")
                    .label(if self.confirm_delete {
                        "确认删除"
                    } else {
                        "删除所选…"
                    })
                    .when(self.confirm_delete, |button| button.danger())
                    .disabled(self.loading || self.selected.is_empty())
                    .on_click(cx.listener(|view, _, _, cx| {
                        if view.attachments.confirm_delete {
                            let items = view.attachments.selected_items();
                            view.attachment_request(
                                json!({"action":"delete","items":items,"revision":view.attachments.state.revision}),
                                cx,
                            );
                        } else {
                            view.attachments.confirm_delete = true;
                            cx.notify();
                        }
                    }))
                    .into_any_element(),
            );
        if self.confirm_delete {
            toolbar = toolbar.child(
                Button::new("attachment-cancel-delete")
                    .label("取消删除")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        toolbar = toolbar.child(
            div()
                .ml_auto()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("attachment-undo")
                        .label("撤销")
                        .ghost()
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"command","name":"edit/undo"}), cx)
                        })),
                )
                .child(
                    Button::new("attachment-refresh")
                        .label("刷新")
                        .ghost()
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.attachment_request(json!({"action":"state"}), cx)
                        })),
                ),
        );
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
            let p = crate::workspace_theme::Palette::current(cx);
            list = list.child(
                div()
                    .id(format!("attachment-row-{index}"))
                    .h_flex()
                    .w_full()
                    .min_w_0()
                    .h(px(38.))
                    .gap_3()
                    .items_center()
                    .px_3()
                    .rounded(px(6.))
                    .border_l_2()
                    .border_color(if selected {
                        p.accent
                    } else {
                        gpui_kit::rgba(0x00000000).into()
                    })
                    .when(selected, |row| row.bg(p.selected))
                    .when(!selected && !self.loading, |row| {
                        row.hover(|row| row.bg(p.raised))
                    })
                    .when(self.loading, |row| row.opacity(0.5))
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.attachments.selected.remove(&index) {
                            view.attachments.selected.insert(index);
                        }
                        view.attachments.confirm_delete = false;
                        cx.notify();
                    }))
                    .child(
                        Icon::new(if attachment.group == "font" {
                            IconName::CaseSensitive
                        } else {
                            IconName::Frame
                        })
                        .size(px(14.))
                        .text_color(if selected {
                            p.accent
                        } else {
                            p.muted
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(if selected { p.selected_text } else { p.text })
                            .child(attachment.name.clone()),
                    )
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_xs()
                            .text_color(if selected { p.selected_text } else { p.muted })
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
                "附件列表",
                Some(
                    crate::page::badge(
                        cx,
                        format!(
                            "{} 个 · 已选 {}",
                            self.state.items.len(),
                            self.selected.len()
                        ),
                    )
                    .into_any_element(),
                ),
                crate::page::section_body().child(list),
            ));
        if let Some(error) = &self.error {
            panel = panel.child(crate::page::error(cx, error.clone()));
        }
        panel
    }
}
