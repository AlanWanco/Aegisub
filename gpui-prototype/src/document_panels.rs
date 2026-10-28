use gpui_kit::base::Disableable;
use std::collections::{BTreeMap, BTreeSet};

use gpui_kit::component::{
    Sizable, StyledExt, Theme,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
};
use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, Subscription, Window, div, prelude::*, px,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Workspace;

pub const STYLE_FIELDS: &[(&str, &str)] = &[
    ("name", "样式名称"),
    ("font", "字体"),
    ("fontsize", "字号"),
    ("primary", "主颜色"),
    ("secondary", "次颜色"),
    ("outline", "描边颜色"),
    ("shadow", "阴影颜色"),
    ("bold", "粗体 · 0/1"),
    ("italic", "斜体 · 0/1"),
    ("underline", "下划线 · 0/1"),
    ("strikeout", "删除线 · 0/1"),
    ("scalex", "水平缩放 %"),
    ("scaley", "垂直缩放 %"),
    ("spacing", "字间距"),
    ("angle", "旋转角度"),
    ("borderstyle", "边框样式 · 1/3/4"),
    ("outline_w", "描边宽度"),
    ("shadow_w", "阴影距离"),
    ("alignment", "对齐 · 1–9"),
    ("margin_left", "左边距"),
    ("margin_right", "右边距"),
    ("margin_vertical", "垂直边距"),
    ("encoding", "字体编码"),
];
pub const SCRIPT_FIELDS: &[(&str, &str)] = &[
    ("Title", "标题"),
    ("Original Script", "原作"),
    ("Original Translation", "翻译"),
    ("Original Editing", "编辑"),
    ("Original Timing", "时间轴"),
    ("Synch Point", "同步点"),
    ("Script Updated By", "更新者"),
    ("Update Details", "更新说明"),
    ("PlayResX", "脚本宽度"),
    ("PlayResY", "脚本高度"),
    ("LayoutResX", "布局宽度"),
    ("LayoutResY", "布局高度"),
    ("WrapStyle", "换行模式 · 0–3"),
    ("ScaledBorderAndShadow", "缩放边框 · yes/no"),
    ("YCbCr Matrix", "色彩矩阵"),
];

