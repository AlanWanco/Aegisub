use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::{
    StyledExt, Theme,
    button::{Button, ButtonVariants},
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
        let toolbar = crate::page::toolbar([
            Button::new("automation-add")
                .icon(IconName::Plus)
                .rounded(gpui_kit::px(999.))
                .label("添加项目脚本…")
                .primary()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| view.pick_automation_script(cx)))
                .into_any_element(),
            Button::new("automation-reload-all")
                .icon(IconName::RefreshCw)
                .rounded(gpui_kit::px(999.))
                .label("重新加载全部")
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.automation_request(json!({"action":"reload_all"}), cx)
                }))
                .into_any_element(),
            Button::new("automation-rescan")
                .icon(IconName::Search)
                .rounded(gpui_kit::px(999.))
                .label("重新扫描目录")
                .ghost()
                .disabled(self.loading)
                .on_click(cx.listener(|view, _, _, cx| {
                    view.automation_request(json!({"action":"rescan"}), cx)
                }))
                .into_any_element(),
        ])
        .child(div().flex_1())
        .child(crate::page::badge(
            cx,
            format!(
                "项目脚本 {local} · 自动加载 {}",
                self.state.scripts.len() - local
            ),
        ))
        .child(crate::page::badge(cx, format!("引擎 {engines}")));

        let mut panel = crate::page::page(cx, "automation-panel")
            .child(crate::page::header(
                cx,
                IconName::SquareTerminal,
                "自动化脚本",
                "脚本随字幕项目关联；移除会卸载脚本，执行脚本会写入当前字幕并可撤销。",
            ))
            .child(toolbar);
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
                    .icon(IconName::RefreshCw)
                    .rounded(gpui_kit::px(999.))
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
                    Button::new(format!("automation-remove-{index}")).icon(IconName::Delete).rounded(gpui_kit::px(999.))
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
                        .icon(IconName::Close)
                        .rounded(gpui_kit::px(999.))
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
            let scope_badge = if script.scope == "local" {
                "项目"
            } else {
                "自动加载"
            };
            let header_actions = div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(crate::page::badge(cx, scope_badge))
                .child(crate::page::badge(cx, state))
                .child(actions);
            let mut details = crate::page::section_body()
                .when(!script.description.is_empty(), |body| {
                    body.child(div().text_sm().child(script.description.clone()))
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(format!("作者 {} · 版本 {}", script.author, script.version)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(Theme::global(cx).muted_foreground)
                        .truncate()
                        .child(script.path.clone()),
                );
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
                                .icon(IconName::Play)
                                .rounded(gpui_kit::px(999.))
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
                details = details.child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(format!(
                            "导出过滤器 · {} — {}",
                            filter.name, filter.description
                        )),
                );
            }
            panel = panel.child(crate::page::section(
                cx,
                script.name.clone(),
                Some(header_actions.into_any_element()),
                details,
            ));
        }
        panel
    }
}
