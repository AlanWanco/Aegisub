use gpui_kit::assets::IconName;
use gpui_kit::base::Disableable;
use gpui_kit::component::Sizable;
use gpui_kit::component::{
    Icon, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    menu::{DropdownMenu, PopupMenuItem},
    scroll::ScrollableElement,
};
use gpui_kit::{
    AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::json;

use crate::Workspace;

#[derive(Clone, Deserialize)]
pub struct SavedVersion {
    pub path: String,
    pub display: String,
    pub kind: String,
}
#[derive(Clone, Deserialize)]
pub struct SavedFile {
    pub name: String,
    pub versions: Vec<SavedVersion>,
}
#[derive(Default, Deserialize)]
pub struct FileState {
    #[serde(default)]
    pub encodings: Vec<String>,
    #[serde(default)]
    pub files: Vec<SavedFile>,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub detected: String,
}
#[derive(Clone, Copy, PartialEq)]
pub enum FilePage {
    Encoding,
    Recovery,
}

pub struct FilePanel {
    pub page: FilePage,
    pub state: FileState,
    pub path: String,
    pub encoding: String,
    pub group: String,
    pub version: String,
    pub query: Entity<InputState>,
    pub loading: bool,
    pub error: Option<String>,
    _events: Subscription,
}

impl FilePanel {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        let query = cx.new(|cx| InputState::new(window, cx).placeholder("筛选文件名称"));
        let events = cx.subscribe(&query, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            page: FilePage::Encoding,
            state: FileState::default(),
            path: String::new(),
            encoding: "Unicode (UTF-8)".into(),
            group: String::new(),
            version: String::new(),
            query,
            loading: false,
            error: None,
            _events: events,
        }
    }

    pub fn receive(&mut self, state: FileState) {
        if !state.path.is_empty() {
            self.path = state.path.clone();
        }
        if !state.files.iter().any(|file| file.name == self.group) {
            self.group = state
                .files
                .first()
                .map(|file| file.name.clone())
                .unwrap_or_default();
        }
        if let Some(file) = state.files.iter().find(|file| file.name == self.group) {
            if !file.versions.iter().any(|v| v.path == self.version) {
                self.version = file
                    .versions
                    .first()
                    .map(|v| v.path.clone())
                    .unwrap_or_default();
            }
        } else {
            self.version.clear();
        }
        self.state = state;
    }

    pub fn render(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let tabs = crate::page::tabs(
            cx,
            [
                (0, FilePage::Encoding, "指定编码打开"),
                (1, FilePage::Recovery, "自动保存与恢复"),
            ]
            .into_iter()
            .map(|(index, page, label)| {
                (
                    format!("file-tools-page-{index}"),
                    label.to_owned(),
                    self.page == page,
                )
            })
            .collect(),
            |view, index, _, cx| {
                view.files.page = if index == 0 {
                    FilePage::Encoding
                } else {
                    FilePage::Recovery
                };
                view.file_tools_request(json!({"action":"state"}), cx);
            },
        );
        let mut panel = crate::page::page(cx, "file-tools-panel").child(tabs);
        if self.loading {
            panel = panel.child("正在读取文件信息…");
        }
        if let Some(error) = &self.error {
            panel = panel.child(
                div()
                    .text_sm()
                    .text_color(Theme::global(cx).danger)
                    .child(error.clone()),
            );
        }
        if self.page == FilePage::Encoding {
            let choices = self.state.encodings.clone();
            let selected = self.encoding.clone();
            let weak = cx.weak_entity();
            let mut presets: Vec<(String, &'static str, bool, bool)> = Vec::new();
            for (index, code) in [
                "UTF-8",
                "GB18030",
                "GBK",
                "BIG5",
                "SHIFT_JIS",
                "UTF-16",
                "UTF-32",
            ]
            .iter()
            .enumerate()
            {
                let code = *code;
                if let Some(value) = self
                    .state
                    .encodings
                    .iter()
                    .find(|name| name.to_uppercase().contains(&format!("({code})")))
                {
                    presets.push((
                        format!("encoding-preset-{index}"),
                        code,
                        *value == self.encoding,
                        self.loading,
                    ));
                }
            }
            let preset_control = crate::page::segmented(cx, presets, |view, index, _, cx| {
                let presets: Vec<String> = [
                    "UTF-8",
                    "GB18030",
                    "GBK",
                    "BIG5",
                    "SHIFT_JIS",
                    "UTF-16",
                    "UTF-32",
                ]
                .iter()
                .filter_map(|code| {
                    view.files
                        .state
                        .encodings
                        .iter()
                        .find(|name| name.to_uppercase().contains(&format!("({code})")))
                        .cloned()
                })
                .collect();
                if let Some(value) = presets.get(index) {
                    view.files.encoding = value.clone();
                }
                cx.notify();
            });
            let detected = if self.path.is_empty() {
                "选择文件后显示编码检测结果".to_owned()
            } else if self.state.detected.is_empty() {
                "未能自动检测编码，请手动选择编码".to_owned()
            } else {
                format!("检测结果：{}", self.state.detected)
            };
            panel =
                panel.child(crate::page::section_with_icon(
                    cx,
                    Some(IconName::FileInput),
                    "按编码重新打开",
                    None,
                    crate::page::section_body()
                        .child(
                            div()
                                .h_flex()
                                .flex_wrap()
                                .items_center()
                                .gap_3()
                                .child(
                                    Button::new("encoded-open-pick")
                                        .icon(IconName::FolderOpen)
                    .rounded(gpui_kit::px(999.)).small()
                                        .label("选择字幕文件…")
                                        .disabled(self.loading)
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.pick_encoded_subtitle(cx)
                                        })),
                                )
                                .child(div().text_sm().min_w_0().truncate().child(
                                    if self.path.is_empty() {
                                        "尚未选择文件".to_owned()
                                    } else {
                                        self.path.clone()
                                    },
                                )),
                        )
                        .child(
                            div()
                                .h_flex()
                                .flex_wrap()
                                .items_center()
                                .gap_5()
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(crate::page::field_label(cx, "读取编码"))
                                        .child(
                                            Button::new("encoded-open-charset")
                    .rounded(gpui_kit::px(999.)).small()
                                                .w(px(300.))
                                                .disabled(self.loading)
                                                .child(
                                                    div()
                                                        .h_flex()
                                                        .w_full()
                                                        .items_center()
                                                        .justify_between()
                                                        .gap_2()
                                                        .child(
                                                            div()
                                                                .h_flex()
                                                                .items_center()
                                                                .gap_2()
                                                                .min_w_0()
                                                                .child(
                                                                    Icon::new(IconName::FileText)
                                                                        .size(px(14.))
                                                                        .text_color(crate::workspace_theme::Palette::current(cx).muted),
                                                                )
                                                                .child(
                                                                    div()
                                                                        .min_w_0()
                                                                        .overflow_hidden()
                                                                        .text_ellipsis()
                                                                        .child(self.encoding.clone()),
                                                                ),
                                                        )
                                                        .child(
                                                            Icon::new(IconName::ChevronDown)
                                                                .size(px(14.))
                                                                .text_color(crate::workspace_theme::Palette::current(cx).muted),
                                                        ),
                                                )
                                                .dropdown_menu(move |mut menu, _, _| {
                                                    for encoding in &choices {
                                                        let weak = weak.clone();
                                                        let value = encoding.clone();
                                                        menu = menu.item(
                                                            PopupMenuItem::new(value.clone())
                                                                .checked(value == selected)
                                                                .on_click(move |_, _, cx| {
                                                                    let _ = weak.update(
                                                                        cx,
                                                                        |view, cx| {
                                                                            view.files.encoding =
                                                                                value.clone();
                                                                            cx.notify();
                                                                        },
                                                                    );
                                                                }),
                                                        );
                                                    }
                                                    menu
                                                }),
                                        ),
                                )
                                .child(
                                    div()
                                        .h_flex()
                                        .items_center()
                                        .gap_2()
                                        .child(crate::page::field_label(cx, "常用"))
                                        .child(preset_control),
                                ),
                        )
                        .child(crate::page::hint(cx, detected))
                        .child(
                            div().h_flex().items_center().gap_3().child(
                                Button::new("encoded-open-submit")
                                    .icon(IconName::FileInput)
                    .rounded(gpui_kit::px(999.)).small()
                                    .label("按所选编码打开")
                                    .primary()
                                    .disabled(self.loading || self.path.is_empty())
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.open_file_tools_selection(cx)
                                    })),
                            ),
                        ),
                ));
        } else {
            let p = crate::workspace_theme::Palette::current(cx);
            let query = self.query.read(cx).value().to_lowercase();
            let mut files = div()
                .id("recovery-file-list")
                .v_flex()
                .gap_1()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar();
            for (index, file) in self.state.files.iter().enumerate() {
                if !query.is_empty() && !file.name.to_lowercase().contains(&query) {
                    continue;
                }
                let name = file.name.clone();
                let selected = self.group == file.name;
                let count = file.versions.len();
                files = files.child(
                    div()
                        .id(format!("recovery-file-{index}"))
                        .v_flex()
                        .gap_0p5()
                        .w_full()
                        .min_w_0()
                        .rounded(px(crate::workspace_theme::RADIUS_SM))
                        .border_l_2()
                        .border_color(if selected {
                            p.accent
                        } else {
                            gpui_kit::rgba(0x00000000).into()
                        })
                        .px_3()
                        .py_2()
                        .cursor_pointer()
                        .when(selected, |row| row.bg(p.selected))
                        .when(!selected && !self.loading, |row| {
                            row.hover(|row| row.bg(p.raised))
                        })
                        .when(self.loading, |row| row.opacity(0.5))
                        .on_mouse_down(
                            gpui_kit::MouseButton::Left,
                            cx.listener(move |view, _, _, cx| {
                                if view.files.loading {
                                    return;
                                }
                                view.files.group = name.clone();
                                view.files.version = view
                                    .files
                                    .state
                                    .files
                                    .iter()
                                    .find(|file| file.name == name)
                                    .and_then(|file| file.versions.first())
                                    .map(|version| version.path.clone())
                                    .unwrap_or_default();
                                cx.notify();
                            }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(if selected { p.selected_text } else { p.text })
                                .truncate()
                                .child(file.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(if selected { p.selected_text } else { p.muted })
                                .child(format!("{count} 个版本")),
                        ),
                );
            }
            let mut versions = div()
                .id("recovery-version-list")
                .v_flex()
                .gap_2()
                .flex_1()
                .min_h_0()
                .overflow_y_scrollbar();
            if let Some(file) = self.state.files.iter().find(|file| file.name == self.group) {
                for (index, version) in file.versions.iter().enumerate() {
                    let path = version.path.clone();
                    let kind = match version.kind.as_str() {
                        "backup" => "原始备份",
                        "recovered" => "异常恢复",
                        _ => "自动保存",
                    };
                    let selected = self.version == version.path;
                    let display = version.display.clone();
                    let full_path = version.path.clone();
                    versions = versions.child(
                        div()
                            .id(format!("recovery-version-{index}"))
                            .v_flex()
                            .gap_1()
                            .w_full()
                            .min_w_0()
                            .rounded(px(crate::workspace_theme::RADIUS_SM))
                            .border_l_2()
                            .border_color(if selected {
                                p.accent
                            } else {
                                gpui_kit::rgba(0x00000000).into()
                            })
                            .px_3()
                            .py_2()
                            .cursor_pointer()
                            .when(selected, |row| row.bg(p.selected))
                            .when(!selected && !self.loading, |row| {
                                row.hover(|row| row.bg(p.raised))
                            })
                            .when(self.loading, |row| row.opacity(0.5))
                            .on_mouse_down(
                                gpui_kit::MouseButton::Left,
                                cx.listener(move |view, _, _, cx| {
                                    if view.files.loading {
                                        return;
                                    }
                                    view.files.version = path.clone();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_medium()
                                    .text_color(if selected { p.selected_text } else { p.text })
                                    .truncate()
                                    .child(format!("{kind} · {display}")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .truncate()
                                    .text_color(if selected { p.selected_text } else { p.muted })
                                    .child(full_path),
                            ),
                    );
                }
            }
            panel = panel
                .child(crate::page::toolbar([
                    Button::new("recovery-open").icon(IconName::FileInput).rounded(gpui_kit::px(999.)).small()
                        .label("打开所选版本")
                        .primary()
                        .disabled(self.loading || self.version.is_empty())
                        .on_click(cx.listener(|view, _, _, cx| view.open_file_tools_selection(cx)))
                        .into_any_element(),
                    Button::new("recovery-refresh").icon(IconName::RefreshCw).rounded(gpui_kit::px(999.)).small()
                        .label("刷新版本")
                        .disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.file_tools_request(json!({"action":"state"}), cx)
                        }))
                        .into_any_element(),
                    div()
                        .ml_auto()
                        .child(Input::new(&self.query).w(px(280.)))
                        .into_any_element(),
                ]))
                .child(crate::page::hint(
                    cx,
                    "版本来自已配置的自动保存、原始备份和异常恢复目录；打开后可用“另存为”保存新的副本。",
                ))
                .when(self.state.files.is_empty(), |panel| {
                    panel.child(crate::page::hint(cx, "没有找到可恢复的字幕版本。"))
                })
                .child(
                    div()
                        .h_flex()
                        .items_stretch()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .gap_4()
                        .child(
                            div()
                                .v_flex()
                                .w(px(320.))
                                .flex_shrink_0()
                                .min_h_0()
                                .gap_2()
                                .child(crate::page::field_label(cx, "文件"))
                                .child(files),
                        )
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .gap_2()
                                .child(crate::page::field_label(cx, "版本 · 按时间从新到旧排列"))
                                .child(versions),
                        ),
                );
        }
        panel
    }
}