pub type Fields = BTreeMap<String, String>;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MediaDetails {
    #[serde(default)]
    pub video: String,
    #[serde(default)]
    pub audio: String,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub matrix: String,
    #[serde(default)]
    pub keyframes: usize,
    #[serde(default)]
    pub timecodes: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DocumentDetails {
    pub styles: Vec<Fields>,
    pub info: Fields,
    pub media: MediaDetails,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct LibraryDetails {
    pub catalog: String,
    pub catalogs: Vec<String>,
    pub styles: Vec<Fields>,
    pub import_styles: Vec<Fields>,
}

pub struct Form {
    pub inputs: BTreeMap<&'static str, Entity<InputState>>,
    pub dirty: BTreeSet<&'static str>,
    _events: Vec<Subscription>,
}

impl Form {
    fn new(
        fields: &'static [(&'static str, &'static str)],
        style: bool,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Self {
        let mut inputs = BTreeMap::new();
        let mut events = Vec::new();
        for &(key, _) in fields {
            let input = cx.new(|cx| InputState::new(window, cx));
            events.push(
                cx.subscribe(&input, move |view, _, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change) {
                        if style {
                            view.document.style_form.dirty.insert(key);
                        } else {
                            view.document.script_form.dirty.insert(key);
                        }
                        cx.notify();
                    }
                }),
            );
            inputs.insert(key, input);
        }
        Self {
            inputs,
            dirty: BTreeSet::new(),
            _events: events,
        }
    }

    pub fn sync(&mut self, fields: &Fields, window: &mut Window, cx: &mut App) {
        for (&key, input) in &self.inputs {
            if self.dirty.contains(key) {
                continue;
            }
            let value = fields.get(key).cloned().unwrap_or_default();
            if input.read(cx).value().as_ref() != value {
                input.update(cx, |state, cx| state.set_value(value, window, cx));
            }
        }
    }

    pub fn patch(&self, cx: &App) -> Value {
        Value::Object(
            self.dirty
                .iter()
                .map(|key| {
                    (
                        (*key).into(),
                        json!(self.inputs[key].read(cx).value().to_string()),
                    )
                })
                .collect(),
        )
    }

    pub fn acknowledge(&mut self, sent: &Value, cx: &App) {
        if let Some(fields) = sent.as_object() {
            self.dirty.retain(|key| {
                fields.get(*key).and_then(Value::as_str)
                    != Some(self.inputs[key].read(cx).value().as_ref())
            });
        }
    }
}

pub struct DocumentPanels {
    pub details: DocumentDetails,
    pub loading: bool,
    pub style: String,
    pub style_form: Form,
    pub script_form: Form,
    pub confirm_delete: bool,
    pub library: LibraryDetails,
    pub library_loading: bool,
    pub library_style: String,
    pub editing_library: bool,
    pub selected_library: BTreeSet<String>,
    pub catalog_name: Entity<InputState>,
    pub confirm_delete_catalog: bool,
    pub overwrite_styles: bool,
    pub import_path: String,
    pub import_styles: Vec<Fields>,
    pub selected_import: BTreeSet<String>,
}

impl DocumentPanels {
    pub fn new(window: &mut Window, cx: &mut Context<Workspace>) -> Self {
        Self {
            details: DocumentDetails::default(),
            loading: false,
            style: String::new(),
            style_form: Form::new(STYLE_FIELDS, true, window, cx),
            script_form: Form::new(SCRIPT_FIELDS, false, window, cx),
            confirm_delete: false,
            library: LibraryDetails::default(),
            library_loading: false,
            library_style: String::new(),
            editing_library: false,
            selected_library: BTreeSet::new(),
            catalog_name: cx.new(|cx| InputState::new(window, cx).placeholder("新样式库名称")),
            confirm_delete_catalog: false,
            overwrite_styles: false,
            import_path: String::new(),
            import_styles: vec![],
            selected_import: BTreeSet::new(),
        }
    }

    pub fn sync(&mut self, window: &mut Window, cx: &mut App) {
        if !self
            .details
            .styles
            .iter()
            .any(|style| style.get("name") == Some(&self.style))
        {
            self.style = self
                .details
                .styles
                .first()
                .and_then(|style| style.get("name"))
                .cloned()
                .unwrap_or_default();
            if !self.editing_library {
                self.style_form.dirty.clear();
            }
        }
        let selected = if self.editing_library {
            self.library
                .styles
                .iter()
                .find(|style| style.get("name") == Some(&self.library_style))
        } else {
            self.details
                .styles
                .iter()
                .find(|style| style.get("name") == Some(&self.style))
        };
        if let Some(style) = selected {
            self.style_form.sync(style, window, cx);
        }
        self.script_form.sync(&self.details.info, window, cx);
    }

    pub fn render_styles(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let muted = Theme::global(cx).muted_foreground;
        let p = crate::workspace_theme::Palette::current(cx);
        let mut list = div()
            .id("document-style-list")
            .v_flex()
            .flex_1()
            .min_h_0()
            .gap_1()
            .p_2()
            .overflow_y_scrollbar();
        for style in &self.details.styles {
            let Some(name) = style.get("name") else {
                continue;
            };
            let name = name.clone();
            list = list.child(
                Button::new(format!("document-style-{name}"))
                    .accessibility_label(name.clone())
                    .ghost()
                    .small()
                    .h(px(30.))
                    .w_full()
                    .when(name == self.style && !self.editing_library, |button| {
                        button.primary()
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .text_left()
                            .child(name.clone()),
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if view.document.style_form.dirty.is_empty() {
                            view.document.style = name.clone();
                            view.document.editing_library = false;
                            view.document.confirm_delete = false;
                        } else {
                            view.status = "请先应用或放弃当前样式的修改".into();
                        }
                        cx.notify();
                    })),
            );
        }
        let style_count = self.details.styles.len();
        let style_card = div()
            .v_flex()
            .w(px(212.))
            .flex_shrink_0()
            .min_h_0()
            .overflow_hidden()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .child(
                div()
                    .flex_shrink_0()
                    .h_flex()
                    .justify_between()
                    .items_center()
                    .px_3()
                    .py_2p5()
                    .border_b_1()
                    .border_color(p.divider)
                    .bg(p.surface)
                    .child(div().font_semibold().child("当前脚本样式"))
                    .child(div().text_xs().text_color(muted).child(style_count.to_string())),
            )
            .child(list)
            .child(
                div()
                    .v_flex()
                    .flex_shrink_0()
                    .gap_1()
                    .p_2()
                    .border_t_1()
                    .border_color(p.divider)
                    .child(
                        Button::new("new-document-style")
                            .label("新建样式")
                            .small()
                            .w_full()
                            .on_click(cx.listener(|view, _, _, cx| view.new_style(cx))),
                    )
                    .child(
                        Button::new("style-copy-to-catalog")
                            .label("复制到样式库 →")
                            .small()
                            .w_full()
                            .disabled(self.library.catalog.is_empty())
                            .on_click(cx.listener(|view, _, _, cx| {
                                let request = json!({"action":"to_catalog", "catalog":view.document.library.catalog, "names":[view.document.style], "overwrite":view.document.overwrite_styles});
                                view.library_request(request, cx);
                            })),
                    ),
            );
        let editing_name = if self.editing_library {
            &self.library_style
        } else {
            &self.style
        };
        let mut editor = div().id("document-style-form").v_flex().flex_1().min_w_0().min_h_0().gap_3().p_3().overflow_y_scrollbar()
            .child(div().h_flex().flex_wrap().gap_2()
                .child(Button::new("apply-style-fields").label("应用样式修改").primary().small()
                    .on_click(cx.listener(|view, _, _, cx| view.commit_style(cx))))
                .child(Button::new("discard-style-fields").label("放弃修改").small()
                    .on_click(cx.listener(|view, _, _, cx| { view.document.style_form.dirty.clear(); cx.notify(); })))
                .child(Button::new("duplicate-document-style").label("复制为新样式").small()
                    .on_click(cx.listener(|view, _, _, cx| view.duplicate_style(cx))))
                .child(Button::new("delete-document-style").label("删除…").small()
                    .on_click(cx.listener(|view, _, _, cx| { view.document.confirm_delete = true; cx.notify(); }))))
            .child(div().text_xs().text_color(muted).child("颜色格式 #RRGGBBAA，AA 沿用 ASS 透明度（00 不透明）。重命名同步更新行样式及 \\r 引用。"));
        if self.confirm_delete && self.editing_library {
            let source = self.library_style.clone();
            editor = editor.child(div().h_flex().gap_2()
                .child(Button::new("confirm-delete-library-style").label("确认从样式库删除").small()
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.library_request(json!({"action":"delete", "catalog":view.document.library.catalog, "source":source, "confirmed":true}), cx);
                        view.document.confirm_delete = false;
                    })))
                .child(Button::new("cancel-delete-library-style").label("取消").small()
                    .on_click(cx.listener(|view, _, _, cx| { view.document.confirm_delete = false; cx.notify(); }))));
        }
        if self.confirm_delete && !self.editing_library {
            let fallback = self
                .details
                .styles
                .iter()
                .filter_map(|style| style.get("name"))
                .find(|name| **name != self.style)
                .cloned();
            let style = self.style.clone();
            let label = fallback
                .as_ref()
                .map(|name| format!("确认删除，并将引用改为 {name}"))
                .unwrap_or_else(|| "至少保留一个样式".into());
            editor = editor.child(div().h_flex().gap_2()
                .child(Button::new("confirm-delete-style").label(label).small().disabled(fallback.is_none())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Some(replacement) = &fallback {
                            view.invoke(json!({"op":"style", "action":"delete", "source":style, "replacement":replacement}), cx);
                            view.document.confirm_delete = false;
                        }
                    })))
                .child(Button::new("cancel-delete-style").label("取消").small().on_click(cx.listener(|view, _, _, cx| { view.document.confirm_delete = false; cx.notify(); }))));
        }
        editor = editor.child(render_fields(
            "style-fields",
            STYLE_FIELDS,
            &self.style_form,
        ));
        let editor_card = div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .child(
                div()
                    .flex_shrink_0()
                    .h_flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .py_2p5()
                    .border_b_1()
                    .border_color(p.divider)
                    .bg(p.surface)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .text_ellipsis()
                            .font_semibold()
                            .child(format!(
                                "{} · {}",
                                if self.editing_library {
                                    "样式库"
                                } else {
                                    "当前脚本"
                                },
                                editing_name
                            )),
                    )
                    .child(div().text_xs().text_color(muted).child(
                        if self.style_form.dirty.is_empty() {
                            "无未应用修改"
                        } else {
                            "有未应用修改"
                        },
                    )),
            )
            .child(editor);
        div()
            .h_flex()
            .items_stretch()
            .flex_1()
            .min_h_0()
            .p_4()
            .gap_4()
            .child(style_card)
            .child(self.render_library(cx))
            .child(editor_card)
    }

    fn render_library(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let p = crate::workspace_theme::Palette::current(cx);
        let mut panel = div()
            .id("style-library")
            .v_flex()
            .w(px(252.))
            .flex_shrink_0()
            .min_h_0()
            .gap_2()
            .p_3()
            .rounded(px(8.))
            .border_1()
            .border_color(p.border)
            .overflow_y_scrollbar()
            .child(div().text_sm().font_semibold().child("样式库"))
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("读取旧版 .sty 库。库内修改即时保存；复制到脚本后可撤销。"),
            )
            .child(Input::new(&self.catalog_name).small().w_full())
            .child(
                div()
                    .h_flex()
                    .gap_1()
                    .child(
                        Button::new("new-style-catalog")
                            .label("新建库")
                            .small()
                            .on_click(cx.listener(|view, _, _, cx| {
                                let catalog =
                                    view.document.catalog_name.read(cx).value().to_string();
                                view.library_request(
                                    json!({"action":"create_catalog", "catalog":catalog}),
                                    cx,
                                );
                            })),
                    )
                    .child(
                        Button::new("refresh-style-catalog")
                            .label("刷新")
                            .small()
                            .on_click(cx.listener(|view, _, _, cx| view.refresh_library(cx))),
                    )
                    .child(
                        Button::new("delete-style-catalog")
                            .label("删除库…")
                            .small()
                            .disabled(self.library.catalogs.len() <= 1)
                            .on_click(cx.listener(|view, _, _, cx| {
                                view.document.confirm_delete_catalog = true;
                                cx.notify();
                            })),
                    ),
            );
        for catalog in &self.library.catalogs {
            let name = catalog.clone();
            panel = panel.child(
                Button::new(format!("catalog-{name}"))
                    .label(name.clone())
                    .small()
                    .ghost()
                    .w_full()
                    .when(name == self.library.catalog, |button| button.primary())
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.library_request(json!({"action":"list", "catalog":name}), cx);
                    })),
            );
        }
        if self.confirm_delete_catalog {
            panel = panel.child(Button::new("confirm-delete-style-catalog").label(format!("确认删除库 {}", self.library.catalog)).small()
                .on_click(cx.listener(|view, _, _, cx| {
                    view.library_request(json!({"action":"delete_catalog", "catalog":view.document.library.catalog, "confirmed":true}), cx);
                    view.document.confirm_delete_catalog = false;
                })));
            panel = panel.child(
                Button::new("cancel-delete-style-catalog")
                    .label("取消删除库")
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.document.confirm_delete_catalog = false;
                        cx.notify();
                    })),
            );
        }
        panel = panel.child(
            Button::new("replace-same-name-style")
                .label("允许替换同名样式")
                .small()
                .when(self.overwrite_styles, |button| button.primary())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.document.overwrite_styles = !view.document.overwrite_styles;
                    cx.notify();
                })),
        );
        for style in &self.library.styles {
            let Some(name) = style.get("name") else {
                continue;
            };
            let name = name.clone();
            panel = panel.child(
                Button::new(format!("library-style-{name}"))
                    .label(name.clone())
                    .small()
                    .ghost()
                    .w_full()
                    .when(self.selected_library.contains(&name), |button| {
                        button.primary()
                    })
                    .on_click(
                        cx.listener(move |view, event: &gpui_kit::ClickEvent, _, cx| {
                            if !view.document.style_form.dirty.is_empty() {
                                view.status = "请先应用或放弃当前样式的修改".into();
                                cx.notify();
                                return;
                            }
                            let modifiers = event.modifiers();
                            if modifiers.control || modifiers.platform {
                                if !view.document.selected_library.remove(&name) {
                                    view.document.selected_library.insert(name.clone());
                                }
                            } else {
                                view.document.selected_library.clear();
                                view.document.selected_library.insert(name.clone());
                            }
                            view.document.library_style = name.clone();
                            view.document.editing_library = true;
                            view.document.confirm_delete = false;
                            cx.notify();
                        }),
                    ),
            );
        }
        panel = panel.child(Button::new("catalog-copy-to-script").label("← 复制选中样式到脚本").small().disabled(self.selected_library.is_empty())
            .on_click(cx.listener(|view, _, _, cx| {
                view.library_request(json!({"action":"to_script", "catalog":view.document.library.catalog, "names":view.document.selected_library, "overwrite":view.document.overwrite_styles}), cx);
            })));
        panel = panel.child(
            Button::new("import-styles-from-file")
                .label("从字幕文件导入…")
                .small()
                .on_click(cx.listener(|view, _, _, cx| view.pick_style_import(cx))),
        );
        for style in &self.import_styles {
            let Some(name) = style.get("name") else {
                continue;
            };
            let name = name.clone();
            panel = panel.child(
                Button::new(format!("import-style-{name}"))
                    .label(format!("导入 · {name}"))
                    .small()
                    .ghost()
                    .w_full()
                    .when(self.selected_import.contains(&name), |button| {
                        button.primary()
                    })
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if !view.document.selected_import.remove(&name) {
                            view.document.selected_import.insert(name.clone());
                        }
                        cx.notify();
                    })),
            );
        }
        if !self.import_path.is_empty() {
            panel = panel.child(Button::new("commit-style-file-import").label("导入勾选的样式").primary().small().disabled(self.selected_import.is_empty())
                .on_click(cx.listener(|view, _, _, cx| {
                    view.library_request(json!({"action":"import", "path":view.document.import_path, "names":view.document.selected_import, "overwrite":view.document.overwrite_styles}), cx);
                })));
        }
        panel
    }

    pub fn render_project(&self, cx: &mut Context<Workspace>) -> impl IntoElement {
        let media = &self.details.media;
        let mut panel = div().id("project-properties").v_flex().flex_1().min_h_0().p_4().gap_3().overflow_y_scrollbar()
            .child(div().text_lg().font_semibold().child("脚本属性"))
            .child(div().h_flex().gap_2()
                .child(Button::new("apply-script-info").label("应用脚本属性").primary().small().on_click(cx.listener(|view, _, _, cx| {
                    view.invoke(json!({"op":"script_info", "fields":view.document.script_form.patch(cx)}), cx);
                })))
                .child(Button::new("discard-script-info").label("放弃修改").small().on_click(cx.listener(|view, _, _, cx| { view.document.script_form.dirty.clear(); cx.notify(); })))
                .child(Button::new("video-script-dimensions").label("使用视频分辨率和色彩矩阵").small().disabled(media.width == 0)
                    .on_click(cx.listener(|view, _, window, cx| {
                        let media = &view.document.details.media;
                        for (key, value) in [("PlayResX", media.width.to_string()), ("PlayResY", media.height.to_string()), ("LayoutResX", media.width.to_string()), ("LayoutResY", media.height.to_string()), ("YCbCr Matrix", media.matrix.clone())] {
                            view.document.script_form.inputs[key].update(cx, |state, cx| state.set_value(value, window, cx));
                            view.document.script_form.dirty.insert(key);
                        }
                        cx.notify();
                    }))))
            .child(div().text_xs().text_color(Theme::global(cx).muted_foreground).child("色彩矩阵：空、None、TV.601、PC.601、TV.709、PC.709、TV.FCC、PC.FCC、TV.240M、PC.240M。更改分辨率只更新属性；缩放排版请使用重采样工具。"))
            .child(render_fields("script-fields", SCRIPT_FIELDS, &self.script_form));
        for (name, value) in [
            ("视频", media.video.clone()),
            ("音频", media.audio.clone()),
            ("关键帧", media.keyframes.to_string()),
            ("时间码已加载", media.timecodes.to_string()),
        ] {
            panel = panel.child(div().text_sm().child(format!("{name}：{value}")));
        }
        panel
    }
}

fn render_fields(
    id: &'static str,
    fields: &'static [(&'static str, &'static str)],
    form: &Form,
) -> impl IntoElement {
    let mut container = div().id(id).flex().flex_wrap().gap_3();
    for &(key, label) in fields {
        container = container.child(
            div()
                .v_flex()
                .w(px(260.))
                .gap_1()
                .child(div().text_sm().child(label))
                .child(
                    Input::new(&form.inputs[key])
                        .id(format!("{id}-{key}"))
                        .w_full()
                        .small(),
                ),
        );
    }
    container
}
