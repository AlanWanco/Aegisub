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
    Icon, Sizable, StyledExt, Theme,
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
        .child(div().text_sm().font_semibold().child(title.into()));
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
    div()
        .v_flex()
        .flex_1()
        .min_w_0()
        .gap_1()
        .child(field_label(cx, label))
        .child(control)
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
            Button::new(id)
                .label(label)
                .ghost()
                .when(selected, |button| button.primary())
                .disabled(disabled)
                .on_click(cx.listener(move |view, _, window, cx| handler(view, index, window, cx))),
        );
    }
    row
}
