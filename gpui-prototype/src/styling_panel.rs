use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};

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
        let current_style = active.map(|line| line.style.to_lowercase());
        let mut matched = 0usize;
        let mut styles = div().v_flex().w_full().min_w_0().gap_1();
        for style in &details.styles {
            let Some(name) = style.get("name") else {
                continue;
            };
            if !typed.is_empty() && !name.to_lowercase().starts_with(&typed) {
                continue;
            }
            matched += 1;
            let name = name.clone();
            let selected = current_style
                .as_deref()
                .is_some_and(|current| current == name.to_lowercase());
            styles = styles.child(
                crate::page::list_row(
                    cx,
                    format!("styling-select-{name}"),
                    name.clone(),
                    selected,
                    false,
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.styling_action("preview", Some(name.clone()), cx)
                })),
            );
        }
        let p = crate::workspace_theme::Palette::current(cx);
        let status = active
            .map(|line| {
                format!(
                    "行 {} / {} · 当前样式 {}",
                    line.row + 1,
                    snapshot.lines.len(),
                    line.style
                )
            })
            .unwrap_or_default();
        crate::page::page(cx, "styling-panel")
            .child(crate::page::toolbar([
                Button::new("styling-commit")
                    .icon(IconName::Check)
                    .rounded(gpui_kit::px(999.))
                    .label("应用并前进")
                    .primary()
                    .disabled(active.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.styling_action("commit", None, cx)))
                    .into_any_element(),
                Button::new("styling-preview")
                    .icon(IconName::Eye)
                    .rounded(gpui_kit::px(999.))
                    .label("应用并预览")
                    .disabled(active.is_none())
                    .on_click(
                        cx.listener(|view, _, _, cx| view.styling_action("preview", None, cx)),
                    )
                    .into_any_element(),
                Button::new("styling-prev")
                    .icon(IconName::ChevronLeft)
                    .rounded(gpui_kit::px(999.))
                    .label("上一行")
                    .ghost()
                    .disabled(active.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.styling_action("prev", None, cx)))
                    .into_any_element(),
                Button::new("styling-next")
                    .icon(IconName::ChevronRight)
                    .rounded(gpui_kit::px(999.))
                    .label("下一行")
                    .ghost()
                    .disabled(active.is_none())
                    .on_click(cx.listener(|view, _, _, cx| view.styling_action("next", None, cx)))
                    .into_any_element(),
                crate::page::badge(cx, status).into_any_element(),
            ]))
            .child(crate::page::section(
                cx,
                "当前行",
                None,
                crate::page::section_body().child(
                    div()
                        .p_3()
                        .rounded(px(crate::workspace_theme::RADIUS_MD))
                        .border_1()
                        .border_color(p.border)
                        .text_sm()
                        .child(active.map(|line| line.text.clone()).unwrap_or_default()),
                ),
            ))
            .child(crate::page::section(
                cx,
                "选择样式",
                Some(crate::page::badge(cx, format!("{matched} 个匹配")).into_any_element()),
                crate::page::section_body()
                    .child(crate::page::field(
                        cx,
                        "样式名称 · 输入前缀可筛选，回车应用并前进",
                        Input::new(&self.input).w_full().max_w(px(420.)),
                    ))
                    .when(matched == 0, |body| {
                        body.child(crate::page::hint(cx, "没有匹配的样式。"))
                    })
                    .child(styles)
                    .child(crate::page::hint(
                        cx,
                        if valid {
                            "Enter 应用并前进；F8 预览，PageUp/PageDown 切行。"
                        } else {
                            "点击下方样式可预览应用到当前行。"
                        },
                    )),
            ))
    }
}
