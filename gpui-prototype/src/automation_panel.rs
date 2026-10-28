use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt, Theme,
    button::{Button, ButtonVariants},
    scroll::ScrollableElement,
};
use gpui_kit::{Context, IntoElement, div, prelude::*, px};
use serde::Deserialize;
use serde_json::json;

use crate::{Workspace, command_catalog::Catalog};

#[derive(Clone, Default, Deserialize)]
pub struct Feature {
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub help: String,
}

#[derive(Clone, Deserialize)]
pub struct Script {
    pub scope: String,
    pub path: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub loaded: bool,
    pub warnings: Vec<String>,
    pub macros: Vec<Feature>,
    pub filters: Vec<Feature>,
}

#[derive(Clone, Deserialize)]
pub struct Engine {
    pub name: String,
    pub pattern: String,
}

#[derive(Default, Deserialize)]
pub struct AutomationState {
    pub scripts: Vec<Script>,
    pub engines: Vec<Engine>,
}

#[derive(Default)]
pub struct AutomationPanel {
    pub state: AutomationState,
    pub loading: bool,
    pub error: Option<String>,
    pub confirm_remove: Option<(String, String)>,
}

impl AutomationPanel {
    pub fn render(&self, catalog: &Catalog, cx: &mut Context<Workspace>) -> impl IntoElement {
        let mut toolbar = div().h_flex().flex_wrap().gap_2().items_center().child(
            Button::new("automation-add")
                .label("添加项目脚本…")
                .primary()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| view.pick_automation_script(cx))),
        );
        for (id, label, action) in [
            ("automation-refresh", "刷新列表", "state"),
            ("automation-rescan", "重新扫描自动加载目录", "rescan"),
            ("automation-reload-all", "重新加载全部", "reload_all"),
        ] {
            toolbar = toolbar.child(
                Button::new(id)
                    .label(label)
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.automation_request(json!({"action":action}), cx)
                    })),
            );
        }
        let local = self
            .state
            .scripts
            .iter()
            .filter(|script| script.scope == "local")
            .count();
        let engines = self
            .state
            .engines
            .iter()
            .map(|engine| format!("{} ({})", engine.name, engine.pattern))
            .collect::<Vec<_>>()
            .join("、");
        let mut panel = div()
            .id("automation-panel")
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_3()
            .p_4()
            .overflow_y_scrollbar()
            .child(toolbar)
            .child(format!(
                "项目脚本 {local} · 自动加载脚本 {}",
                self.state.scripts.len() - local
            ))
            .child(div().text_sm().child(format!("已安装引擎：{engines}")))
            .child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child(
                        "项目脚本随字幕项目关联；移除会卸载脚本。宏使用当前字幕、选择和撤销记录。",
                    ),
            );
        if self.loading {
            panel = panel.child(div().text_sm().child("正在同步脚本…"));
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        if self.state.scripts.is_empty() && !self.loading {
            panel = panel.child("当前没有已加载脚本，可添加 Lua 或 MoonScript 文件。");
        }
        for (index, script) in self.state.scripts.iter().enumerate() {
            let scope = script.scope.clone();
            let path = script.path.clone();
            let remove_scope = scope.clone();
            let remove_path = path.clone();
            let confirming = self.confirm_remove.as_ref() == Some(&(scope.clone(), path.clone()));
            let mut actions = div().h_flex().gap_2().child(
                Button::new(format!("automation-reload-{index}"))
                    .label("重新加载")
                    .disabled(self.loading)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.automation_request(
                            json!({"action":"reload", "scope":scope, "path":path}),
                            cx,
                        )
                    })),
            );
            if script.scope == "local" {
                actions = actions.child(
                    Button::new(format!("automation-remove-{index}"))
                        .label(if confirming { "确认卸载项目脚本" } else { "卸载…" })

                        .disabled(self.loading)
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if view.automation.confirm_remove.as_ref() == Some(&(remove_scope.clone(), remove_path.clone())) {
                                view.automation.confirm_remove = None;
                                view.automation_request(json!({"action":"remove", "scope":remove_scope, "path":remove_path}), cx);
                            } else {
                                view.automation.confirm_remove = Some((remove_scope.clone(), remove_path.clone()));
                                cx.notify();
                            }
                        })),
                );
            }
            if confirming {
                actions = actions.child(
                    Button::new(format!("automation-cancel-remove-{index}"))
                        .label("取消")
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.automation.confirm_remove = None;
                            cx.notify();
                        })),
                );
            }
            let state = if !script.loaded {
                "加载失败"
            } else if !script.warnings.is_empty() {
                "已加载，有警告"
            } else {
                "已加载"
            };
            let mut details = div()
                .v_flex()
                .gap_2()
                .border_1()
                .border_color(Theme::global(cx).border)
                .rounded_md()
                .p_3()
                .child(
                    div()
                        .h_flex()
                        .flex_wrap()
                        .gap_3()
                        .items_center()
                        .justify_between()
                        .child(div().font_semibold().child(format!(
                            "{} · {} · {state}",
                            script.name,
                            if script.scope == "local" {
                                "项目"
                            } else {
                                "自动加载"
                            }
                        )))
                        .child(actions),
                )
                .child(div().text_sm().child(script.description.clone()))
                .child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(format!(
                            "作者：{} · 版本：{}",
                            script.author, script.version
                        )),
                )
                .child(div().text_xs().child(script.path.clone()));
            for warning in &script.warnings {
                details = details.child(div().text_sm().child(format!("警告：{warning}")));
            }
            for feature in &script.macros {
                let name = feature.name.clone();
                let enabled = catalog
                    .command(&name)
                    .is_some_and(|command| command.can_invoke());
                details = details.child(
                    div()
                        .h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Button::new(format!("automation-macro-{}-{index}", feature.name))
                                .label(format!("执行 {}", feature.label))
                                .disabled(self.loading || !enabled)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.invoke(json!({"op":"command", "name":name}), cx)
                                })),
                        )
                        .child(
                            div()
                                .text_sm()
                                .flex_1()
                                .min_w(px(80.))
                                .child(feature.help.clone()),
                        ),
                );
            }
            for filter in &script.filters {
                details = details.child(div().text_sm().child(format!(
                    "导出过滤器：{} · {}",
                    filter.name, filter.description
                )));
            }
            panel = panel.child(details);
        }
        panel
    }
}
