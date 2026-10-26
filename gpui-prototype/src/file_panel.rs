use gpui_kit::base::Disableable;
use gpui_kit::component::{
    Sizable, StyledExt, Theme,
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
        let mut tabs = div().h_flex().gap_2();
        for (i, page, label) in [
            (0, FilePage::Encoding, "指定编码打开"),
            (1, FilePage::Recovery, "自动保存与恢复"),
        ] {
            tabs = tabs.child(
                Button::new(format!("file-tools-page-{i}"))
                    .label(label)
                    .small()
                    .when(self.page == page, |button| button.primary())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.files.page = page;
                        view.file_tools_request(json!({"action":"state"}), cx);
                    })),
            );
        }
        let mut panel = div().v_flex().flex_1().min_h_0().p_4().gap_3().child(tabs);
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
            let mut common = div()
                .h_flex()
                .flex_wrap()
                .gap_2()
                .items_center()
                .child(div().text_sm().child("常用编码"));
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
                    let value = value.clone();
                    common = common.child(
                        Button::new(format!("encoding-preset-{index}"))
                            .label(code)
                            .small()
                            .when(value == self.encoding, |button| button.primary())
                            .disabled(self.loading)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.files.encoding = value.clone();
                                cx.notify();
                            })),
                    );
                }
            }
            panel = panel
                .child(div().text_lg().font_semibold().child("指定编码打开字幕"))
                .child(
                    div()
                        .h_flex()
                        .gap_3()
                        .items_center()
                        .child(
                            Button::new("encoded-open-pick")
                                .label("选择字幕文件…")
                                .small()
                                .disabled(self.loading)
                                .on_click(
                                    cx.listener(|view, _, _, cx| view.pick_encoded_subtitle(cx)),
                                ),
                        )
                        .child(div().text_sm().child(if self.path.is_empty() {
                            "尚未选择文件".into()
                        } else {
                            self.path.clone()
                        })),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_3()
                        .items_center()
                        .child("读取编码")
                        .child(
                            Button::new("encoded-open-charset")
                                .label(self.encoding.clone())
                                .small()
                                .w(px(330.))
                                .disabled(self.loading)
                                .dropdown_menu(move |mut menu, _, _| {
                                    for encoding in &choices {
                                        let weak = weak.clone();
                                        let value = encoding.clone();
                                        menu = menu.item(
                                            PopupMenuItem::new(value.clone())
                                                .checked(value == selected)
                                                .on_click(move |_, _, cx| {
                                                    let _ = weak.update(cx, |view, cx| {
                                                        view.files.encoding = value.clone();
                                                        cx.notify();
                                                    });
                                                }),
                                        );
                                    }
                                    menu
                                }),
                        ),
                )
                .child(common)
                .child(
                    div()
                        .text_sm()
                        .text_color(Theme::global(cx).muted_foreground)
                        .child(if self.path.is_empty() {
                            "选择文件后显示编码检测结果".into()
                        } else if self.state.detected.is_empty() {
                            "未能自动检测编码，请手动选择编码".into()
                        } else {
                            format!("检测结果：{}", self.state.detected)
                        }),
                )
                .child(
                    Button::new("encoded-open-submit")
                        .label("按所选编码打开")
                        .primary()
                        .small()
                        .disabled(self.loading || self.path.is_empty())
                        .on_click(cx.listener(|view, _, _, cx| view.open_file_tools_selection(cx))),
                );
        } else {
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
                files = files.child(
                    Button::new(format!("recovery-file-{index}"))
                        .label(format!("{} · {} 个版本", file.name, file.versions.len()))
                        .small()
                        .when(self.group == file.name, |button| button.primary())
                        .disabled(self.loading)
                        .on_click(cx.listener(move |view, _, _, cx| {
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
                        })),
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
                    versions = versions.child(
                        div()
                            .v_flex()
                            .gap_1()
                            .px_2()
                            .py_2()
                            .border_b_1()
                            .border_color(Theme::global(cx).border)
                            .child(
                                Button::new(format!("recovery-version-{index}"))
                                    .label(format!("{kind} · {}", version.display))
                                    .small()
                                    .when(self.version == version.path, |button| button.primary())
                                    .disabled(self.loading)
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.files.version = path.clone();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(Theme::global(cx).muted_foreground)
                                    .child(version.path.clone()),
                            ),
                    );
                }
            }
            panel = panel.child(div().h_flex().flex_wrap().gap_2().items_center()
                .child(div().text_lg().font_semibold().child("自动保存与恢复"))
                .child(Button::new("recovery-refresh").label("刷新版本").small().disabled(self.loading).on_click(cx.listener(|view, _, _, cx| view.file_tools_request(json!({"action":"state"}), cx))))
                .child(Button::new("recovery-open").label("打开所选版本").primary().small().disabled(self.loading || self.version.is_empty()).on_click(cx.listener(|view, _, _, cx| view.open_file_tools_selection(cx))))
                .child(Input::new(&self.query).small().w(px(300.))))
                .child(div().text_sm().text_color(Theme::global(cx).muted_foreground).child("版本来自已配置的自动保存、原始备份和异常恢复目录；打开后可用“另存为”保存新的副本。"))
                .when(self.state.files.is_empty(), |panel| panel.child("没有找到可恢复的字幕版本。"))
                .child(div().h_flex().items_stretch().flex_1().min_h_0().overflow_hidden().gap_4()
                    .child(div().v_flex().w(px(340.)).flex_shrink_0().min_h_0().gap_2().child("文件").child(files))
                    .child(div().v_flex().flex_1().min_w_0().min_h_0().gap_2().child("版本 · 按时间从新到旧排列").child(versions)));
        }
        panel
    }
}
