//! Shared page scaffolding.
//!
//! Every tool page uses the same rhythm so the workspace reads as one
//! application instead of a pile of hand-placed buttons:
//!
//! * a page header (title + one-line description),
//! * a toolbar row holding the page's actions, with a single primary action,
//! * sections rendered as bordered cards with their own header,
//! * consistent 8/12/16/24 spacing.

use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Icon, StyledExt, Theme,
    button::{Button, ButtonVariants},
    scroll::{Scrollable, ScrollableElement},
};
use gpui_kit::{IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::workspace_theme::Palette;

/// Outer scroll container for a tool page.
pub fn page(cx: &gpui_kit::App, id: &'static str) -> Scrollable<gpui_kit::Stateful<gpui_kit::Div>> {
    div()
        .id(id)
        .v_flex()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .gap_4()
        .p_5()
        .overflow_y_scrollbar()
        .bg(Palette::current(cx).canvas)
}

/// Title plus a muted one-line description of what the page does.
pub fn header(
    cx: &gpui_kit::App,
    icon: IconName,
    title: impl Into<gpui_kit::SharedString>,
    description: impl Into<gpui_kit::SharedString>,
) -> gpui_kit::Div {
    let p = Palette::current(cx);
    div()
        .v_flex()
        .gap_1()
        .child(
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(Icon::new(icon).size(px(18.)).text_color(p.accent))
                .child(div().text_lg().font_semibold().child(title.into())),
        )
        .child(
            div()
                .text_sm()
                .text_color(p.muted)
                .child(description.into()),
        )
}

/// A row of page actions. Pass one primary action first; the rest should be
/// secondary or ghost so the page has a single obvious next step.
pub fn toolbar(children: impl IntoIterator<Item = gpui_kit::AnyElement>) -> gpui_kit::Div {
    div()
        .h_flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .children(children)
}

/// Bordered card with a header row (title, optional trailing content).
pub fn section(
    cx: &gpui_kit::App,
    title: impl Into<gpui_kit::SharedString>,
    trailing: Option<gpui_kit::AnyElement>,
    body: gpui_kit::Div,
) -> gpui_kit::Div {
    section_with_icon(cx, None, title, trailing, body)
}

/// Section card whose header carries a small icon as a visual anchor.
pub fn section_with_icon(
    cx: &gpui_kit::App,
    icon: Option<IconName>,
    title: impl Into<gpui_kit::SharedString>,
    trailing: Option<gpui_kit::AnyElement>,
    body: gpui_kit::Div,
) -> gpui_kit::Div {
    let p = Palette::current(cx);
    let mut head = div()
        .h_flex()
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(p.divider)
        .bg(p.surface)
        .child({
            let mut row = div().h_flex().items_center().gap_2();
            if let Some(icon) = icon {
                row = row.child(Icon::new(icon).size(px(14.)).text_color(p.accent));
            }
            row.child(div().text_sm().font_semibold().child(title.into()))
        });
    if let Some(trailing) = trailing {
        head = head.child(trailing);
    }
    div()
        .v_flex()
        .w_full()
        .min_w_0()
        .overflow_hidden()
        .rounded(px(10.))
        .border_1()
        .border_color(p.border)
        .bg(p.raised)
        .child(head)
        .child(body)
}

/// Body of a section: padded column with a comfortable gap.
pub fn section_body() -> gpui_kit::Div {
    div().v_flex().w_full().min_w_0().gap_3().p_4()
}

/// Muted helper text.
pub fn hint(cx: &gpui_kit::App, text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
    div()
        .text_sm()
        .text_color(Palette::current(cx).muted)
        .child(text.into())
}

/// Label above a control.
pub fn field_label(cx: &gpui_kit::App, text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
    div()
        .text_sm()
        .text_color(Palette::current(cx).muted)
        .child(text.into())
}

/// One labelled control.
pub fn field(
    cx: &gpui_kit::App,
    label: impl Into<gpui_kit::SharedString>,
    control: impl IntoElement,
) -> gpui_kit::Div {
    // Full width, but never vertically greedy: a `flex_1` here would stretch
    // fields down the whole page inside a column container.
    div()
        .v_flex()
        .w_full()
        .min_w_0()
        .gap_1()
        .child(field_label(cx, label))
        .child(control)
}

/// Row of fields sharing the width equally.
pub fn field_row(children: impl IntoIterator<Item = gpui_kit::AnyElement>) -> gpui_kit::Div {
    div()
        .h_flex()
        .items_start()
        .w_full()
        .min_w_0()
        .gap_3()
        .children(children)
}

/// Cell inside [`field_row`]; gives the field an equal share of the row.
pub fn field_cell(field: gpui_kit::Div) -> gpui_kit::AnyElement {
    div().flex_1().min_w_0().child(field).into_any_element()
}

/// Error text placed directly under the control it belongs to.
pub fn error(cx: &gpui_kit::App, text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
    div()
        .text_sm()
        .text_color(Theme::global(cx).danger)
        .child(text.into())
}

/// Small pill used for counts and states in section headers.
pub fn badge(cx: &gpui_kit::App, text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
    let p = Palette::current(cx);
    div()
        .rounded_full()
        .px_2()
        .py_0p5()
        .text_xs()
        .text_color(p.muted)
        .bg(p.inset)
        .child(text.into())
}

/// Mutually exclusive options rendered as one pill group, so the current
/// choice is obvious and the row reads as a single control.
pub fn segmented(
    cx: &mut gpui_kit::Context<crate::Workspace>,
    segments: Vec<(String, &'static str, bool, bool)>,
    on_select: impl Fn(
        &mut crate::Workspace,
        usize,
        &mut gpui_kit::Window,
        &mut gpui_kit::Context<crate::Workspace>,
    ) + 'static,
) -> gpui_kit::Div {
    let handler = std::rc::Rc::new(on_select);
    let p = Palette::current(cx);
    // The track hugs its items: a full-width inset bar reads as an empty field.
    let mut row = div()
        .h_flex()
        .items_center()
        .gap_0p5()
        .p_0p5()
        .rounded(px(8.))
        .bg(p.inset);
    for (index, (id, label, selected, disabled)) in segments.into_iter().enumerate() {
        let handler = handler.clone();
        row = row.child(
            div()
                .id(id)
                .h(px(26.))
                .px_3()
                .h_flex()
                .items_center()
                .rounded(px(6.))
                .text_sm()
                .whitespace_nowrap()
                .role(gpui_kit::Role::Button)
                .aria_label(label)
                .when(selected, |item| item.bg(p.accent).text_color(p.canvas))
                .when(!selected && !disabled, |item| {
                    item.text_color(p.text).hover(|item| item.bg(p.raised))
                })
                .when(disabled, |item| item.text_color(p.muted).opacity(0.5))
                .when(!disabled, |item| item.cursor_pointer())
                .on_click(cx.listener(move |view, _, window, cx| handler(view, index, window, cx)))
                .child(label),
        );
    }
    // Not full width: a wrapping row of controls must be able to sit beside it.
    div().h_flex().flex_shrink_0().child(row)
}

/// Horizontal tab bar with an underline for the active tab. Use it when the
/// options switch the page's content instead of setting a value.
pub fn tabs(
    cx: &mut gpui_kit::Context<crate::Workspace>,
    items: Vec<(String, String, bool)>,
    on_select: impl Fn(
        &mut crate::Workspace,
        usize,
        &mut gpui_kit::Window,
        &mut gpui_kit::Context<crate::Workspace>,
    ) + 'static,
) -> gpui_kit::Div {
    let handler = std::rc::Rc::new(on_select);
    let p = Palette::current(cx);
    let mut bar = div()
        .h_flex()
        .flex_wrap()
        .items_end()
        .w_full()
        .min_w_0()
        .gap_1()
        .border_b_1()
        .border_color(p.divider);
    for (index, (id, label, active)) in items.into_iter().enumerate() {
        let handler = handler.clone();
        let mut tab = div().v_flex().items_center().gap_1();
        tab = tab.child(
            Button::new(id)
                .label(label)
                .ghost()
                .text_color(if active { p.text } else { p.muted })
                .when(active, |button| button.font_semibold())
                .on_click(cx.listener(move |view, _, window, cx| handler(view, index, window, cx))),
        );
        if active {
            tab = tab.child(div().h(px(2.)).w_full().rounded(px(1.)).bg(p.accent));
        }
        bar = bar.child(tab);
    }
    bar
}

/// Selectable row for lists. Selection is a tint plus a left accent bar, not a
/// filled button: a list of filled pills makes every entry look like an action.
pub fn list_row(
    cx: &gpui_kit::App,
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    selected: bool,
    disabled: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = Palette::current(cx);
    div()
        .id(id)
        .h_flex()
        .w_full()
        .min_w_0()
        .h(px(34.))
        .gap_2()
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
        .when(!selected && !disabled, |row| {
            row.hover(|row| row.bg(p.raised))
        })
        .text_sm()
        .text_color(if selected { p.selected_text } else { p.text })
        .when(disabled, |row| row.opacity(0.5))
        .cursor_pointer()
        .child(div().truncate().child(label.into()))
}

/// Multi-select chip. Chips are for picking things (styles, fields, flags);
/// buttons are for acting. Keeping the two visually distinct stops a row of
/// identical pills from hiding which one is the page's real action.
pub fn toggle_chip(
    cx: &gpui_kit::App,
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    on: bool,
    disabled: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = Palette::current(cx);
    div()
        .id(id)
        .h(px(28.))
        .px_3()
        .h_flex()
        .items_center()
        .rounded_full()
        .border_1()
        .text_sm()
        .whitespace_nowrap()
        .when(on, |chip| {
            chip.bg(p.accent)
                .border_color(p.accent)
                .text_color(p.canvas)
        })
        .when(!on, |chip| {
            chip.bg(p.inset).border_color(p.border).text_color(p.text)
        })
        .when(!on && !disabled, |chip| {
            chip.hover(|chip| chip.bg(p.raised))
        })
        .when(disabled, |chip| chip.opacity(0.45))
        .when(!disabled, |chip| chip.cursor_pointer())
        .child(label.into())
}

/// Dropdown trigger that reads like a select: value on the left, caret on the
/// right. A plain `Button` centers its label, which makes a wide select look
/// like a call to action instead of a field.
pub fn select_trigger(
    cx: &gpui_kit::App,
    id: impl Into<gpui_kit::ElementId>,
    value: impl Into<gpui_kit::SharedString>,
    disabled: bool,
) -> Button {
    let p = Palette::current(cx);
    Button::new(id)
        .w_full()
        .disabled(disabled)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_left()
                .child(value.into()),
        )
        .child(
            Icon::new(IconName::ChevronDown)
                .size(px(14.))
                .text_color(p.muted),
        )
}

/// Checkbox row for a boolean setting inside a card.
pub fn check_row(
    cx: &gpui_kit::App,
    id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    checked: bool,
    disabled: bool,
) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = Palette::current(cx);
    div()
        .id(id)
        .h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .py_1()
        .rounded(px(4.))
        .text_sm()
        .text_color(p.text)
        .when(!disabled, |row| {
            row.cursor_pointer().hover(|row| row.bg(p.raised))
        })
        .when(disabled, |row| row.opacity(0.5))
        .child(
            div()
                .size(px(15.))
                .flex_shrink_0()
                .h_flex()
                .items_center()
                .justify_center()
                .rounded(px(3.))
                .border_1()
                .border_color(if checked { p.accent } else { p.border })
                .when(checked, |box_| box_.bg(p.accent))
                .when(checked, |box_| {
                    box_.child(
                        Icon::new(IconName::Check)
                            .size(px(11.))
                            .text_color(p.canvas),
                    )
                }),
        )
        .child(div().flex_1().min_w_0().truncate().child(label.into()))
}
