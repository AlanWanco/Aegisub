// Release builds are GUI-only: without this Windows opens a console window for
// the front-end that the host launches in the background.
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod attachment_panel;
mod file_panel;
mod spelling_panel;
use file_panel::{FilePage, FilePanel, FileState};
use spelling_panel::{SpellState, SpellingPanel};
mod audio_panel;
use attachment_panel::{AttachmentPanel, AttachmentState};
mod automation_panel;
use automation_panel::{AutomationPanel, AutomationState};
mod command_catalog;

use audio_panel::{AudioImage, AudioPanel, AudioState};
mod document_panels;
mod hotkey_panel;
mod paste_panel;
mod preferences_panel;
use hotkey_panel::{HotkeyPanel, HotkeyState};
use preferences_panel::{PreferenceState, PreferencesPanel};
mod resample_panel;
use paste_panel::{PastePanel, PasteReply};
mod search_panel;
use search_panel::{SearchPanel, SearchReply};
mod styling_panel;
use resample_panel::{ResamplePanel, ResampleState};
mod page;
mod timing_panel;
mod timing_processor_panel;
use timing_panel::{ShiftState, TimingPanel};
use timing_processor_panel::{ProcessorState, TimingProcessorPanel};
mod translation_panel;
use styling_panel::StylingPanel;
mod video_overscan;
mod video_settings_panel;
mod video_timeline;
use translation_panel::{TranslationPanel, TranslationState};
use video_settings_panel::{VideoPage, VideoSettings, VideoSettingsPanel};
mod visual_panel;
mod workspace_theme;
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable;
use image::Frame as VideoImageFrame;
use workspace_theme::Palette;

use visual_panel::{VisualPanel, VisualReply, video_base_bounds, zoomed_video_bounds};

use video_timeline::VideoTimeline;

use document_panels::{DocumentDetails, DocumentPanels, LibraryDetails};

use command_catalog::{Catalog, populate_menu};
use std::{
    collections::BTreeMap,
    net::SocketAddr,
    ops::Range,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, anyhow};
use async_net::TcpStream;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use gpui_kit::{
    App, AppContext, Context, Corners, Entity, ExternalPaths, Focusable, Image, IntoElement,
    Render, ScrollDelta, Subscription, Window, WindowBounds, WindowOptions, canvas, div,
    prelude::*, px, rgb, size, uniform_list,
};
use gpui_kit::{
    base::{Disableable, Selectable},
    component::{
        Icon, Root, StyledExt, Theme, ThemeMode, WindowExt,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        menu::{DropdownMenu, PopupMenuItem},
        scroll::ScrollableElement,
    },
    platform::application,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, PartialEq)]
struct RunCommand {
    name: String,
}

// Native menu bars cannot carry a closure, so the menu item identifies the
// catalog command and the app-level handler forwards it to the workspace.
impl gpui_kit::Action for RunCommand {
    fn boxed_clone(&self) -> Box<dyn gpui_kit::Action> {
        Box::new(self.clone())
    }

    fn partial_eq(&self, other: &dyn gpui_kit::Action) -> bool {
        other.as_any().downcast_ref::<Self>() == Some(self)
    }

    fn name(&self) -> &'static str {
        "aegisub::RunCommand"
    }

    fn name_for_type() -> &'static str
    where
        Self: Sized,
    {
        "aegisub::RunCommand"
    }

    fn build(value: Value) -> Result<Box<dyn gpui_kit::Action>>
    where
        Self: Sized,
    {
        Ok(Box::new(Self {
            name: value
                .get("name")
                .and_then(|name| name.as_str())
                .unwrap_or_default()
                .to_owned(),
        }))
    }
}

fn dropped_kind(path: &std::path::Path) -> Option<&'static str> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "ass" | "ssa" | "srt" | "vtt" | "sub" => Some("subtitle"),
        "mp4" | "mkv" | "mov" | "avi" | "webm" | "m4v" | "mpg" | "mpeg" | "ts" => Some("video"),
        "wav" | "flac" | "mp3" | "ogg" | "m4a" | "aac" | "opus" | "wma" => Some("audio"),
        "txt" | "log" | "pass" | "stats" => Some("timecodes"),
        _ => None,
    }
}

fn menu_label(text: &str) -> String {
    let label = text.split('\t').next().unwrap_or(text).replace('&', "");
    let label = label.trim();
    match label {
        "File" => "文件",
        "Edit" => "编辑",
        "View" => "视图",
        "Subtitle" => "字幕",
        "Timing" => "时间",
        "Video" => "视频",
        "Audio" => "音频",
        "Automation" => "自动化",
        "Help" => "帮助",
        _ => label,
    }
    .to_owned()
}

fn native_menu_items(catalog: &Catalog, submenu: &str) -> Vec<gpui_kit::MenuItem> {
    let mut items = Vec::new();
    for entry in catalog.menu(submenu) {
        if !entry.submenu.is_empty() {
            let children = native_menu_items(catalog, &entry.submenu);
            if !children.is_empty() {
                items.push(gpui_kit::MenuItem::submenu(
                    gpui_kit::Menu::new(menu_label(&entry.text)).items(children),
                ));
            }
            continue;
        }
        if entry.command.is_empty() {
            if !matches!(items.last(), Some(gpui_kit::MenuItem::Separator)) {
                items.push(gpui_kit::MenuItem::separator());
            }
            continue;
        }
        let Some(command) = catalog.command(&entry.command) else {
            continue;
        };
        if !command.can_invoke() {
            continue;
        }
        let label = if entry.text.is_empty() {
            command.label.clone()
        } else {
            menu_label(&entry.text)
        };
        items.push(gpui_kit::MenuItem::action(
            label,
            RunCommand {
                name: command.name.clone(),
            },
        ));
    }
    while matches!(items.last(), Some(gpui_kit::MenuItem::Separator)) {
        items.pop();
    }
    items
}

/// macOS puts the menu bar in the system bar rather than inside the window.
fn install_native_menus(catalog: &Catalog, cx: &mut App) {
    if !cfg!(target_os = "macos") {
        return;
    }
    let mut menus = vec![gpui_kit::Menu::new("Aegisub").items(vec![
        gpui_kit::MenuItem::action(
            "关于 Aegisub",
            RunCommand {
                name: "app/about".into(),
            },
        ),
        gpui_kit::MenuItem::separator(),
        gpui_kit::MenuItem::action(
            "新建窗口",
            RunCommand {
                name: "app/new_window".into(),
            },
        ),
        gpui_kit::MenuItem::separator(),
        gpui_kit::MenuItem::action(
            "退出 Aegisub",
            RunCommand {
                name: "app/exit".into(),
            },
        ),
    ])];
    for entry in catalog.menu("main") {
        if entry.submenu.is_empty()
            || entry.submenu == "main/window"
            || entry.special == "automation"
        {
            continue;
        }
        let items = native_menu_items(catalog, &entry.submenu);
        if items.is_empty() {
            continue;
        }
        menus.push(gpui_kit::Menu::new(menu_label(&entry.text)).items(items));
    }
    cx.set_menus(menus);
}

#[derive(Clone, Debug, Deserialize)]
struct Line {
    id: i64,
    row: i64,
    start: i64,
    end: i64,
    #[serde(default)]
    start_frame: i64,
    #[serde(default)]
    end_frame: i64,
    layer: i64,
    comment: bool,
    style: String,
    actor: String,
    #[serde(default)]
    effect: String,
    #[serde(default)]
    margin: [i64; 3],
    text: String,
    #[serde(default)]
    display_text: String,
}

/// Space between subtitle list columns. Rows and the header both use it, so a
/// column boundary lands on the same x in both.
const COLUMN_GAP: f32 = 12.;

/// The subtitle text column never shrinks below this, so adding optional
/// columns squeezes the text last instead of making it disappear.
const MIN_TEXT_COLUMN: f32 = 140.;

/// One column of the subtitle list. The text column is implicit and always
/// last, so it keeps absorbing whatever width the fixed columns leave over.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SubtitleColumn {
    IdLayer,
    Time,
    StyleActor,
    Layer,
    Effect,
    Actor,
    Comment,
}

impl SubtitleColumn {
    fn label(self) -> &'static str {
        match self {
            Self::IdLayer => "行 / 层",
            Self::Time => "开始 → 结束",
            Self::StyleActor => "样式 / 角色",
            Self::Layer => "图层",
            Self::Effect => "效果",
            Self::Actor => "演员",
            Self::Comment => "类型",
        }
    }

    fn default_width(self, secondary: f32) -> f32 {
        match self {
            Self::IdLayer => (secondary * 4.2).clamp(46., 70.),
            Self::Time => (secondary * 15.5).clamp(170., 245.),
            Self::StyleActor => (secondary * 8.4).clamp(100., 145.),
            Self::Layer => 52.,
            Self::Effect => (secondary * 9.).clamp(90., 160.),
            Self::Actor => (secondary * 7.5).clamp(80., 140.),
            Self::Comment => 56.,
        }
    }

    /// Floor used when the columns have to be squeezed to fit the panel.
    fn min_width(self) -> f32 {
        match self {
            Self::IdLayer => 44.,
            Self::Time => 150.,
            Self::StyleActor => 64.,
            Self::Layer => 32.,
            Self::Effect => 52.,
            Self::Actor => 52.,
            Self::Comment => 40.,
        }
    }

    /// Columns the user can add from the header menu.
    fn optional() -> [Self; 4] {
        [Self::Layer, Self::Effect, Self::Actor, Self::Comment]
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Appearance {
    #[serde(default)]
    colors: BTreeMap<String, u32>,
    #[serde(default)]
    default_colors: Vec<String>,
    #[serde(default)]
    grid_font: String,
    #[serde(default)]
    grid_size: i64,
    #[serde(default)]
    edit_font: String,
    #[serde(default)]
    edit_size: i64,
    #[serde(default)]
    soft_line_break: bool,
    #[serde(default)]
    highlight_visible: bool,
    #[serde(default)]
    focus_grid: bool,
}

impl Appearance {
    fn color(&self, name: &str, fallback: gpui_kit::Hsla) -> gpui_kit::Hsla {
        if self.default_colors.iter().any(|key| key == name) {
            return fallback;
        }
        self.colors
            .get(name)
            .map(|color| gpui_kit::rgba(*color).into())
            .unwrap_or(fallback)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
struct PlaybackSnapshot {
    #[serde(default)]
    video: bool,
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    frame: i64,
    #[serde(default)]
    video_epoch: u64,
    #[serde(default)]
    video_time: i64,
    #[serde(default)]
    audio_playing: bool,
    #[serde(default)]
    audio_position: i64,
}

#[derive(Clone, Debug, Deserialize)]
struct Snapshot {
    #[serde(default)]
    file: String,
    #[serde(default)]
    modified: bool,
    #[serde(default)]
    can_save: bool,
    #[serde(default = "default_true")]
    dark: bool,
    #[serde(default = "default_true")]
    show_toolbar: bool,
    #[serde(default)]
    appearance: Appearance,
    #[serde(default)]
    revision: u64,
    #[serde(default)]
    details_revision: u64,
    #[serde(default)]
    video: bool,
    #[serde(default)]
    timecodes: bool,
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    audio: AudioState,
    #[serde(default)]
    frame: i64,
    #[serde(default)]
    frame_count: i64,
    #[serde(default)]
    video_time: i64,
    #[serde(default)]
    video_duration: i64,
    #[serde(default)]
    playback_rate: f64,
    #[serde(default)]
    aspect: f64,
    #[serde(default)]
    video_window_zoom: f32,
    #[serde(default)]
    video_height: f32,
    #[serde(default)]
    video_scroll: i64,
    #[serde(default)]
    video_ctrl_scroll: i64,
    #[serde(default)]
    video_shift_scroll: i64,
    #[serde(default)]
    video_overscan: bool,
    #[serde(default)]
    video_epoch: u64,
    fps: Option<f64>,
    #[serde(default = "no_active_line")]
    active: i64,
    #[serde(default)]
    selected: Vec<i64>,
    #[serde(default)]
    text_selection: [usize; 2],
    #[serde(default)]
    lines: Vec<Line>,
}

fn hotkey_key(key: &str) -> String {
    match key {
        "backspace" => "Backspace".into(),
        "delete" => "Delete".into(),
        "enter" => "Enter".into(),
        "escape" => "Escape".into(),
        "space" => "Space".into(),
        "tab" => "Tab".into(),
        "left" => "Left".into(),
        "right" => "Right".into(),
        "up" => "Up".into(),
        "down" => "Down".into(),
        "home" => "Home".into(),
        "end" => "End".into(),
        "pageup" => "PageUp".into(),
        "pagedown" => "PageDown".into(),
        "insert" => "Insert".into(),
        "minus" => "-".into(),
        "equal" => "=".into(),
        "comma" => ",".into(),
        "period" => ".".into(),
        "slash" => "/".into(),
        "backslash" => "\\".into(),
        "semicolon" => ";".into(),
        "quote" => "'".into(),
        "leftbracket" => "[".into(),
        "rightbracket" => "]".into(),
        "grave" => "`".into(),
        _ => key.to_uppercase(),
    }
}

fn default_true() -> bool {
    true
}

fn no_active_line() -> i64 {
    -1
}

#[derive(Clone, Debug, Deserialize)]
struct FrameReply {
    frame: Option<i64>,
    request: Option<u64>,
    video: Option<bool>,
    width: Option<u32>,
    height: Option<u32>,
    image: Option<String>,
    error: Option<String>,
}

#[derive(Clone)]
struct Bridge {
    endpoint: String,
    token: String,
}

impl Bridge {
    async fn call(&self, body: Value) -> Result<Value> {
        let address: SocketAddr = self
            .endpoint
            .parse()
            .context("invalid local bridge endpoint")?;
        let mut stream = TcpStream::connect(address)
            .await
            .context("Aegisub bridge is unavailable")?;
        let mut request = body;
        request["token"] = Value::String(self.token.clone());
        let mut bytes = serde_json::to_vec(&request)?;
        bytes.push(b'\n');
        stream
            .write_all(&bytes)
            .await
            .context("could not write to Aegisub bridge")?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .context("could not read Aegisub bridge response")?;
        let value: Value =
            serde_json::from_slice(&response).context("invalid Aegisub bridge response")?;
        if value.get("ok").and_then(Value::as_bool) == Some(false) {
            return Err(anyhow!(
                value
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap_or("Aegisub command failed")
                    .to_owned()
            ));
        }
        Ok(value)
    }

    async fn snapshot(&self) -> Result<Snapshot> {
        let value = self.call(json!({"op": "snapshot"})).await?;
        serde_json::from_value(value).context("invalid document snapshot")
    }

    async fn playback(&self) -> Result<PlaybackSnapshot> {
        let value = self.call(json!({"op": "playback"})).await?;
        serde_json::from_value(value).context("invalid playback snapshot")
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Panel {
    Subtitles,
    Styles,
    Project,
    Audio,
    Visual,
    Translation,
    Styling,
    Timing,
    Automation,
    Resample,
    Processor,
    Search,
    Paste,
    VideoSettings,
    Attachments,
    Preferences,
    Hotkeys,
    Files,
    Spelling,
    Commands,
}

impl Panel {
    fn label(self) -> &'static str {
        match self {
            Self::Subtitles => "字幕编辑",
            Self::Audio => "音频调时",
            Self::Visual => "视觉排版",
            Self::Translation => "翻译助手",
            Self::Styles => "样式管理",
            Self::Styling => "配样式",
            Self::Timing => "时间平移",
            Self::Processor => "时间后处理",
            Self::Resample => "重采样",
            Self::Search => "查找替换",
            Self::Spelling => "拼写检查",
            Self::Paste => "覆盖粘贴",
            Self::Project => "脚本属性",
            Self::VideoSettings => "视频设置",
            Self::Automation => "自动化脚本",
            Self::Attachments => "附件管理",
            Self::Files => "文件与恢复",
            Self::Preferences => "偏好设置",
            Self::Hotkeys => "快捷键",
            Self::Commands => "全部命令",
        }
    }

    fn icon(self) -> IconName {
        match self {
            Self::Subtitles => IconName::Captions,
            Self::Audio => IconName::AudioLines,
            Self::Visual => IconName::Frame,
            Self::Translation => IconName::Languages,
            Self::Styles => IconName::Palette,
            Self::Styling => IconName::CaseSensitive,
            Self::Timing => IconName::Clock3,
            Self::Processor => IconName::Settings2,
            Self::Resample => IconName::Maximize,
            Self::Search => IconName::Search,
            Self::Spelling => IconName::SpellCheck,
            Self::Paste => IconName::Copy,
            Self::Project => IconName::FileText,
            Self::VideoSettings => IconName::Play,
            Self::Automation => IconName::SquareTerminal,
            Self::Attachments => IconName::Paperclip,
            Self::Files => IconName::FolderOpen,
            Self::Preferences => IconName::Settings,
            Self::Hotkeys => IconName::Asterisk,
            Self::Commands => IconName::Menu,
        }
    }
}

#[derive(Clone, Copy)]
enum WorkspaceResizeDrag {
    SubtitleEditor {
        start_y: gpui_kit::Pixels,
        start_height: f32,
    },
    /// Height of the compact waveform strip.
    AudioPanel {
        start_y: gpui_kit::Pixels,
        start_height: f32,
    },
    /// Width of the video/properties column beside the subtitle list.
    ColumnSplit {
        start_x: gpui_kit::Pixels,
        start_width: f32,
    },
    /// Height of the waveform on the full audio page.
    AudioPage {
        start_y: gpui_kit::Pixels,
        start_height: f32,
    },
    /// Width of one fixed column of the subtitle list.
    SubtitleColumn {
        column: SubtitleColumn,
        start_x: gpui_kit::Pixels,
        start_width: f32,
    },
    /// Height of the video preview above the line properties.
    VideoPreview {
        start_y: gpui_kit::Pixels,
        start_height: f32,
        /// Keeps the properties card from being squeezed out of the column.
        max_height: f32,
    },
}

struct Workspace {
    bridge: Bridge,
    focus_handle: gpui_kit::FocusHandle,
    snapshot: Snapshot,
    frame_image: Option<Arc<gpui_kit::RenderImage>>,
    frame_image_frame: i64,
    frame_request_inflight: bool,
    frame_request_generation: u64,
    video_epoch: u64,
    list_scroll_handle: gpui_kit::UniformListScrollHandle,
    subtitle_filter_cache: Option<(u64, String, String, Arc<Vec<Line>>)>,
    subtitle_editor_height: f32,
    /// Fixed columns shown before the subtitle text column.
    subtitle_columns: Vec<SubtitleColumn>,
    /// User-adjusted widths; anything missing falls back to the default.
    subtitle_column_widths: Vec<(SubtitleColumn, f32)>,
    /// Measured width of the subtitle list, used to squeeze the columns.
    subtitle_grid_width: f32,
    layout_drag: Option<WorkspaceResizeDrag>,
    audio_panel_height: f32,
    audio_page_height: f32,
    column_split: Option<f32>,
    video_preview_height: Option<f32>,
    last_preview_height: std::cell::Cell<f32>,
    editor: Entity<TextareaState>,
    search: Entity<InputState>,
    command_search: Entity<InputState>,
    catalog: Arc<Catalog>,
    catalog_loading: bool,
    document: DocumentPanels,
    audio: AudioPanel,
    timeline: VideoTimeline,
    visual: VisualPanel,
    translation: TranslationPanel,
    styling: StylingPanel,
    timing: TimingPanel,
    automation: AutomationPanel,
    resample: ResamplePanel,
    processor: TimingProcessorPanel,
    find_panel: SearchPanel,
    paste: PastePanel,
    video_settings: VideoSettingsPanel,
    attachments: AttachmentPanel,
    preferences: PreferencesPanel,
    hotkeys: HotkeyPanel,
    files: FilePanel,
    spelling: SpellingPanel,
    property_inputs: BTreeMap<&'static str, Entity<InputState>>,
    properties_dirty: bool,
    properties_line_id: i64,
    _property_events: Vec<Subscription>,
    _command_search_events: Subscription,
    pending_edits: Arc<AtomicU64>,
    editor_line_id: i64,
    editor_dirty_line: i64,
    pending_text_selection: Option<[usize; 2]>,
    active_panel: Panel,
    sidebar_collapsed: Option<bool>,
    selected_line: i64,
    status: String,
    _editor_events: Subscription,
    _search_events: Subscription,
}

impl Workspace {
    fn clear_frame_image(&mut self, _cx: &mut Context<Self>) {
        self.frame_image.take();
        self.frame_image_frame = -1;
    }

    fn replace_frame_image(&mut self, image: Arc<gpui_kit::RenderImage>, frame: i64) {
        self.frame_image = Some(image);
        self.frame_image_frame = frame;
    }

    fn clear_audio_image(&mut self, cx: &mut Context<Self>) {
        if let Some(image) = self.audio.image.take() {
            image.remove_asset(cx);
        }
    }

    fn new(bridge: Bridge, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let editor = cx.new(|cx| TextareaState::new(window, cx));
        let editor_for_events = editor.clone();
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value("")
                .placeholder("筛选文本、角色或样式")
        });
        let search_events = cx.subscribe(&search, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let command_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索命令名称、说明或 ID"));
        let command_search_events =
            cx.subscribe(&command_search, |_, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            });
        let mut property_inputs = BTreeMap::new();
        let mut property_events = Vec::new();
        for key in [
            "start",
            "end",
            "layer",
            "style",
            "actor",
            "effect",
            "margin_left",
            "margin_right",
            "margin_vertical",
        ] {
            let input = cx.new(|cx| InputState::new(window, cx));
            property_events.push(cx.subscribe(&input, |view, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    view.properties_dirty = true;
                    cx.notify();
                }
            }));
            property_inputs.insert(key, input);
        }
        let document = DocumentPanels::new(window, cx);
        let timeline = VideoTimeline::new(cx);
        let translation = TranslationPanel::new(window, cx);
        let styling = StylingPanel::new(window, cx);
        let timing = TimingPanel::new(window, cx);
        let resample = ResamplePanel::new(window, cx);
        let processor = TimingProcessorPanel::new(window, cx);
        let find_panel = SearchPanel::new(window, cx);
        let video_settings = VideoSettingsPanel::new(window, cx);
        let preferences = PreferencesPanel::new(window, cx);
        let hotkeys = HotkeyPanel::new(window, cx);
        let files = FilePanel::new(window, cx);
        let spelling = SpellingPanel::new(window, cx);
        let pending_edits = Arc::new(AtomicU64::new(0));
        let editor_events = cx.subscribe(&editor, {
            let weak = cx.weak_entity();
            let bridge = bridge.clone();
            let pending_edits = pending_edits.clone();
            move |this, _, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let line_id = this.selected_line;
                let selected_ids = this.snapshot.selected.clone();
                this.editor_dirty_line = line_id;
                let epoch = pending_edits.fetch_add(1, Ordering::Relaxed) + 1;
                let editor = editor_for_events.clone();
                let pending_edits = pending_edits.clone();
                let bridge = bridge.clone();
                let weak = weak.clone();
                cx.spawn(async move |_, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(450))
                        .await;
                    if pending_edits.load(Ordering::Relaxed) != epoch || line_id < 0 {
                        return;
                    }
                    let Some(view) = weak.upgrade() else {
                        return;
                    };
                    let current_line = view.read_with(cx, |view, _| view.selected_line);
                    if current_line != line_id {
                        return;
                    }
                    let text = editor.read_with(cx, |state, _| state.value().to_string());
                    match bridge
                        .call(json!({"op":"edit", "id":line_id, "ids":selected_ids, "text":text}))
                        .await
                    {
                        Ok(_) => {
                            if let Ok(snapshot) = bridge.snapshot().await {
                                if let Some(view) = weak.upgrade() {
                                    let _ = view
                                        .update(cx, |view, cx| view.apply_snapshot(snapshot, cx));
                                }
                            }
                        }
                        Err(error) => {
                            if let Some(view) = weak.upgrade() {
                                let _ = view.update(cx, |view, cx| {
                                    view.status = format!("提交失败：{error}");
                                    cx.notify();
                                });
                            }
                        }
                    }
                })
                .detach();
            }
        });

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        Self {
            bridge,
            focus_handle,
            snapshot: Snapshot {
                file: "正在连接 Aegisub…".into(),
                modified: false,
                can_save: false,
                dark: true,
                show_toolbar: true,
                appearance: Appearance::default(),
                revision: 0,
                details_revision: 0,
                video: false,
                timecodes: false,
                playing: false,
                audio: AudioState::default(),
                frame: 0,
                frame_count: 0,
                video_time: 0,
                video_duration: 0,
                playback_rate: 1.,
                aspect: 1.,
                video_window_zoom: 1.,
                video_height: 0.,
                video_scroll: 0,
                video_ctrl_scroll: 2,
                video_shift_scroll: 4,
                video_overscan: false,
                video_epoch: 0,
                fps: None,
                active: -1,
                selected: vec![],
                text_selection: [0, 0],
                lines: vec![],
            },
            frame_image: None,
            frame_image_frame: -1,
            frame_request_inflight: false,
            frame_request_generation: 0,
            video_epoch: 0,
            list_scroll_handle: gpui_kit::UniformListScrollHandle::new(),
            subtitle_filter_cache: None,
            subtitle_editor_height: 210.,
            subtitle_columns: vec![
                SubtitleColumn::IdLayer,
                SubtitleColumn::Time,
                SubtitleColumn::StyleActor,
            ],
            subtitle_column_widths: Vec::new(),
            subtitle_grid_width: 0.,
            layout_drag: None,
            audio_panel_height: 80.,
            audio_page_height: 260.,
            column_split: None,
            video_preview_height: None,
            last_preview_height: std::cell::Cell::new(220.),
            editor,
            search,
            command_search,
            catalog: Arc::new(Catalog::default()),
            catalog_loading: false,
            document,
            audio: AudioPanel::new(),
            timeline,
            visual: VisualPanel::new(),
            translation,
            styling,
            timing,
            automation: AutomationPanel::default(),
            resample,
            processor,
            find_panel,
            paste: PastePanel::default(),
            video_settings,
            attachments: AttachmentPanel::default(),
            preferences,
            hotkeys,
            files,
            spelling,
            property_inputs,
            properties_dirty: false,
            properties_line_id: -1,
            _property_events: property_events,
            _command_search_events: command_search_events,
            pending_edits,
            editor_line_id: -1,
            editor_dirty_line: -1,
            pending_text_selection: None,
            active_panel: Panel::Subtitles,
            sidebar_collapsed: None,
            selected_line: -1,
            status: "已连接当前 Aegisub 文档".into(),
            _editor_events: editor_events,
            _search_events: search_events,
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        if snapshot.dark != Theme::global(cx).is_dark() {
            Theme::change(
                if snapshot.dark {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                },
                None,
                cx,
            );
            workspace_theme::apply(cx);
        }
        let command_state_changed = snapshot.revision != self.snapshot.revision
            || snapshot.selected != self.snapshot.selected
            || snapshot.video != self.snapshot.video
            || snapshot.timecodes != self.snapshot.timecodes
            || snapshot.show_toolbar != self.snapshot.show_toolbar
            || snapshot.video_window_zoom != self.snapshot.video_window_zoom
            || snapshot.video_overscan != self.snapshot.video_overscan;
        let document_changed = snapshot.file != self.snapshot.file;
        let details_changed = snapshot.details_revision != self.snapshot.details_revision;
        if document_changed {
            self.document.style_form.dirty.clear();
            self.document.script_form.dirty.clear();
            self.properties_dirty = false;
        }
        if snapshot.audio.render_revision != self.snapshot.audio.render_revision {
            self.audio.requested = None;
        }
        if snapshot.audio.epoch != self.snapshot.audio.epoch
            || snapshot.audio.loaded != self.snapshot.audio.loaded
        {
            self.clear_audio_image(cx);
            self.audio.requested = None;
            self.audio.gesture = None;
            self.audio.dragging = false;
        }
        let active_exists = snapshot.lines.iter().any(|line| line.id == snapshot.active);
        let selected = if active_exists { snapshot.active } else { -1 };
        let active_changed = selected != self.selected_line;
        if snapshot.audio.autoscroll
            && (active_changed || snapshot.audio.epoch != self.snapshot.audio.epoch)
        {
            self.audio.view_start = (snapshot.audio.start - 1000).max(0);
        }
        if snapshot.audio.spectrum != self.snapshot.audio.spectrum {
            self.audio.mode = if snapshot.audio.spectrum {
                "spectrum"
            } else {
                "waveform"
            };
        }
        self.selected_line = selected;
        let media_changed = snapshot.video_epoch != self.video_epoch
            || snapshot.video != self.snapshot.video
            || snapshot.aspect != self.snapshot.aspect;
        if media_changed || snapshot.video_window_zoom != self.snapshot.video_window_zoom {
            self.cancel_visual_drag(cx);
            self.visual.reset_zoom();
            self.visual.fit_video = false;
        }
        if active_changed {
            self.cancel_visual_drag(cx);
        }
        if media_changed {
            self.visual.queue.clear();
            self.visual.commands.clear();
            self.visual.drawn = None;
            self.visual.dragging = false;
            self.visual.bounds.replace(gpui_kit::Bounds::default());
            self.visual
                .canvas_bounds
                .replace(gpui_kit::Bounds::default());
        }
        if snapshot.revision != self.snapshot.revision {
            self.subtitle_filter_cache = None;
        }
        if media_changed || snapshot.revision != self.snapshot.revision {
            self.clear_frame_image(cx);
            self.frame_request_generation += 1;
            self.frame_request_inflight = false;
        }
        self.video_epoch = snapshot.video_epoch;
        // Keep the last complete frame visible until its replacement arrives.
        // Clearing it here makes playback flash whenever the bridge is late.
        if active_changed {
            self.editor_line_id = -1;
            self.editor_dirty_line = -1;
        } else if self.editor_dirty_line == selected {
            if let Some(line) = snapshot.lines.iter().find(|line| line.id == selected) {
                if self.editor.read(cx).value().as_ref() == line.text {
                    self.editor_dirty_line = -1;
                }
            }
        }
        if !snapshot.video {
            self.clear_frame_image(cx);
            self.frame_request_generation += 1;
            self.frame_request_inflight = false;
        }
        self.snapshot = snapshot;
        if command_state_changed {
            self.refresh_catalog(cx);
        }
        if details_changed || document_changed || media_changed {
            self.refresh_document(cx);
        }
        if matches!(self.active_panel, Panel::Audio | Panel::Subtitles) {
            self.refresh_audio(cx);
        }
        if self.active_panel == Panel::Attachments
            && self.snapshot.revision != self.attachments.state.revision
        {
            self.attachment_request(json!({"action":"state"}), cx);
        }
        let visual_state = (
            self.snapshot.revision,
            self.snapshot.frame,
            self.snapshot.active,
            self.video_epoch as i64,
        );
        if self.active_panel == Panel::Visual && self.visual.drawn != Some(visual_state) {
            self.visual.drawn = Some(visual_state);
            self.queue_visual(json!({"action":"draw"}), cx);
        }
        cx.notify();
    }

    fn apply_playback(&mut self, playback: PlaybackSnapshot, cx: &mut Context<Self>) {
        if self.snapshot.video != playback.video || self.video_epoch != playback.video_epoch {
            return;
        }
        self.snapshot.frame = playback.frame;
        self.snapshot.video_time = playback.video_time;
        self.snapshot.playing = playback.playing;
        self.snapshot.audio.playing = playback.audio_playing;
        self.snapshot.audio.position = playback.audio_position;
        cx.notify();
    }

    fn select_lines(&mut self, id: i64, extend: bool, toggle: bool, cx: &mut Context<Self>) {
        let mut ids = if toggle {
            self.snapshot.selected.clone()
        } else {
            vec![]
        };
        if extend {
            if let (Some(anchor), Some(target)) = (
                self.snapshot
                    .lines
                    .iter()
                    .position(|line| line.id == self.selected_line),
                self.snapshot.lines.iter().position(|line| line.id == id),
            ) {
                ids = self.snapshot.lines[anchor.min(target)..=anchor.max(target)]
                    .iter()
                    .map(|line| line.id)
                    .collect();
            }
        } else if toggle {
            if ids.contains(&id) {
                ids.retain(|selected| *selected != id);
            } else {
                ids.push(id);
            }
        } else {
            ids.push(id);
        }
        // Keep an active row when removing one row from a multi-selection.
        let active = if ids.contains(&id) {
            id
        } else if ids.is_empty() {
            ids.push(id);
            id
        } else if ids.contains(&self.selected_line) {
            self.selected_line
        } else {
            ids[0]
        };
        self.invoke(json!({"op":"select", "id":active, "ids":ids}), cx);
    }

    /// Classify dropped files the same way the open dialogs do, so dragging a
    /// subtitle, video, audio or timecode file onto the window opens it.
    fn open_dropped(&mut self, paths: &ExternalPaths, cx: &mut Context<Self>) {
        let mut opened = Vec::new();
        for path in paths.paths() {
            let Some(kind) = dropped_kind(path) else {
                continue;
            };
            // Only one subtitle can be the active document; media files add to it.
            if kind == "subtitle" && opened.iter().any(|(kind, _)| *kind == "subtitle") {
                continue;
            }
            opened.push((kind, path.to_string_lossy().to_string()));
        }
        if opened.is_empty() {
            self.status = "拖入的文件类型不受支持：可用字幕、视频、音频或时间码".into();
            cx.notify();
            return;
        }
        for (kind, path) in opened {
            let label = match kind {
                "subtitle" => "字幕",
                "video" => "视频",
                "audio" => "音频",
                _ => "时间码",
            };
            self.status = format!("已拖入{label}：{path}");
            self.invoke(json!({"op":"open", "kind":kind, "path":path}), cx);
        }
        cx.notify();
    }

    fn pick_file(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let dialog = rfd::AsyncFileDialog::new();
            let dialog = match kind {
                "subtitle" => dialog.add_filter("字幕文件", &["ass", "ssa", "srt", "vtt"]),
                "timecodes" => dialog.add_filter("时间码文件", &["txt"]),
                "keyframes" => dialog.add_filter("关键帧文件", &["txt", "log", "stats", "pass"]),
                "audio" => dialog.add_filter(
                    "音频文件",
                    &["wav", "flac", "mp3", "ogg", "m4a", "mp4", "mkv"],
                ),
                _ => dialog.add_filter("视频文件", &["mp4", "mkv", "mov", "avi", "webm"]),
            };
            if let Some(file) = dialog.pick_file().await {
                let _ = weak.update(cx, |view, cx| {
                    view.invoke(
                        json!({"op":"open", "kind":kind, "path":file.path().to_string_lossy()}),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    fn save_file_as(&mut self, cx: &mut Context<Self>) {
        let filename = std::path::Path::new(&self.snapshot.file)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("untitled");
        let filename = format!("{filename}.ass");
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_file_name(filename)
                .add_filter("ASS 字幕", &["ass"])
                .save_file()
                .await
            {
                let _ = weak.update(cx, |view, cx| {
                    view.invoke(
                        json!({"op":"save_as", "path":file.path().to_string_lossy()}),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    fn save_media_file(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let stem = std::path::Path::new(&self.snapshot.file)
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled");
        let (extension, label, title) = match kind {
            "audio_clip" => ("wav", "WAV 音频", "导出所选字幕范围的音频"),
            "keyframes" => ("key.txt", "关键帧文本", "保存关键帧"),
            _ => ("timecodes.txt", "VFR 时间码", "保存时间码"),
        };
        let filename = format!("{stem}.{extension}");
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new().set_title(title).set_file_name(filename).add_filter(label, &[if kind == "audio_clip" {"wav"} else {"txt"}]).save_file().await {
                let _ = weak.update(cx, |view, cx| {
                    view.invoke(json!({"op":"save_media","kind":kind,"path":file.path().to_string_lossy(),"overwrite":true}), cx);
                });
            }
        }).detach();
    }

    /// 新建窗口：同一份文档的第二个窗口，各自持有自己的桥接连接。
    fn open_new_window(&mut self, cx: &mut Context<Self>) {
        // Cascade the new window: offset it and shrink it a little, otherwise every
        // window ends up centered on exactly the same spot.
        let cascade = cx.windows().len() as f32;
        let bounds = cx.primary_display().map(|display| {
            let screen = display.bounds();
            let step = px(32.) * cascade;
            let width = (screen.size.width - step * 2. - px(64.))
                .min(px(1420.))
                .max(px(880.));
            let height = (screen.size.height - step * 2. - px(64.))
                .min(px(920.))
                .max(px(560.));
            gpui_kit::Bounds {
                origin: gpui_kit::point(
                    screen.origin.x + (screen.size.width - width) / 2. + step,
                    screen.origin.y + (screen.size.height - height) / 2. + step,
                ),
                size: size(width, height),
            }
        });
        if let Err(error) = open_workspace(cx, self.bridge.clone(), bounds) {
            eprintln!("Could not open another GPUI window: {error:#}");
            self.status = format!("无法打开新窗口：{error}");
            cx.notify();
        }
    }

    fn invoke(&mut self, mut payload: Value, cx: &mut Context<Self>) {
        if payload["op"] == "command" {
            match payload["name"].as_str() {
                // These two belong to the front end: the host would answer them
                // by opening its own wx window or frame.
                Some("app/exit") => {
                    cx.quit();
                    return;
                }
                Some("app/new_window") => {
                    self.open_new_window(cx);
                    return;
                }
                Some(name @ ("audio/save/clip" | "keyframe/save" | "timecode/save")) => {
                    self.save_media_file(
                        match name {
                            "audio/save/clip" => "audio_clip",
                            "keyframe/save" => "keyframes",
                            _ => "timecodes",
                        },
                        cx,
                    );
                    return;
                }
                Some("video/reset_pan") => {
                    self.cancel_visual_drag(cx);
                    self.visual.reset_zoom();
                    cx.notify();
                    return;
                }
                Some(name) if name.starts_with("video/zoom/") => {
                    self.cancel_visual_drag(cx);
                    self.visual.fit_video = false;
                    self.visual.reset_zoom();
                }
                Some("subtitle/spellcheck") => {
                    self.active_panel = Panel::Spelling;
                    self.spelling_request(json!({"action":"state"}), cx);
                    return;
                }
                Some("app/toggle/toolbar") => {
                    self.preferences_request(json!({"action":"apply","fields":{"App/Show Toolbar":if self.snapshot.show_toolbar {"0"} else {"1"}}}), cx);
                    return;
                }
                Some("app/options") => {
                    self.active_panel = Panel::Preferences;
                    self.preferences_request(json!({"action":"state"}), cx);
                    return;
                }
                Some("subtitle/attachment") => {
                    self.active_panel = Panel::Attachments;
                    self.attachment_request(json!({"action":"state"}), cx);
                    return;
                }
                Some("subtitle/open/charset") => {
                    self.active_panel = Panel::Files;
                    self.files.page = FilePage::Encoding;
                    self.file_tools_request(json!({"action":"state"}), cx);
                    self.pick_encoded_subtitle(cx);
                    return;
                }
                Some("subtitle/open/autosave") => {
                    self.active_panel = Panel::Files;
                    self.files.page = FilePage::Recovery;
                    self.file_tools_request(json!({"action":"state"}), cx);
                    return;
                }
                Some("subtitle/open") => {
                    self.pick_file("subtitle", cx);
                    return;
                }
                Some(
                    name @ ("video/open/dummy"
                    | "video/jump"
                    | "video/aspect/custom"
                    | "video/details"),
                ) => {
                    self.active_panel = Panel::VideoSettings;
                    self.video_settings.page = match name {
                        "video/open/dummy" => VideoPage::Dummy,
                        "video/jump" => VideoPage::Jump,
                        "video/aspect/custom" => VideoPage::Aspect,
                        _ => VideoPage::Details,
                    };
                    self.video_settings.initialized = false;
                    self.video_settings_request("state", cx);
                    return;
                }
                Some("video/open") => {
                    self.pick_file("video", cx);
                    return;
                }
                Some("audio/open") => {
                    self.pick_file("audio", cx);
                    return;
                }
                Some("keyframe/open") => {
                    self.pick_file("keyframes", cx);
                    return;
                }
                Some("timecode/open") => {
                    self.pick_file("timecodes", cx);
                    return;
                }
                Some("subtitle/save/as") => {
                    self.save_file_as(cx);
                    return;
                }
                Some("subtitle/save") if !self.snapshot.can_save => {
                    self.save_file_as(cx);
                    return;
                }
                Some(name)
                    if name.starts_with("audio/go_to") || name.starts_with("audio/scroll/") =>
                {
                    self.active_panel = Panel::Audio;
                    let state = &self.snapshot.audio;
                    let start = match name {
                        "audio/go_to/start" => state.start - self.audio.span / 2,
                        "audio/go_to/end" => state.end - self.audio.span / 2,
                        "audio/scroll/left" => {
                            self.audio.view_start
                                - (self.audio.span * 128
                                    / f32::from(self.audio.bounds.borrow().size.width).max(1.)
                                        as i64)
                        }
                        "audio/scroll/right" => {
                            self.audio.view_start
                                + (self.audio.span * 128
                                    / f32::from(self.audio.bounds.borrow().size.width).max(1.)
                                        as i64)
                        }
                        _ => (state.start + state.end - self.audio.span) / 2,
                    };
                    self.audio.view_start =
                        start.clamp(0, (state.duration - self.audio.span).max(0));
                    self.refresh_audio(cx);
                    cx.notify();
                    return;
                }
                Some("am/manager" | "am/meta") => {
                    self.active_panel = Panel::Automation;
                    self.automation_request(json!({"action":"state"}), cx);
                    return;
                }
                Some("am/reload" | "am/reload/autoload") => {
                    let action = if payload["name"] == "am/reload" {
                        "reload_all"
                    } else {
                        "rescan"
                    };
                    self.automation_request(json!({"action":action}), cx);
                    return;
                }
                Some("edit/line/paste/over") => {
                    self.active_panel = Panel::Paste;
                    self.paste_request(false, cx);
                    return;
                }
                Some("tool/line/select") => {
                    self.open_search(true, cx);
                    return;
                }
                Some("subtitle/find" | "edit/find_replace") => {
                    self.open_search(false, cx);
                    return;
                }
                Some("subtitle/find/next") => {
                    if self.find_panel.initialized
                        && !self.find_panel.find.read(cx).value().is_empty()
                    {
                        self.search_request("find", cx);
                    } else {
                        self.open_search(false, cx);
                    }
                    return;
                }
                Some("tool/time/postprocess") => {
                    self.active_panel = Panel::Processor;
                    self.processor_request("state", cx);
                    return;
                }
                Some("tool/resampleres") => {
                    self.active_panel = Panel::Resample;
                    self.resample_request("state", cx);
                    return;
                }
                Some("time/shift") => {
                    self.active_panel = Panel::Timing;
                    self.shift_times_request("state", cx);
                    return;
                }
                Some("tool/style/assistant") => {
                    self.active_panel = Panel::Styling;
                    self.styling.line_id = -1;
                    self.styling.focus_pending = true;
                    self.refresh_document(cx);
                    self.styling_action("start", None, cx);
                    return;
                }
                Some("tool/styling_assistant/commit") => {
                    self.styling_action("commit", None, cx);
                    return;
                }
                Some("tool/styling_assistant/preview") => {
                    self.styling_action("preview", None, cx);
                    return;
                }
                Some("tool/translation_assistant") => {
                    self.active_panel = Panel::Translation;
                    self.translation_action("start", cx);
                    return;
                }
                Some(name) if name.starts_with("tool/translation_assistant/") => {
                    let action = match name.rsplit('/').next().unwrap_or_default() {
                        "commit" => "commit",
                        "preview" => "preview",
                        "next" => "next",
                        "prev" => "prev",
                        _ => "insert_original",
                    };
                    self.translation_action(action, cx);
                    return;
                }
                Some("tool/style/manager") => {
                    self.active_panel = Panel::Styles;
                    self.refresh_document(cx);
                    self.refresh_library(cx);
                    cx.notify();
                    return;
                }
                Some("subtitle/properties") => {
                    self.active_panel = Panel::Project;
                    self.refresh_document(cx);
                    cx.notify();
                    return;
                }
                _ => {}
            }
        }
        if payload["op"] == "command" && self.selected_line >= 0 {
            let range = self.editor.read(cx).selected_range();
            payload["text_selection"] = json!([range.start, range.end]);
        }
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let draft = self.pending_draft(cx);
        cx.spawn(async move |_, cx| {
            if let Some(draft) = draft {
                if let Err(error) = bridge.call(draft).await {
                    set_status(&weak, cx, format!("字幕提交失败：{error}"));
                    return;
                }
            }
            let succeeded = invoke_weak(&bridge, &weak, cx, payload.clone()).await;
            if let Some(view) = weak.upgrade() {
                let _ = view.update(cx, |view, cx| {
                    if succeeded {
                        match payload["op"].as_str() {
                            Some("script_info") => view
                                .document
                                .script_form
                                .acknowledge(&payload["fields"], cx),
                            Some("style") => {
                                view.document.style_form.acknowledge(&payload["fields"], cx);
                                if let Some(name) = payload["fields"]["name"].as_str() {
                                    view.document.style = name.into();
                                }
                            }
                            Some("styling") => {
                                view.styling.pending_value = None;
                                view.styling.line_id = -1;
                                view.styling.focus_pending = true;
                            }
                            Some("open")
                                if payload["kind"] == "subtitle"
                                    && view.active_panel == Panel::Files =>
                            {
                                view.active_panel = Panel::Subtitles;
                            }
                            Some("save_media") => {
                                view.status = format!(
                                    "已保存：{}",
                                    payload["path"].as_str().unwrap_or_default()
                                )
                            }
                            Some("line_fields") => view.properties_dirty = false,
                            Some("command") => {
                                view.pending_text_selection = Some(view.snapshot.text_selection);
                                if let Some(name) = payload["name"]
                                    .as_str()
                                    .and_then(|name| name.strip_prefix("video/tool/"))
                                {
                                    let (mode, subtool) = match name {
                                        "cross" => ("cross", None),
                                        "drag" => ("drag", None),
                                        "rotate/z" => ("rotate/z", None),
                                        "rotate/xy" => ("rotate/xy", None),
                                        "scale" => ("scale", None),
                                        "clip" => ("clip", None),
                                        "vclip/drag" => ("vector_clip", Some(0)),
                                        "vclip/line" => ("vector_clip", Some(1)),
                                        "vclip/bicubic" => ("vector_clip", Some(2)),
                                        "vclip/convert" => ("vector_clip", Some(3)),
                                        "vclip/insert" => ("vector_clip", Some(4)),
                                        "vclip/remove" => ("vector_clip", Some(5)),
                                        "vclip/freehand" => ("vector_clip", Some(6)),
                                        "vclip/freehand_smooth" => ("vector_clip", Some(7)),
                                        _ => ("vector_clip", None),
                                    };
                                    view.select_visual_tool(mode, subtool, cx);
                                }
                                cx.notify();
                            }
                            _ => {}
                        }
                    }
                    view.refresh_catalog(cx);
                    view.refresh_document(cx);
                    if view.active_panel == Panel::Attachments {
                        view.attachment_request(json!({"action":"state"}), cx);
                    }
                    if view.active_panel == Panel::Processor {
                        view.processor_request("state", cx);
                    }
                    if view.active_panel == Panel::Spelling {
                        view.spelling_request(json!({"action":"state"}), cx);
                    }
                });
            }
        })
        .detach();
    }

    fn pending_draft(&mut self, cx: &mut Context<Self>) -> Option<Value> {
        if self.editor_dirty_line != self.selected_line || self.selected_line < 0 {
            return None;
        }
        self.pending_edits.fetch_add(1, Ordering::Relaxed);
        Some(
            json!({"op":"edit", "id":self.selected_line, "ids":self.snapshot.selected, "text":self.editor.read(cx).value().to_string()}),
        )
    }

    fn flush_video_seek(&mut self, cx: &mut Context<Self>) {
        if self.timeline.seeking || !self.snapshot.video {
            return;
        }
        let Some(frame) = self.timeline.pending_frame.take() else {
            return;
        };
        self.timeline.seeking = true;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let succeeded =
                invoke_weak(&bridge, &weak, cx, json!({"op":"seek", "frame":frame})).await;
            let _ = weak.update(cx, |view, cx| {
                view.timeline.seeking = false;
                if succeeded {
                    view.refresh_frame(cx);
                }
                view.flush_video_seek(cx);
            });
        })
        .detach();
    }

    fn select_visual_tool(
        &mut self,
        mode: &'static str,
        subtool: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        self.active_panel = Panel::Visual;
        self.visual.bounds.replace(gpui_kit::Bounds::default());
        self.visual
            .canvas_bounds
            .replace(gpui_kit::Bounds::default());
        self.visual.tool = mode;
        if let Some(subtool) = subtool {
            self.visual.subtool = subtool;
        }
        self.visual.queue.clear();
        self.visual.commands.clear();
        self.visual.drawn = None;
        self.visual.dragging = false;
        cx.notify();
    }

    fn queue_visual(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if !self.snapshot.video {
            return;
        }
        let bounds = *self.visual.canvas_bounds.borrow();
        if bounds.size.width < px(16.) || bounds.size.height < px(16.) {
            return;
        }
        request["op"] = json!("visual");
        request["tool"] = json!(self.visual.tool);
        request["width"] = json!(f32::from(bounds.size.width).round() as i64);
        request["height"] = json!(f32::from(bounds.size.height).round() as i64);
        if self.visual.tool == "vector_clip" {
            request["subtool"] = json!(self.visual.subtool);
        }
        let replace = self.visual.queue.back().is_some_and(|previous| {
            previous["action"] == request["action"]
                && matches!(request["action"].as_str(), Some("move" | "draw"))
        });
        if replace {
            self.visual.queue.pop_back();
        }
        self.visual.queue.push_back(request);
        self.flush_visual(cx);
    }

    fn flush_visual(&mut self, cx: &mut Context<Self>) {
        if self.visual.loading {
            return;
        }
        let Some(request) = self.visual.queue.pop_front() else {
            return;
        };
        self.visual.loading = true;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let changes = matches!(
            request["action"].as_str(),
            Some("down" | "double" | "toggle_move" | "up")
        ) || self.visual.dragging;
        let request_epoch = self.video_epoch;
        let refresh_catalog = request["action"] == "draw";
        cx.spawn(async move |_, cx| {
            let result = bridge.call(request).await.and_then(|value| {
                serde_json::from_value::<VisualReply>(value).context("invalid visual drawing")
            });
            let snapshot = if changes && result.is_ok() {
                bridge.snapshot().await.ok()
            } else {
                None
            };
            let _ = weak.update(cx, |view, cx| {
                view.visual.loading = false;
                match result {
                    Ok(reply) => {
                        let bounds = *view.visual.canvas_bounds.borrow();
                        if view.video_epoch == request_epoch
                            && reply.tool == view.visual.tool
                            && reply.width == f32::from(bounds.size.width).round() as i64
                            && reply.height == f32::from(bounds.size.height).round() as i64
                            && (reply.tool != "vector_clip" || reply.subtool == view.visual.subtool)
                        {
                            view.visual.commands = reply.commands;
                        }
                    }
                    Err(error) => view.status = format!("视觉工具失败：{error}"),
                }
                if let Some(snapshot) = snapshot {
                    view.apply_snapshot(snapshot, cx);
                    view.refresh_frame(cx);
                }
                if refresh_catalog {
                    view.refresh_catalog(cx);
                }
                view.flush_visual(cx);
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_audio(&mut self, cx: &mut Context<Self>) {
        if !self.snapshot.audio.loaded || self.audio.loading {
            return;
        }
        let request = (
            self.snapshot.audio.epoch,
            self.audio.view_start,
            self.audio.span,
            self.audio.mode,
            self.audio.gain,
            self.snapshot.audio.decoded,
            self.snapshot.audio.render_revision,
        );
        if self.audio.requested == Some(request) {
            return;
        }
        self.audio.loading = true;
        self.audio.requested = Some(request);
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge.call(json!({"op":"audio_render", "start":request.1, "span":request.2, "mode":request.3, "gain":request.4, "width":1400, "height":260})).await
                .and_then(|value| serde_json::from_value::<AudioImage>(value).context("invalid audio display"));
            let result = result.and_then(|reply| {
                let image = reply.image.map(|encoded| base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)).transpose()?;
                Ok((reply.state, reply.view_start, image.map(|bytes| Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, bytes)))))
            });
            let _ = weak.update(cx, |view, cx| {
                view.audio.loading = false;
                match result {
                    Ok((state, start, image)) if state.epoch == view.snapshot.audio.epoch
                        && state.render_revision == view.snapshot.audio.render_revision
                        && request.1 == view.audio.view_start && request.2 == view.audio.span
                        && request.3 == view.audio.mode && request.4 == view.audio.gain => {
                        if let Some(image) = image {
                            if let Some(previous) = view.audio.image.replace(image) {
                                previous.remove_asset(cx);
                            }
                        }
                        let _ = start;
                    }
                    Ok(_) => {},
                    Err(error) => { view.status = format!("音频画面生成失败：{error}"); view.audio.requested = None; }
                }
                if view.audio.requested.is_some() { view.refresh_audio(cx); }
                cx.notify();
            });
        }).detach();
    }

    fn begin_audio_drag(
        &mut self,
        ms: i64,
        right: bool,
        control: bool,
        alt: bool,
        cx: &mut Context<Self>,
    ) {
        if self.audio.drag_loading {
            return;
        }
        self.audio.dragging = true;
        self.audio.finish_drag = false;
        self.audio.pending_ms = None;
        self.audio.drag_loading = true;
        let sensitivity = (self.audio.span * self.snapshot.audio.drag_sensitivity
            / f32::from(self.audio.bounds.borrow().size.width).max(1.) as i64)
            .clamp(1, 10000);
        let snap = if self.snapshot.audio.snap {
            (self.audio.span * self.snapshot.audio.snap_distance
                / f32::from(self.audio.bounds.borrow().size.width).max(1.) as i64)
                .clamp(0, 10000)
        } else {
            0
        };
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge.call(json!({"op":"audio_action", "action":"down", "ms":ms, "right":right, "control":control, "alt":alt, "sensitivity":sensitivity, "snap":snap})).await;
            let _ = weak.update(cx, |view, cx| {
                view.audio.drag_loading = false;
                match result {
                    Ok(reply) => {
                        view.audio.gesture = reply["gesture"].as_u64();
                        if let Ok(state) = serde_json::from_value::<AudioState>(reply) { view.snapshot.audio = state; }
                        view.flush_audio_drag(cx);
                    }
                    Err(error) => { view.audio.dragging = false; view.status = format!("音频选区失败：{error}"); }
                }
                cx.notify();
            });
        }).detach();
    }

    fn flush_audio_drag(&mut self, cx: &mut Context<Self>) {
        if self.audio.drag_loading {
            return;
        }
        let (Some(gesture), Some(ms)) = (self.audio.gesture, self.audio.pending_ms.take()) else {
            return;
        };
        self.audio.drag_loading = true;
        let finished = self.audio.finish_drag;
        if finished {
            self.audio.dragging = false;
        }
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let snap = if self.snapshot.audio.snap {
            (self.audio.span * self.snapshot.audio.snap_distance
                / f32::from(self.audio.bounds.borrow().size.width).max(1.) as i64)
                .clamp(0, 10000)
        } else {
            0
        };
        cx.spawn(async move |_, cx| {
            let result = bridge.call(json!({"op":"audio_action", "action":if finished {"up"} else {"drag"}, "ms":ms, "gesture":gesture, "snap":snap})).await;
            let _ = weak.update(cx, |view, cx| {
                view.audio.drag_loading = false;
                match result {
                    Ok(reply) => { if let Ok(state) = serde_json::from_value::<AudioState>(reply) { view.snapshot.audio = state; } }
                    Err(error) => { view.audio.dragging = false; view.status = format!("音频拖动失败：{error}"); }
                }
                if finished { view.audio.gesture = None; }
                else { view.flush_audio_drag(cx); }
                cx.notify();
            });
        }).detach();
    }

    fn spelling_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.spelling.loading {
            return;
        }
        let action = request["action"].as_str().unwrap_or("state").to_owned();
        request["op"] = json!("spelling");
        if action == "configure" {
            for (key, value) in [
                ("language", json!(self.spelling.state.language)),
                ("skip_comments", json!(self.spelling.state.skip_comments)),
                ("skip_uppercase", json!(self.spelling.state.skip_uppercase)),
            ] {
                if request.get(key).is_none() {
                    request[key] = value;
                }
            }
        }
        if matches!(
            action.as_str(),
            "next" | "ignore_all" | "replace" | "replace_all" | "add"
        ) {
            request["id"] = json!(self.spelling.state.id);
            request["start"] = json!(self.spelling.state.start);
            request["expected"] = json!(self.spelling.state.word);
            request["replacement"] = json!(self.spelling.replacement.read(cx).value().to_string());
        }
        let draft = if action == "state" {
            None
        } else {
            self.pending_draft(cx)
        };
        self.spelling.loading = true;
        self.spelling.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<SpellState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.spelling.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.status = if state.changed > 0 {
                            format!("已替换 {} 处，可撤销", state.changed)
                        } else {
                            "拼写检查已同步".into()
                        };
                        view.spelling.dictionary_checked = action == "state"
                            && state.dictionary_word == view.spelling.dictionary_word;
                        view.spelling.receive(state, action != "state");
                        view.spelling.confirm_remove = false;
                        if action == "remove" {
                            view.spelling.dictionary_checked = false;
                            view.spelling.dictionary_word.clear();
                            view.spelling.state.can_remove = false;
                        }
                        view.apply_snapshot(snapshot, cx);
                        view.pending_text_selection = Some(view.snapshot.text_selection);
                    }
                    Err(error) => {
                        view.spelling.error = Some(error.to_string());
                        view.status = format!("拼写检查失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn pick_encoded_subtitle(&mut self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("选择要指定编码的字幕")
                .add_filter("字幕文件", &["ass", "ssa", "srt", "txt", "vtt"])
                .pick_file()
                .await
            {
                let _ = weak.update(cx, |view, cx| {
                    view.file_tools_request(
                        json!({"action":"inspect", "path":file.path().to_string_lossy()}),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    fn file_tools_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.files.loading {
            return;
        }
        request["op"] = json!("file_tools");
        self.files.loading = true;
        self.files.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge.call(request).await.and_then(|value| {
                serde_json::from_value::<FileState>(value).context("invalid file tool state")
            });
            let _ = weak.update(cx, |view, cx| {
                view.files.loading = false;
                match result {
                    Ok(state) => view.files.receive(state),
                    Err(error) => view.files.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_file_tools_selection(&mut self, cx: &mut Context<Self>) {
        let (path, encoding, recovery) = if self.files.page == FilePage::Encoding {
            (self.files.path.clone(), self.files.encoding.clone(), false)
        } else {
            (self.files.version.clone(), String::new(), true)
        };
        if path.is_empty() {
            return;
        }
        self.invoke(
            json!({"op":"open", "kind":"subtitle", "path":path, "encoding":encoding, "recovery":recovery}),
            cx,
        );
    }

    fn pick_preference_directory(&mut self, key: String, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(directory) = rfd::AsyncFileDialog::new().pick_folder().await {
                let _ = weak.update(cx, |view, cx| {
                    view.preferences
                        .drafts
                        .insert(key.clone(), directory.path().to_string_lossy().into_owned());
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn preferences_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.preferences.loading {
            return;
        }
        request["op"] = json!("preferences");
        self.preferences.loading = true;
        self.preferences.error = None;
        let sent = request["fields"].clone();
        let reset = request["names"].clone();
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                let state = serde_json::from_value::<PreferenceState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.preferences.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        if let Some(names) = reset.as_array() {
                            for name in names {
                                if let Some(name) = name.as_str() {
                                    view.preferences.drafts.remove(name);
                                }
                            }
                        }
                        view.preferences.receive(state, &sent);
                        view.preferences.confirm_reset = false;
                        view.audio.requested = None;
                        view.visual.drawn = None;
                        view.apply_snapshot(snapshot, cx);
                        view.refresh_catalog(cx);
                        view.status = "偏好设置已同步".into();
                    }
                    Err(error) => {
                        view.preferences.error = Some(error.to_string());
                        view.status = format!("设置未应用：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn hotkey_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.hotkeys.loading {
            return;
        }
        request["op"] = json!("hotkeys");
        self.hotkeys.loading = true;
        self.hotkeys.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge.call(request).await.and_then(|state| {
                serde_json::from_value::<HotkeyState>(state).context("invalid hotkey response")
            });
            let _ = weak.update(cx, |view, cx| {
                view.hotkeys.loading = false;
                match result {
                    Ok(state) => {
                        view.hotkeys.bindings = state.bindings;
                        view.hotkeys.confirm_reset = false;
                        view.hotkeys.replace = false;
                        view.refresh_catalog(cx);
                        view.status = "快捷键已同步".into();
                    }
                    Err(error) => {
                        view.hotkeys.error = Some(error.to_string());
                        view.status = format!("快捷键未应用：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn pick_attachments(&mut self, kind: &'static str, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let dialog = rfd::AsyncFileDialog::new();
            let dialog = if kind == "font" {
                dialog.add_filter("字体文件", &["ttf", "ttc", "otf", "pfb"])
            } else {
                dialog.add_filter(
                    "图像文件",
                    &["png", "jpg", "jpeg", "bmp", "gif", "ico", "wmf"],
                )
            };
            if let Some(files) = dialog.pick_files().await {
                let paths: Vec<_> = files
                    .iter()
                    .map(|file| file.path().to_string_lossy().into_owned())
                    .collect();
                let _ = weak.update(cx, |view, cx| {
                    view.attachment_request(json!({"action":"add","group":kind,"paths":paths}), cx)
                });
            }
        })
        .detach();
    }

    fn extract_attachments(&mut self, cx: &mut Context<Self>) {
        let items = self.attachments.selected_items();
        if items.is_empty() {
            return;
        }
        let revision = self.attachments.state.revision;
        let file = self
            .attachments
            .state
            .items
            .iter()
            .find(|item| self.attachments.selected.contains(&item.index))
            .unwrap();
        let name = if file.safe_name {
            file.filename.clone()
        } else {
            "attachment".into()
        };
        let directory = items.len() > 1;
        let names: Vec<_> = self
            .attachments
            .state
            .items
            .iter()
            .filter(|item| self.attachments.selected.contains(&item.index))
            .map(|item| item.filename.clone())
            .collect();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let path = if directory { rfd::AsyncFileDialog::new().pick_folder().await.map(|file| file.path().to_owned()) }
            else { rfd::AsyncFileDialog::new().set_file_name(name).save_file().await.map(|file| file.path().to_owned()) };
            if let Some(path) = path {
                let exists = if directory { names.iter().any(|name| path.join(name).exists()) } else { path.exists() };
                let overwrite = if exists {
                    rfd::AsyncMessageDialog::new().set_title("替换已有附件文件").set_description("目标文件已存在，是否替换所选附件对应的文件？").set_buttons(rfd::MessageButtons::YesNo).show().await == rfd::MessageDialogResult::Yes
                } else { false };
                if exists && !overwrite { return; }
                let _ = weak.update(cx, |view, cx| view.attachment_request(json!({"action":"extract","items":items,"revision":revision,"path":path.to_string_lossy(),"directory":directory,"overwrite":overwrite}), cx));
            }
        }).detach();
    }

    fn attachment_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.attachments.loading {
            return;
        }
        request["op"] = json!("attachments");
        self.attachments.loading = true;
        self.attachments.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let draft = if request["action"] == "state" {
            self.pending_draft(cx)
        } else {
            None
        };
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<AttachmentState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.attachments.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.attachments.receive(state);
                        view.apply_snapshot(snapshot, cx);
                        view.status = "附件已同步".into();
                    }
                    Err(error) => {
                        view.attachments.error = Some(error.to_string());
                        view.status = format!("附件操作失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn video_settings_request(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.video_settings.loading {
            return;
        }
        let mut request = json!({"op":"video_settings", "action":action});
        let fields: &[&str] = match action {
            "dummy" => &["width", "height", "length"],
            "jump_frame" => &["frame"],
            _ => &[],
        };
        for &field in fields {
            match self.video_settings.inputs[field]
                .read(cx)
                .value()
                .trim()
                .parse::<i64>()
            {
                Ok(value) => request[field] = json!(value),
                Err(_) => {
                    self.video_settings.error = Some(format!("{field}：请输入有效整数"));
                    cx.notify();
                    return;
                }
            }
        }
        if action == "dummy" {
            request["fps"] = json!(
                self.video_settings.inputs["fps"]
                    .read(cx)
                    .value()
                    .to_string()
            );
            request["color"] = json!(
                self.video_settings.inputs["color"]
                    .read(cx)
                    .value()
                    .to_string()
            );
            request["pattern"] = json!(self.video_settings.pattern);
        }
        if action == "aspect" {
            request["value"] = json!(
                self.video_settings.inputs["aspect"]
                    .read(cx)
                    .value()
                    .to_string()
            );
        }
        if action == "jump_time" {
            match parse_time(&self.video_settings.inputs["time"].read(cx).value()) {
                Ok(value) => request["time"] = json!(value),
                Err(error) => {
                    self.video_settings.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            }
        }
        self.video_settings.loading = true;
        self.video_settings.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let draft = self.pending_draft(cx);
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<VideoSettings>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.video_settings.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.video_settings.receive(state);
                        view.apply_snapshot(snapshot, cx);
                        view.status = "视频参数已同步".into();
                    }
                    Err(error) => {
                        view.video_settings.error = Some(error.to_string());
                        view.status = format!("视频操作失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn paste_request(&mut self, apply: bool, cx: &mut Context<Self>) {
        if self.paste.loading {
            return;
        }
        let mut request = json!({"op":"paste_over"});
        if apply {
            request["fields"] = json!(self.paste.fields);
        }
        self.paste.loading = true;
        self.paste.error = None;
        let draft = self.pending_draft(cx);
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<PasteReply>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.paste.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        if !view.paste.initialized && state.fields.len() == 11 {
                            view.paste.fields.copy_from_slice(&state.fields);
                            view.paste.initialized = true;
                        }
                        if let Some(changed) = state.changed {
                            view.paste.message = format!("已覆盖 {changed} 行，可撤销");
                            view.status = view.paste.message.clone();
                        }
                        view.apply_snapshot(snapshot, cx);
                    }
                    Err(error) => {
                        view.paste.error = Some(error.to_string());
                        view.status = format!("覆盖粘贴失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn open_search(&mut self, selection: bool, cx: &mut Context<Self>) {
        let changed = self.find_panel.selection_mode != selection;
        self.active_panel = Panel::Search;
        self.find_panel.selection_mode = selection;
        if changed {
            self.find_panel.initialized = false;
        }
        self.find_panel.focus_pending = true;
        self.search_request("state", cx);
    }

    fn search_request(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.find_panel.loading {
            return;
        }
        let mut request = json!({"op":"search", "action":action});
        if action != "state" {
            request["find"] = json!(self.find_panel.find.read(cx).value().to_string());
            request["replace_with"] = json!(self.find_panel.replace.read(cx).value().to_string());
            request["field"] = json!(self.find_panel.field);
            request["mode"] = json!(self.find_panel.mode);
            request["selection_action"] = json!(self.find_panel.selection_action);
            for (&key, value) in &self.find_panel.options {
                request[key] = json!(value);
            }
        }
        self.find_panel.loading = true;
        self.find_panel.error = None;
        let draft = self.pending_draft(cx);
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let reply = serde_json::from_value::<SearchReply>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((reply, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.find_panel.loading = false;
                match result {
                    Ok((reply, snapshot)) => {
                        if action != "state" {
                            view.find_panel.message = match action {
                                "select" | "preview" => format!(
                                    "匹配 {} 行：{}",
                                    reply.count.unwrap_or(0),
                                    reply
                                        .rows
                                        .iter()
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                        .join("、")
                                ),
                                "replace_all" => {
                                    format!("已替换 {} 处，可撤销", reply.count.unwrap_or(0))
                                }
                                _ if reply.found == Some(true) => format!(
                                    "{}第 {} 行",
                                    if reply.replaced == Some(true) {
                                        "已替换，并找到"
                                    } else {
                                        "找到"
                                    },
                                    reply.row.unwrap_or(0)
                                ),
                                _ if reply.replaced == Some(true) => {
                                    "已替换，未找到其他匹配".into()
                                }
                                _ => "没有找到匹配项".into(),
                            };
                        }
                        view.find_panel.receive(reply);
                        view.apply_snapshot(snapshot, cx);
                        if matches!(action, "find" | "replace") {
                            view.pending_text_selection = Some(view.snapshot.text_selection);
                        }
                        view.status = if action == "state" {
                            "检索参数已同步".into()
                        } else {
                            view.find_panel.message.clone()
                        };
                    }
                    Err(error) => {
                        view.find_panel.error = Some(error.to_string());
                        view.status = format!("检索失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn processor_request(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.processor.loading {
            return;
        }
        let mut request = json!({"op":"timing_processor", "action":action});
        if action == "apply" {
            for &(key, label) in timing_processor_panel::NUMBERS {
                let maximum = if key == "adjacent_bias" {
                    100
                } else {
                    35999990
                };
                let parsed = self.processor.inputs[key]
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<i64>();
                match parsed {
                    Ok(value) if (0..=maximum).contains(&value) => request[key] = json!(value),
                    _ => {
                        self.processor.error =
                            Some(format!("{label}：请输入 0 到 {maximum} 的整数"));
                        cx.notify();
                        return;
                    }
                }
            }
            for (&key, value) in &self.processor.options {
                request[key] = json!(value);
            }
            request["styles"] = json!(self.processor.selected_styles);
        }
        self.processor.loading = true;
        self.processor.error = None;
        let draft = if action == "apply" {
            self.pending_draft(cx)
        } else {
            None
        };
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<ProcessorState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.processor.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.status = state.changed.map_or_else(
                            || "后处理参数已同步".into(),
                            |changed| format!("时间后处理修改 {changed} 行，可撤销"),
                        );
                        view.processor.receive(state);
                        view.apply_snapshot(snapshot, cx);
                    }
                    Err(error) => {
                        view.processor.error = Some(error.to_string());
                        view.status = format!("时间后处理失败：{error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn resample_request(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.resample.loading {
            return;
        }
        let mut request = json!({"op":"resample", "action":action});
        if action == "apply" {
            for &(key, _) in resample_panel::FIELDS {
                let value = self.resample.inputs[key]
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<i64>();
                let range = if matches!(key, "left" | "right" | "top" | "bottom") {
                    -9999..=9999
                } else {
                    1..=999999
                };
                if matches!(key, "left" | "right" | "top" | "bottom") && self.resample.ar_mode != 3
                {
                    request[key] = json!(0);
                    continue;
                }
                match value {
                    Ok(value) if range.contains(&value) => request[key] = json!(value),
                    _ => {
                        self.resample.error = Some(format!(
                            "{key}：请输入 {} 到 {} 的整数",
                            range.start(),
                            range.end()
                        ));
                        cx.notify();
                        return;
                    }
                }
            }
            request["ar_mode"] = json!(self.resample.ar_mode);
            request["source_matrix"] = json!(self.resample.source_matrix);
            request["dest_matrix"] = json!(self.resample.dest_matrix);
            request["expected_source"] =
                json!([self.resample.state.source_x, self.resample.state.source_y]);
        }
        self.resample.loading = true;
        self.resample.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let draft = self.pending_draft(cx);
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<ResampleState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.resample.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        if action == "apply" {
                            view.resample.initialized = false;
                        }
                        view.resample.receive(state);
                        view.status = if action == "apply" {
                            "重采样已应用，可撤销".into()
                        } else {
                            "重采样参数已同步".into()
                        };
                        view.apply_snapshot(snapshot, cx);
                        view.refresh_document(cx);
                    }
                    Err(error) => {
                        view.resample.error = Some(format!("重采样失败：{error}"));
                        view.status = "重采样未完成，请检查参数后重试".into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn pick_automation_script(&mut self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("Automation 脚本", &["lua", "moon"])
                .pick_file()
                .await
            {
                let _ = weak.update(cx, |view, cx| {
                    view.automation_request(
                        json!({"action":"add", "path":file.path().to_string_lossy()}),
                        cx,
                    );
                });
            }
        })
        .detach();
    }

    fn automation_request(&mut self, mut request: Value, cx: &mut Context<Self>) {
        if self.automation.loading {
            return;
        }
        request["op"] = json!("automation_manager");
        self.automation.loading = true;
        self.automation.error = None;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let draft = self.pending_draft(cx);
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<AutomationState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.automation.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.automation.state = state;
                        view.automation.confirm_remove = None;
                        view.status = "Automation 脚本已同步".into();
                        view.apply_snapshot(snapshot, cx);
                    }
                    Err(error) => {
                        view.automation.error = Some(error.to_string());
                        view.status = format!("Automation 操作失败：{error}");
                    }
                }
                view.refresh_catalog(cx);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn shift_times_request(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.timing.loading {
            return;
        }
        let mut request = json!({"op":"shift_times", "action":action});
        if action == "apply" {
            let parsed = if self.timing.by_time {
                parse_time(&self.timing.time.read(cx).value())
                    .map_err(|_| anyhow!("请输入有效的时:分:秒.百分秒"))
            } else {
                self.timing
                    .frames
                    .read(cx)
                    .value()
                    .trim()
                    .parse::<i64>()
                    .context("请输入非负整数帧数")
            };
            let amount = match parsed {
                Ok(amount) if (0..=35999990).contains(&amount) => amount,
                Ok(_) => {
                    self.timing.error = Some("平移量必须在 0 到 35999990 之间".into());
                    cx.notify();
                    return;
                }
                Err(error) => {
                    self.timing.error = Some(error.to_string());
                    cx.notify();
                    return;
                }
            };
            request["by_time"] = json!(self.timing.by_time);
            request["backward"] = json!(self.timing.backward);
            request["amount"] = json!(amount);
            request["mode"] = json!(self.timing.mode);
            request["fields"] = json!(self.timing.fields);
        }
        self.timing.loading = true;
        self.timing.error = None;
        let draft = if action == "apply" {
            self.pending_draft(cx)
        } else {
            None
        };
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let state = serde_json::from_value::<ShiftState>(bridge.call(request).await?)?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.timing.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        view.status = if let Some(changed) = state.changed {
                            format!("已平移 {changed} 行")
                        } else {
                            "平移历史已同步".into()
                        };
                        view.timing.receive(state);
                        view.apply_snapshot(snapshot, cx);
                    }
                    Err(error) => {
                        view.timing.error = Some(format!("平移失败：{error}"));
                        view.status = "平移未完成，请检查参数后重试".into();
                    }
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn styling_action(
        &mut self,
        action: &'static str,
        style: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let style = style.unwrap_or_else(|| self.styling.input.read(cx).value().to_string());
        if let Some(value) = self
            .document
            .details
            .styles
            .iter()
            .filter_map(|style| style.get("name"))
            .find(|name| name.eq_ignore_ascii_case(&style))
        {
            self.styling.pending_value = Some(value.clone());
        }
        self.invoke(json!({"op":"styling", "action":action, "id":self.snapshot.active, "style":style, "seek":self.styling.seek}), cx);
    }

    fn translation_action(&mut self, action: &'static str, cx: &mut Context<Self>) {
        if self.translation.loading {
            return;
        }
        if action == "insert_original" {
            let mut value = self.translation.editor.read(cx).value().to_string();
            let selected = self.translation.editor.read(cx).selected_range();
            if selected.end <= value.len()
                && value.is_char_boundary(selected.start)
                && value.is_char_boundary(selected.end)
            {
                value.replace_range(selected, &self.translation.state.text);
                self.translation.pending_text = Some(value);
                self.translation.focus_pending = true;
                cx.notify();
            }
            return;
        }
        self.translation.loading = true;
        let state = &self.translation.state;
        let mut request = json!({"op":"translation", "action":action, "block":state.block, "seek":self.translation.seek, "skip_whitespace":self.translation.skip_whitespace});
        if action != "start" && state.available {
            request["id"] = json!(state.id);
        }
        if matches!(action, "preview" | "commit") {
            request["text"] = json!(self.translation.editor.read(cx).value().to_string());
            request["expected"] = json!(state.text);
        }
        let draft = self.pending_draft(cx);
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = async {
                if let Some(draft) = draft {
                    bridge.call(draft).await?;
                }
                let value = bridge.call(request).await?;
                let state = serde_json::from_value::<TranslationState>(value)
                    .context("invalid translation state")?;
                let snapshot = bridge.snapshot().await?;
                Ok::<_, anyhow::Error>((state, snapshot))
            }
            .await;
            let _ = weak.update(cx, |view, cx| {
                view.translation.loading = false;
                match result {
                    Ok((state, snapshot)) => {
                        let keep = state.boundary && matches!(action, "next" | "prev");
                        if !keep {
                            view.translation.pending_text = Some(String::new());
                        }
                        view.translation.focus_pending = true;
                        view.status = if state.exhausted {
                            "已到最后一个可翻译文本块".into()
                        } else {
                            "翻译助手已同步".into()
                        };
                        view.translation.state = state;
                        view.apply_snapshot(snapshot, cx);
                    }
                    Err(error) => view.status = format!("翻译助手：{error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn refresh_library(&mut self, cx: &mut Context<Self>) {
        self.library_request(
            json!({"action":"list", "catalog":self.document.library.catalog}),
            cx,
        );
    }

    fn library_request(&mut self, mut payload: Value, cx: &mut Context<Self>) {
        if self.document.library_loading {
            return;
        }
        if !self.document.style_form.dirty.is_empty() && payload["action"] != "edit" {
            self.status = "请先应用或放弃当前样式的修改".into();
            cx.notify();
            return;
        }
        payload["op"] = json!("style_library");
        self.document.library_loading = true;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge.call(payload.clone()).await.and_then(|value| {
                serde_json::from_value::<LibraryDetails>(value).context("invalid style library")
            });
            let _ = weak.update(cx, |view, cx| {
                view.document.library_loading = false;
                match result {
                    Ok(mut details) => {
                        if matches!(
                            payload["action"].as_str(),
                            Some("import_preview" | "import")
                        ) {
                            view.document.import_path =
                                payload["path"].as_str().unwrap_or_default().into();
                            view.document.import_styles =
                                std::mem::take(&mut details.import_styles);
                            view.document.selected_import.clear();
                        }
                        if payload["action"] == "edit" {
                            view.document.style_form.acknowledge(&payload["fields"], cx);
                            if let Some(name) = payload["fields"]["name"].as_str() {
                                view.document.library_style = name.into();
                            }
                        }
                        if payload["action"] == "duplicate" {
                            if let Some(name) = payload["fields"]["name"].as_str() {
                                view.document.library_style = name.into();
                            }
                        }
                        if details.catalog != view.document.library.catalog {
                            view.document.selected_library.clear();
                            view.document.editing_library = false;
                        }
                        view.document.selected_library.retain(|name| {
                            details
                                .styles
                                .iter()
                                .any(|style| style.get("name") == Some(name))
                        });
                        if view.document.editing_library
                            && !details.styles.iter().any(|style| {
                                style.get("name") == Some(&view.document.library_style)
                            })
                        {
                            view.document.editing_library = false;
                        }
                        view.document.library = details;
                        view.status = "样式库已同步".into();
                        view.refresh_document(cx);
                    }
                    Err(error) => view.status = format!("样式库操作失败：{error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn pick_style_import(&mut self, cx: &mut Context<Self>) {
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            if let Some(file) = rfd::AsyncFileDialog::new()
                .add_filter("字幕样式", &["ass", "ssa", "srt"])
                .pick_file()
                .await
            {
                let _ = weak.update(cx, |view, cx| {
                    view.library_request(
                        json!({"action":"import_preview", "path":file.path().to_string_lossy()}),
                        cx,
                    )
                });
            }
        })
        .detach();
    }

    fn new_style(&mut self, cx: &mut Context<Self>) {
        if !self.document.style_form.dirty.is_empty() {
            self.status = "请先应用或放弃当前样式的修改".into();
            cx.notify();
            return;
        }
        let mut name = "新样式".to_string();
        let mut suffix = 2;
        while self
            .document
            .details
            .styles
            .iter()
            .any(|style| style.get("name") == Some(&name))
        {
            name = format!("新样式 {suffix}");
            suffix += 1;
        }
        self.document.editing_library = false;
        self.invoke(
            json!({"op":"style", "action":"create", "source":"", "fields":{"name":name}}),
            cx,
        );
    }

    fn refresh_document(&mut self, cx: &mut Context<Self>) {
        if self.document.loading {
            return;
        }
        self.document.loading = true;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge
                .call(json!({"op":"document"}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<DocumentDetails>(value)
                        .context("invalid document details")
                });
            let _ = weak.update(cx, |view, cx| {
                view.document.loading = false;
                match result {
                    Ok(details) => view.document.details = details,
                    Err(error) => view.status = format!("项目属性读取失败：{error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn commit_style(&mut self, cx: &mut Context<Self>) {
        if self.document.editing_library {
            self.library_request(json!({"action":"edit", "catalog":self.document.library.catalog, "source":self.document.library_style, "fields":self.document.style_form.patch(cx)}), cx);
            return;
        }
        self.invoke(json!({"op":"style", "action":"edit", "source":self.document.style, "fields":self.document.style_form.patch(cx)}), cx);
    }

    fn duplicate_style(&mut self, cx: &mut Context<Self>) {
        if !self.document.style_form.dirty.is_empty() {
            self.status = "请先应用或放弃当前样式的修改".into();
            cx.notify();
            return;
        }
        if self.document.editing_library {
            let base = format!("{} 副本", self.document.library_style);
            let mut name = base.clone();
            let mut suffix = 2;
            while self
                .document
                .library
                .styles
                .iter()
                .any(|style| style.get("name") == Some(&name))
            {
                name = format!("{base} {suffix}");
                suffix += 1;
            }
            self.library_request(json!({"action":"duplicate", "catalog":self.document.library.catalog, "source":self.document.library_style, "fields":{"name":name}}), cx);
            return;
        }
        let base = format!("{} 副本", self.document.style);
        let mut name = base.clone();
        let mut suffix = 2;
        while self.document.details.styles.iter().any(|style| {
            style
                .get("name")
                .is_some_and(|existing| existing.eq_ignore_ascii_case(&name))
        }) {
            name = format!("{base} {suffix}");
            suffix += 1;
        }
        self.invoke(json!({"op":"style", "action":"create", "source":self.document.style, "fields":{"name":name}}), cx);
    }

    fn refresh_catalog(&mut self, cx: &mut Context<Self>) {
        if self.catalog_loading {
            return;
        }
        self.catalog_loading = true;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            let result = bridge
                .call(json!({"op":"catalog"}))
                .await
                .and_then(|value| {
                    serde_json::from_value::<Catalog>(value).context("invalid command catalog")
                });
            let _ = weak.update(cx, |view, cx| {
                view.catalog_loading = false;
                match result {
                    Ok(catalog) => {
                        view.catalog = Arc::new(catalog);
                        install_native_menus(&view.catalog, cx);
                    }
                    Err(error) => view.status = format!("命令读取失败：{error}"),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn commit_properties(&mut self, cx: &mut Context<Self>) {
        let current = self
            .snapshot
            .lines
            .iter()
            .find(|line| line.id == self.selected_line);
        let Some(line) = current else {
            return;
        };
        let mut fields = serde_json::Map::new();
        for (key, input) in &self.property_inputs {
            let value = input.read(cx).value().to_string();
            let parsed: Result<Value> = match *key {
                "start" | "end" => parse_time(&value).map(Value::from),
                "layer" | "margin_left" | "margin_right" | "margin_vertical" => {
                    value.parse::<i64>().map(Value::from).context("请输入整数")
                }
                _ => Ok(Value::from(value)),
            };
            let value = match parsed {
                Ok(value) => value,
                Err(error) => {
                    self.status = format!("{key}：{error}");
                    cx.notify();
                    return;
                }
            };
            let previous = match *key {
                "start" => json!(line.start),
                "end" => json!(line.end),
                "layer" => json!(line.layer),
                "style" => json!(line.style),
                "actor" => json!(line.actor),
                "effect" => json!(line.effect),
                "margin_left" => json!(line.margin[0]),
                "margin_right" => json!(line.margin[1]),
                _ => json!(line.margin[2]),
            };
            if value != previous {
                fields.insert((*key).into(), value);
            }
        }
        self.invoke(
            json!({"op":"line_fields", "ids":self.snapshot.selected, "fields":fields}),
            cx,
        );
    }

    fn refresh_frame(&mut self, cx: &mut Context<Self>) {
        if !self.snapshot.video {
            self.clear_frame_image(cx);
            self.frame_request_inflight = false;
            self.frame_request_generation += 1;
            return;
        }
        if self.frame_request_inflight || self.frame_image_frame == self.snapshot.frame {
            return;
        }
        self.frame_request_inflight = true;
        self.frame_request_generation += 1;
        let generation = self.frame_request_generation;
        let request_frame = self.snapshot.frame;
        let request_epoch = self.video_epoch;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            match bridge.call(json!({"op":"frame", "frame":request_frame, "request":generation, "epoch":request_epoch})).await {
                Ok(value) => {
                    let reply: FrameReply = match serde_json::from_value(value) {
                        Ok(reply) => reply,
                        Err(error) => {
                            if let Some(view) = weak.upgrade() { let _ = view.update(cx, |view, cx| { if view.frame_request_generation == generation { view.frame_request_inflight = false; view.status = format!("帧数据格式错误：{error}"); cx.notify(); } }); }
                            return;
                        }
                    };
                    if reply.request.is_some_and(|request| request != generation) {
                        if let Some(view) = weak.upgrade() {
                            let _ = view.update(cx, |view, cx| {
                                if view.frame_request_generation == generation {
                                    view.frame_request_inflight = false;
                                    view.refresh_frame(cx);
                                }
                            });
                        }
                        return;
                    }
                    let Some(encoded) = reply.image else {
                        if let Some(view) = weak.upgrade() {
                            let _ = view.update(cx, |view, cx| {
                                if view.frame_request_generation != generation { return; }
                                view.frame_request_inflight = false;
                                if reply.video == Some(false) {
                                    view.apply_snapshot(Snapshot { video: false, ..view.snapshot.clone() }, cx);
                                }
                                if let Some(error) = reply.error {
                                    view.status = format!("视频帧请求失败：{error}");
                                    cx.notify();
                                } else {
                                    view.status = "等待 Aegisub 当前视频帧".into();
                                    cx.notify();
                                }
                            });
                        }
                        return;
                    };
                    let bytes = match cx.background_executor().spawn(async move {
                        base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
                    }).await {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            if let Some(view) = weak.upgrade() { let _ = view.update(cx, |view, cx| { if view.frame_request_generation == generation { view.frame_request_inflight = false; view.status = format!("视频帧解码失败：{error}"); cx.notify(); } }); }
                            return;
                        }
                    };
                    let width = reply.width.unwrap_or(0);
                    let height = reply.height.unwrap_or(0);
                    let frame = reply.frame.unwrap_or(-1);
                    let decoded = cx.background_executor().spawn(async move {
                        let decoder = image::ImageReader::with_format(
                            std::io::Cursor::new(bytes),
                            image::ImageFormat::Jpeg,
                        )
                        .into_decoder()?;
                        let rgba = image::DynamicImage::from_decoder(decoder)?.into_rgba8();
                        let mut bgra = rgba.into_raw();
                        for pixel in bgra.chunks_exact_mut(4) {
                            pixel.swap(0, 2);
                        }
                        let image = image::RgbaImage::from_raw(width, height, bgra)
                            .context("video frame dimensions do not match its pixel data")?;
                        Ok::<_, anyhow::Error>(Arc::new(gpui_kit::RenderImage::new(vec![
                            VideoImageFrame::new(image),
                        ])))
                    }).await;
                    let image = match decoded {
                        Ok(image) => image,
                        Err(error) => {
                            if let Some(view) = weak.upgrade() {
                                let _ = view.update(cx, |view, cx| {
                                    if view.frame_request_generation == generation {
                                        view.frame_request_inflight = false;
                                        view.status = format!("视频帧解码失败：{error}");
                                        cx.notify();
                                    }
                                });
                            }
                            return;
                        }
                    };
                    if let Some(view) = weak.upgrade() {
                        let _ = view.update(cx, |view, cx| {
                            if view.frame_request_generation != generation { return; }
                            view.frame_request_inflight = false;
                            if view.snapshot.video && view.video_epoch == request_epoch
                                && (view.snapshot.frame == frame
                                    || (view.snapshot.playing && frame >= view.frame_image_frame)) {
                                view.replace_frame_image(image, frame);
                                cx.notify();
                            } else {
                                view.refresh_frame(cx);
                            }
                        });
                    }
                }
                Err(error) => {
                    if let Some(view) = weak.upgrade() { let _ = view.update(cx, |view, cx| { if view.frame_request_generation == generation { view.frame_request_inflight = false; view.status = format!("获取视频帧失败：{error}"); cx.notify(); } }); }
                }
            }
        }).detach();
    }

    fn handle_key(
        &mut self,
        event: &gpui_kit::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if self.active_panel == Panel::Hotkeys && self.hotkeys.recording {
            if key.key == "escape" {
                self.hotkeys.recording = false;
            } else if !matches!(
                key.key.as_str(),
                "shift" | "control" | "alt" | "cmd" | "platform"
            ) {
                let mut combo = String::new();
                if if cfg!(target_os = "macos") {
                    key.modifiers.platform
                } else {
                    key.modifiers.control
                } {
                    combo.push_str("Ctrl-");
                }
                if key.modifiers.alt {
                    combo.push_str("Alt-");
                }
                if key.modifiers.shift {
                    combo.push_str("Shift-");
                }
                combo.push_str(&hotkey_key(&key.key));
                self.hotkeys
                    .key
                    .update(cx, |state, cx| state.set_value(combo, window, cx));
                self.hotkeys.recording = false;
            }
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let primary = if cfg!(target_os = "macos") {
            key.modifiers.platform
        } else {
            key.modifiers.control
        };
        if primary && key.modifiers.shift && key.key == "p" {
            self.active_panel = Panel::Commands;
            self.refresh_catalog(cx);
            self.command_search
                .update(cx, |state, cx| state.focus(window, cx));
            cx.stop_propagation();
            cx.notify();
            return;
        }
        let translation_focused = self
            .translation
            .editor
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let styling_focused = self
            .styling
            .input
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let assistant_context = if self.active_panel == Panel::Translation && translation_focused {
            Some("Translation Assistant")
        } else if self.active_panel == Panel::Styling && styling_focused {
            Some("Styling Assistant")
        } else {
            None
        };
        if let Some(context) = assistant_context {
            let shortcut = match key.key.as_str() {
                "f8" => "F8",
                "pageup" => "PageUp",
                "pagedown" => "PageDown",
                "home" => "Home",
                "end" => "End",
                "insert" => "Insert",
                _ => "",
            };
            if !shortcut.is_empty() && !primary && !key.modifiers.alt && !key.modifiers.shift {
                if let Some(command) = self.catalog.shortcut(context, shortcut) {
                    let name = command.name.clone();
                    if context == "Styling Assistant"
                        && matches!(name.as_str(), "grid/line/next" | "grid/line/prev")
                    {
                        self.styling_action(
                            if name.ends_with("next") {
                                "next"
                            } else {
                                "prev"
                            },
                            None,
                            cx,
                        );
                    } else {
                        self.invoke(json!({"op":"command", "name":name}), cx);
                    }
                    cx.stop_propagation();
                    return;
                }
            }
        }
        let editor_focused = self.editor.read(cx).focus_handle(cx).is_focused(window);
        let input_focused = window.has_focused_input(cx);
        let command_search_focused = self
            .command_search
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        if editor_focused {
            let line_break =
                key.key == "enter" && key.modifiers.shift && !primary && !key.modifiers.alt;
            let hard_space = key.key == "space"
                && if cfg!(target_os = "macos") {
                    key.modifiers.alt && !primary && !key.modifiers.shift
                } else {
                    primary && key.modifiers.shift && !key.modifiers.alt
                };
            if line_break || hard_space {
                let text = if hard_space {
                    "\\h"
                } else if self.snapshot.appearance.soft_line_break {
                    "\\n"
                } else {
                    "\\N"
                };
                self.editor
                    .update(cx, |state, cx| state.replace(text, window, cx));
                self.editor_dirty_line = self.selected_line;
                self.invoke(json!({"op":"edit", "id":self.selected_line, "ids":self.snapshot.selected, "text":self.editor.read(cx).value().to_string()}), cx);
                cx.stop_propagation();
                return;
            }
        }
        // A character key belongs to whichever text field has focus. With no text
        // focus there is nothing to type into, so the key is only a shortcut:
        // macOS reports Option+letter as a character, and dropping those here is
        // what used to make every Alt+letter binding dead in the grid.
        if event.prefer_character_input
            && (input_focused || editor_focused || command_search_focused)
        {
            return;
        }
        // A form field keeps the keys that would type into it, but command
        // shortcuts still have to reach the catalog: pressing Cmd+S with the
        // filter box focused must save, not be swallowed.
        if input_focused && !editor_focused && !command_search_focused && !primary {
            return;
        }
        if command_search_focused {
            if !primary
                || matches!(
                    key.key.as_str(),
                    "a" | "c" | "v" | "x" | "left" | "right" | "up" | "down"
                )
            {
                return;
            }
            let mut binding = Vec::new();
            if primary {
                binding.push("Ctrl");
            }
            if key.modifiers.alt {
                binding.push("Alt");
            }
            if key.modifiers.shift {
                binding.push("Shift");
            }
            let key_name = hotkey_key(&key.key);
            binding.push(&key_name);
            if let Some(command) = self.catalog.shortcut("Subtitle Grid", &binding.join("-")) {
                let name = command.name.clone();
                self.invoke(json!({"op":"command","name":name}), cx);
                cx.stop_propagation();
            }
            return;
        }
        if editor_focused
            && (!primary
                || matches!(
                    key.key.as_str(),
                    "a" | "c" | "v" | "x" | "left" | "right" | "up" | "down"
                ))
        {
            return;
        }
        let mut combo = String::new();
        if primary {
            combo.push_str("Ctrl-");
        }
        if key.modifiers.alt {
            combo.push_str("Alt-");
        }
        if key.modifiers.shift {
            combo.push_str("Shift-");
        }
        combo.push_str(&hotkey_key(&key.key));
        let context = if editor_focused {
            "Subtitle Edit Box"
        } else if command_search_focused {
            "Subtitle Grid"
        } else if self.active_panel == Panel::Audio {
            "Audio"
        } else if self.active_panel == Panel::Visual {
            "Video"
        } else {
            "Subtitle Grid"
        };
        if let Some(command) = self.catalog.shortcut(context, &combo) {
            let name = command.name.clone();
            self.invoke(json!({"op":"command", "name":name}), cx);
            cx.stop_propagation();
        }
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let filename = std::path::Path::new(&self.snapshot.file)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("未命名字幕");
        let enabled = |name: &str| {
            self.catalog
                .command(name)
                .is_some_and(|command| command.can_invoke())
        };
        div()
            .h_flex()
            .h(px(58.))
            .flex_shrink_0()
            .gap_2()
            .px_4()
            .items_center()
            .bg(p.surface)
            .border_b_1()
            .border_color(p.divider)
            .child(
                div()
                    .h_flex()
                    .flex_1()
                    .min_w_0()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(16.))
                            .font_semibold()
                            .child(self.active_panel.label()),
                    )
                    .child(
                        div()
                            .h_flex()
                            .flex_1()
                            .min_w_0()
                            .items_center()
                            .gap_2()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(Icon::new(IconName::FileText).size(px(12.)).flex_shrink_0())
                            .child(
                                div()
                                    .min_w_0()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .child(filename.to_owned()),
                            ),
                    ),
            )
            .child(
                Button::new("open-subtitles")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::FolderOpen)
                    .label("打开")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| view.pick_file("subtitle", cx))),
            )
            .child(
                Button::new("save-subtitles")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Save)
                    .label("保存")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"subtitle/save"}), cx)
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).mx_1().bg(p.divider))
            .child(
                Button::new("undo")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Undo2).size(px(15.)))
                    .accessibility_label("撤销")
                    .tooltip("撤销")
                    .ghost()
                    .disabled(!enabled("edit/undo"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                    })),
            )
            .child(
                Button::new("redo")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Redo2).size(px(15.)))
                    .accessibility_label("重做")
                    .tooltip("重做")
                    .ghost()
                    .disabled(!enabled("edit/redo"))
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"edit/redo"}), cx)
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).mx_1().bg(p.divider))
            .child(
                Button::new("open-video")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Play)
                    .label("视频")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| view.pick_file("video", cx))),
            )
            .child(
                div()
                    .ml_2()
                    .h_flex()
                    .gap_1()
                    .items_center()
                    .text_size(px(11.))
                    .text_color(if self.snapshot.modified {
                        p.warning
                    } else {
                        p.success
                    })
                    .child(
                        Icon::new(if self.snapshot.modified {
                            IconName::Asterisk
                        } else {
                            IconName::CircleCheck
                        })
                        .size(px(12.)),
                    )
                    .child(if self.snapshot.modified {
                        "未保存"
                    } else {
                        "已保存"
                    }),
            )
    }

    fn render_legacy_menus(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let mut bar = div()
            .h_flex()
            .items_center()
            .gap_2()
            .px_4()
            .h(px(38.))
            .bg(p.surface)
            .flex_shrink_0();
        for entry in self.catalog.menu("main") {
            if entry.special == "window" || entry.submenu == "main/window" {
                continue;
            }
            let catalog = self.catalog.clone();
            let weak = cx.weak_entity();
            let refresh_weak = weak.clone();
            let original_label = entry.text.replace('&', "");
            let label = match original_label.as_str() {
                "File" => "文件",
                "Edit" => "编辑",
                "View" => "视图",
                "Subtitle" => "字幕",
                "Timing" => "时间",
                "Video" => "视频",
                "Audio" => "音频",
                "Automation" => "自动化",
                "Help" => "帮助",
                _ => &original_label,
            }
            .to_owned();
            let submenu = entry.submenu.clone();
            let entries = if entry.special == "automation" {
                vec![entry]
            } else {
                catalog.menu(&submenu)
            };
            bar = bar.child(
                Button::new(format!("legacy-menu-{label}"))
                    .icon(IconName::Menu)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .label(label)
                    .ghost()
                    .dropdown_menu(move |menu, window, cx| {
                        populate_menu(
                            menu,
                            entries.clone(),
                            catalog.clone(),
                            weak.clone(),
                            window,
                            cx,
                        )
                    })
                    .on_open_change(move |open, _, cx| {
                        if *open {
                            let _ = refresh_weak.update(cx, |view, cx| view.refresh_catalog(cx));
                        }
                    }),
            );
        }
        bar.child(div().flex_1()).child(
            Button::new("command-browser")
                .rounded(gpui_kit::px(999.))
                .small()
                .px(gpui_kit::px(13.))
                .py(gpui_kit::px(3.))
                .icon(IconName::Search)
                .label("搜索命令")
                .ghost()
                .on_click(cx.listener(|view, _, _, cx| {
                    view.active_panel = Panel::Commands;
                    view.refresh_catalog(cx);
                    cx.notify();
                })),
        )
    }

    fn render_commands(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.command_search.read(cx).value().to_lowercase();
        let commands: Vec<_> = self
            .catalog
            .commands
            .iter()
            .filter(|command| {
                command.surface != "internal"
                    && (query.is_empty()
                        || format!("{} {} {}", command.name, command.label, command.help)
                            .to_lowercase()
                            .contains(&query))
            })
            .cloned()
            .collect();
        let count = commands.len();
        let weak = cx.weak_entity();
        page::page(cx, "commands-panel")
            .child(page::toolbar([
                Input::new(&self.command_search)
                    .id("command-search")
                    .w(px(320.))
                    .into_any_element(),
                Button::new("refresh-command-catalog").icon(IconName::RefreshCw).rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                    .label("刷新状态")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| view.refresh_catalog(cx)))
                    .into_any_element(),
                page::badge(cx, format!("{count} 项命令")).into_any_element(),
            ]))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_hidden()
                    .rounded(px(crate::workspace_theme::RADIUS_LG))
                    .border_1()
                    .border_color(crate::workspace_theme::Palette::current(cx).border)
                    .bg(crate::workspace_theme::Palette::current(cx).surface)
                    .shadow(page::card_shadow(Theme::global(cx).is_dark()))
                    .child(uniform_list("legacy-command-list", count, cx.processor(move |_, range: Range<usize>, _, cx| {
                range.map(|index| {
                    let command = commands[index].clone();
                    let name = command.name.clone();
                    let weak = weak.clone();
                    div().w_full().h(px(68.)).px_2().border_b_1().border_color(Theme::global(cx).border).h_flex().gap_3().items_center()
                        .child(div().flex_1().min_w_0()
                            .child(div().text_sm().child(format!("{}{}", if command.active { "✓ " } else { "" }, command.label)))
                            .child(div().text_xs().text_color(Theme::global(cx).muted_foreground).child(format!("{} · {}", command.name, command.help))))
                        .child(div().w(px(120.)).flex_shrink_0().text_xs().child(command.surface_label()))
                        .child(div().w(px(100.)).flex_shrink_0().text_xs().child(command.shortcut.clone()))
                        .child(Button::new(format!("invoke-{name}")).icon(IconName::Play).rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.)).label("执行").disabled(!command.can_invoke()).on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |view, cx| view.invoke(json!({"op":"command", "name":name}), cx));
                        }))
                }).collect()
            })).size_full()),
            )
    }

    /// Line inspector. `rail` is the copy that lives beside the text editor:
    /// narrower, no card chrome, and its own scroll.
    fn render_properties(&self, rail: Option<f32>, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let cell = |key: &'static str, label: &'static str| {
            div()
                .v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_size(px(11.)).text_color(p.muted).child(label))
                .child(
                    Input::new(&self.property_inputs[key])
                        .id(format!("property-{key}"))
                        .aria_label(label)
                        .h(px(28.))
                        .w_full(),
                )
        };
        let group = |title: &'static str, rows: Vec<Vec<gpui_kit::AnyElement>>| {
            let mut body = div().v_flex().w_full().min_w_0().gap_1();
            for row in rows {
                body = body.child(
                    div()
                        .h_flex()
                        .items_start()
                        .w_full()
                        .min_w_0()
                        .gap_2()
                        .children(row),
                );
            }
            div()
                .v_flex()
                .w_full()
                .min_w_0()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.))
                        .font_semibold()
                        .text_color(p.muted)
                        .child(title),
                )
                .child(body)
        };
        let commented = self
            .snapshot
            .lines
            .iter()
            .find(|line| line.id == self.selected_line)
            .is_some_and(|line| line.comment);
        let toggle_comment = Button::new("toggle-comment")
            .icon(IconName::Captions)
            .rounded(gpui_kit::px(999.))
            .small()
            .px(gpui_kit::px(13.))
            .py(gpui_kit::px(3.))
            .when(rail.is_some(), |button| {
                button.tooltip(if commented { "改为对白" } else { "改为注释" })
            })
            .when(rail.is_none(), |button| {
                button.label(if commented { "改为对白" } else { "改为注释" })
            })
            .ghost()
            .disabled(self.selected_line < 0)
            .on_click(cx.listener(move |view, _, _, cx| {
                view.invoke(json!({"op": "line_fields", "ids": view.snapshot.selected, "fields": {"comment": !commented}}), cx);
            }));
        let save_as = Button::new("save-as")
            .rounded(gpui_kit::px(999.))
            .small()
            .px(gpui_kit::px(13.))
            .py(gpui_kit::px(3.))
            .child(Icon::new(IconName::Save).size(px(15.)))
            .accessibility_label("另存为")
            .tooltip("另存为…")
            .ghost()
            .on_click(cx.listener(|view, _, _, cx| {
                view.invoke(json!({"op":"command", "name":"subtitle/save/as"}), cx)
            }));
        let apply = Button::new("apply-line-fields")
            .rounded(gpui_kit::px(999.))
            .small()
            .px(gpui_kit::px(13.))
            .py(gpui_kit::px(3.))
            .when(rail.is_some(), |button| {
                // The rail is too narrow for a label, so it shows the check only.
                button
                    .child(Icon::new(IconName::Check).size(px(15.)))
                    .accessibility_label("应用修改")
                    .tooltip("应用修改")
            })
            .when(rail.is_none(), |button| {
                button.icon(IconName::Check).label("应用修改")
            })
            .primary()
            .disabled(self.selected_line < 0 || !self.properties_dirty)
            .on_click(cx.listener(|view, _, _, cx| view.commit_properties(cx)));
        let count = div()
            .px_2()
            .py_0p5()
            .rounded(px(4.))
            .bg(p.raised)
            .text_size(px(11.))
            .text_color(p.muted)
            .child(format!("{} 行", self.snapshot.selected.len()));
        let header = if rail.is_some() {
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Settings2)
                        .size(px(15.))
                        .text_color(p.muted),
                )
                .child(div().text_sm().font_semibold().flex_1().child("行属性"))
                .child(toggle_comment)
                .child(apply)
        } else {
            div()
                .h_flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(IconName::Settings2)
                        .size(px(15.))
                        .text_color(p.muted),
                )
                .child(div().text_sm().font_semibold().flex_1().child("行属性"))
                .child(count)
                .child(toggle_comment)
                .child(save_as)
                .child(apply)
        };
        let groups = if rail.is_some() {
            vec![
                group(
                    "时间",
                    vec![vec![
                        cell("start", "开始").into_any_element(),
                        cell("end", "结束").into_any_element(),
                    ]],
                ),
                group(
                    "字幕",
                    vec![
                        vec![
                            cell("style", "样式").into_any_element(),
                            cell("actor", "角色").into_any_element(),
                        ],
                        vec![
                            cell("layer", "图层").into_any_element(),
                            cell("effect", "效果").into_any_element(),
                        ],
                    ],
                ),
                group(
                    "边距",
                    vec![vec![
                        cell("margin_left", "左").into_any_element(),
                        cell("margin_right", "右").into_any_element(),
                        cell("margin_vertical", "垂直").into_any_element(),
                    ]],
                ),
            ]
        } else {
            vec![
                group(
                    "时间",
                    vec![vec![
                        cell("start", "开始").into_any_element(),
                        cell("end", "结束").into_any_element(),
                    ]],
                ),
                group(
                    "字幕",
                    vec![vec![
                        cell("style", "样式").into_any_element(),
                        cell("actor", "角色").into_any_element(),
                        cell("layer", "图层").into_any_element(),
                        cell("effect", "效果").into_any_element(),
                    ]],
                ),
                group(
                    "边距",
                    vec![vec![
                        cell("margin_left", "左").into_any_element(),
                        cell("margin_right", "右").into_any_element(),
                        cell("margin_vertical", "垂直").into_any_element(),
                    ]],
                ),
            ]
        };
        div()
            .id("line-properties")
            .v_flex()
            .min_w_0()
            .min_h_0()
            .when(rail.is_some(), |panel| {
                panel
                    .w(px(rail.unwrap_or(272.)))
                    .flex_shrink_0()
                    .h_full()
                    .p_3()
                    .gap_3()
                    .border_l_1()
                    .border_color(p.divider)
            })
            .when(rail.is_none(), |panel| {
                panel
                    .flex_1()
                    .rounded(px(crate::workspace_theme::RADIUS_MD))
                    .border_1()
                    .border_color(p.border)
                    .bg(p.surface)
                    .shadow(crate::page::card_shadow(Theme::global(cx).is_dark()))
                    .p_2()
                    .gap_0p5()
            })
            .child(header)
            .children(groups)
            .overflow_y_scrollbar()
    }

    fn activate_panel(&mut self, panel: Panel, window: &mut Window, cx: &mut Context<Self>) {
        self.focus_handle.focus(window, cx);
        if self.active_panel == Panel::Visual && panel != Panel::Visual {
            self.visual.dragging = false;
            self.visual.queue.clear();
            self.queue_visual(json!({"action":"cancel"}), cx);
        }
        self.visual.pan_last = None;
        self.active_panel = panel;
        if panel == Panel::Commands {
            self.refresh_catalog(cx);
        }
        if panel == Panel::Styles || panel == Panel::Project {
            self.refresh_document(cx);
        }
        if panel == Panel::Styles {
            self.refresh_library(cx);
        }
        if panel == Panel::VideoSettings {
            self.video_settings_request("state", cx);
        }
        if panel == Panel::Preferences {
            self.preferences_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Hotkeys {
            self.hotkey_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Paste {
            self.paste_request(false, cx);
        }
        if panel == Panel::Files {
            self.file_tools_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Attachments {
            self.attachment_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Spelling {
            self.spelling_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Search {
            self.find_panel.focus_pending = true;
            self.search_request("state", cx);
        }
        if panel == Panel::Processor {
            self.processor_request("state", cx);
        }
        if panel == Panel::Resample {
            self.resample_request("state", cx);
        }
        if panel == Panel::Automation {
            self.automation_request(json!({"action":"state"}), cx);
        }
        if panel == Panel::Timing {
            self.shift_times_request("state", cx);
        }
        if panel == Panel::Audio {
            self.refresh_audio(cx);
        }
        if panel == Panel::Translation {
            self.translation_action("start", cx);
        }
        if panel == Panel::Styling {
            self.styling.line_id = -1;
            self.styling.focus_pending = true;
            self.refresh_document(cx);
            self.styling_action("start", None, cx);
        }
        if panel == Panel::Visual {
            self.visual.drawn = None;
            self.queue_visual(json!({"action":"draw"}), cx);
        }
        cx.notify();
    }

    fn render_menu(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let compact = self
            .sidebar_collapsed
            .unwrap_or(f32::from(window.viewport_size().width) < 1150.);
        let toggle_label = if compact {
            "展开侧栏"
        } else {
            "折叠侧栏"
        };
        let mut navigation = div()
            .id("workspace-navigation")
            .v_flex()
            .min_h_0()
            .flex_1()
            .gap_1()
            .px_3()
            .py_4()
            .overflow_y_scrollbar();
        for (group, panels) in [
            (
                "工作区",
                vec![
                    Panel::Subtitles,
                    Panel::Audio,
                    Panel::Visual,
                    Panel::Translation,
                ],
            ),
            (
                "字幕工具",
                vec![
                    Panel::Styles,
                    Panel::Styling,
                    Panel::Timing,
                    Panel::Processor,
                    Panel::Resample,
                    Panel::Search,
                    Panel::Spelling,
                    Panel::Paste,
                ],
            ),
            (
                "项目与设置",
                vec![
                    Panel::Project,
                    Panel::VideoSettings,
                    Panel::Automation,
                    Panel::Attachments,
                    Panel::Files,
                    Panel::Preferences,
                    Panel::Hotkeys,
                    Panel::Commands,
                ],
            ),
        ] {
            navigation = navigation.child(
                div()
                    .mt_4()
                    .mb_2()
                    .px_2()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .child(if compact { "" } else { group }),
            );
            for panel in panels {
                let selected = self.active_panel == panel;
                let content = div()
                    .h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .when(compact, |v| v.justify_center())
                    .child(Icon::new(panel.icon()).size(px(15.)))
                    .when(!compact, |v| {
                        v.child(div().flex_1().text_size(px(13.)).child(panel.label()))
                    });
                let button = Button::new(format!("panel-{}", panel.label()))
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .ghost()
                    .selected(selected)
                    .accessibility_label(panel.label())
                    .tooltip(panel.label())
                    .w_full()
                    .h(px(36.))
                    .px_2()
                    .child(content)
                    .on_click(cx.listener(move |view, _, window, cx| {
                        view.activate_panel(panel, window, cx)
                    }));
                navigation = navigation.child(button);
            }
        }
        div()
            .v_flex()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .w(px(if compact { 60. } else { 164. }))
            .flex_shrink_0()
            .bg(p.canvas)
            .border_r_1()
            .border_color(p.divider)
            .child(
                div()
                    .h_flex()
                    .h(px(64.))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .justify_between()
                    .when(compact, |v| v.justify_center())
                    .flex_shrink_0()
                    .when(!compact, |v| {
                        v.child(
                            div()
                                .v_flex()
                                .child(div().font_semibold().text_size(px(15.)).child("Aegisub"))
                                .child(
                                    div()
                                        .text_size(px(11.))
                                        .text_color(p.muted)
                                        .child("字幕工作台"),
                                ),
                        )
                    })
                    .child(
                        // Button::icon() renders nothing inside this header row,
                        // so the icon goes in as an explicit child like the nav
                        // rows do.
                        Button::new("toggle-sidebar")
                            .rounded(gpui_kit::px(999.))
                            .small()
                            .px(gpui_kit::px(11.))
                            .py(gpui_kit::px(3.))
                            .ghost()
                            .accessibility_label(toggle_label)
                            .tooltip(toggle_label)
                            .child(
                                Icon::new(if compact {
                                    IconName::PanelLeftOpen
                                } else {
                                    IconName::PanelLeftClose
                                })
                                .size(px(16.))
                                .text_color(p.muted),
                            )
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.sidebar_collapsed = Some(!compact);
                                cx.notify();
                            })),
                    ),
            )
            .child(navigation)
    }

    fn cancel_visual_drag(&mut self, cx: &mut Context<Self>) {
        if self.visual.dragging {
            self.visual.dragging = false;
            self.visual.queue.clear();
            self.queue_visual(json!({"action":"cancel"}), cx);
        }
    }

    fn adjust_video_view(
        &mut self,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        zoom: f32,
        cx: &mut Context<Self>,
    ) {
        self.cancel_visual_drag(cx);
        self.visual.zoom_at(position, zoom / self.visual.zoom);
        cx.notify();
    }

    fn pan_video(&mut self, delta: gpui_kit::Point<gpui_kit::Pixels>, cx: &mut Context<Self>) {
        self.cancel_visual_drag(cx);
        self.visual.pan_by(delta);
        cx.notify();
    }

    fn update_video_pan(
        &mut self,
        position: gpui_kit::Point<gpui_kit::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(previous) = self.visual.pan_last {
            self.visual.pan_last = Some(position);
            self.pan_video(position - previous, cx);
        }
    }

    fn handle_video_scroll(&mut self, event: &gpui_kit::ScrollWheelEvent, cx: &mut Context<Self>) {
        if !self.snapshot.video {
            return;
        }
        let primary = if cfg!(target_os = "macos") {
            event.modifiers.platform
        } else {
            event.modifiers.control
        };
        if primary && event.modifiers.shift {
            return;
        }
        let action = if primary {
            self.snapshot.video_ctrl_scroll
        } else if event.modifiers.shift {
            self.snapshot.video_shift_scroll
        } else {
            self.snapshot.video_scroll
        };
        let delta = match event.delta {
            ScrollDelta::Pixels(delta) => {
                gpui_kit::point(f32::from(delta.x) / 40., f32::from(delta.y) / 40.)
            }
            ScrollDelta::Lines(delta) => delta,
        };
        match action {
            0 | 1 if delta.y != 0. => {
                let direction = if action == 1 { -1. } else { 1. };
                self.cancel_visual_drag(cx);
                self.visual.fit_video = false;
                self.invoke(json!({"op":"video_settings", "action":"window_zoom", "value":(self.snapshot.video_window_zoom + direction * 0.125 * delta.y).clamp(0.125, 10.).to_string()}), cx);
            }
            2 | 3 if delta.y != 0. => {
                let direction = if action == 3 { -1. } else { 1. };
                self.adjust_video_view(
                    event.position,
                    self.visual.zoom * 1.125_f32.powf(direction * delta.y),
                    cx,
                );
            }
            4 | 5 => {
                let distance = if action == 4 {
                    delta
                } else {
                    gpui_kit::point(delta.y, delta.x)
                };
                self.pan_video(
                    gpui_kit::point(px(distance.x * 5.), px(distance.y * 5.)),
                    cx,
                );
            }
            _ => return,
        }
        cx.stop_propagation();
    }

    fn render_player(
        &self,
        window: &Window,
        side_by_side: bool,
        fill: bool,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let p = Palette::current(cx);
        let aspect = self.snapshot.aspect as f32;
        let zoom = self.visual.zoom;
        let pan = self.visual.pan;
        let height =
            self.snapshot.video_height * self.snapshot.video_window_zoom / window.scale_factor();
        let display_size = (self.snapshot.video && !self.visual.fit_video)
            .then(|| size(px(height * aspect), px(height)));
        let available_height = f32::from(window.viewport_size().height);
        let view_height = if fill {
            // The line properties moved beside the text editor, so nothing is
            // stacked under the preview any more: it takes the whole column.
            (available_height - 430.).max(160.)
        } else if self.visual.fit_video || !self.snapshot.video {
            if side_by_side {
                (available_height - 510.).clamp(130., 140.)
            } else if self.active_panel == Panel::Visual {
                (available_height - 350.).max(160.)
            } else {
                (available_height - 620.).clamp(90., 240.)
            }
        } else {
            let reserved_height = if side_by_side {
                510.
            } else if self.active_panel == Panel::Visual {
                350.
            } else {
                620.
            };
            let max_height = available_height - reserved_height;
            height.clamp(80., max_height.max(80.))
        };
        let view_height = if fill {
            view_height
        } else {
            self.video_preview_height.unwrap_or(view_height)
        };
        self.last_preview_height.set(view_height);
        let image = self.frame_image.clone();
        let has_image = image.is_some();
        let overscan = self.snapshot.video && self.snapshot.video_overscan;
        let viewport = self.visual.viewport_bounds.clone();
        let canvas_region = self.visual.canvas_bounds.clone();
        let image_region = self.visual.bounds.clone();
        let visual_active = self.active_panel == Panel::Visual;
        let status = if self.snapshot.video {
            format!(
                "帧 {:05} · {:.2}:1 · {} fps",
                self.snapshot.frame,
                self.snapshot.aspect,
                self.snapshot
                    .fps
                    .map(|fps| format!("{fps:.3}"))
                    .unwrap_or_else(|| "VFR".into())
            )
        } else {
            "未加载视频".into()
        };
        let weak = cx.weak_entity();
        let fit = self.visual.fit_video;
        let window_zoom = self.snapshot.video_window_zoom;
        let controls = div()
            .h_flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .px_2()
            .py_2()
            .rounded_t(px(crate::workspace_theme::RADIUS_LG - 1.))
            .bg(p.surface)
            .text_color(p.text)
            .child(Icon::new(IconName::Play).size(px(14.)).text_color(p.muted))
            .child(
                div()
                    .text_size(px(12.))
                    .font_semibold()
                    .px_1()
                    .child("视频预览"),
            )
            .child(div().w(px(1.)).h(px(16.)).mx_2().bg(p.divider))
            .child(
                Button::new("video-window-zoom-menu")
                    .icon(IconName::Maximize)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .label(if fit {
                        "适应".to_owned()
                    } else {
                        format!("{}%", window_zoom * 100.)
                    })
                    .child(Icon::new(IconName::ChevronDown).size(px(15.)))
                    .ghost()
                    .tooltip("视频框倍率")
                    .dropdown_menu(move |mut menu, _, _| {
                        for (name, label, value) in [
                            ("video/zoom/50", "50%", 0.5),
                            ("video/zoom/100", "100%", 1.),
                            ("video/zoom/200", "200%", 2.),
                            ("video/zoom/in", "放大视频框", -1.),
                            ("video/zoom/out", "缩小视频框", -1.),
                        ] {
                            let weak = weak.clone();
                            menu = menu.item(
                                gpui_kit::component::menu::PopupMenuItem::new(label)
                                    .checked(!fit && window_zoom == value)
                                    .on_click(move |_, _, cx| {
                                        let _ = weak.update(cx, |view, cx| {
                                            view.invoke(json!({"op":"command", "name":name}), cx);
                                        });
                                    }),
                            );
                        }
                        let weak = weak.clone();
                        menu.item(
                            gpui_kit::component::menu::PopupMenuItem::new("刷新当前视频帧")
                                .on_click(move |_, _, cx| {
                                    let _ = weak.update(cx, |view, cx| {
                                        view.clear_frame_image(cx);
                                        view.refresh_frame(cx);
                                    });
                                }),
                        )
                    }),
            )
            .child(
                Button::new("video-fit")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Maximize).size(px(15.)))
                    .accessibility_label("适应窗口")
                    .tooltip("适应窗口")
                    .ghost()
                    .selected(fit)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.cancel_visual_drag(cx);
                        view.visual.fit_video = true;
                        view.visual.reset_zoom();
                        cx.notify();
                    })),
            )
            .child(div().flex_1())
            .child(
                Button::new("video-content-out")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::ZoomOut).size(px(15.)))
                    .accessibility_label("缩小画面")
                    .tooltip("缩小画面")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        let bounds = *view.visual.viewport_bounds.borrow();
                        view.adjust_video_view(bounds.center(), view.visual.zoom / 1.125, cx);
                    })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .w(px(48.))
                    .text_center()
                    .child(format!("{:.0}%", zoom * 100.)),
            )
            .child(
                Button::new("video-content-in")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::ZoomIn).size(px(15.)))
                    .accessibility_label("放大画面")
                    .tooltip("放大画面")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        let bounds = *view.visual.viewport_bounds.borrow();
                        view.adjust_video_view(bounds.center(), view.visual.zoom * 1.125, cx);
                    })),
            )
            .child(
                Button::new("video-zoom-reset")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::RotateCw).size(px(15.)))
                    .accessibility_label("复位画面")
                    .tooltip("复位缩放和平移")
                    .ghost()
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"video/reset_pan"}), cx)
                    })),
            )
            .child(
                Button::new("video-pan-menu")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .child(Icon::new(IconName::Move).size(px(15.)))
                    .accessibility_label("平移画面")
                    .tooltip("平移画面")
                    .ghost()
                    .dropdown_menu({
                        let weak = cx.weak_entity();
                        move |mut menu, _, _| {
                            for (label, x, y) in [
                                ("向左平移", -20., 0.),
                                ("向右平移", 20., 0.),
                                ("向上平移", 0., -20.),
                                ("向下平移", 0., 20.),
                            ] {
                                let weak = weak.clone();
                                menu = menu.item(
                                    gpui_kit::component::menu::PopupMenuItem::new(label).on_click(
                                        move |_, _, cx| {
                                            let _ = weak.update(cx, |view, cx| {
                                                view.pan_video(gpui_kit::point(px(x), px(y)), cx)
                                            });
                                        },
                                    ),
                                );
                            }
                            menu
                        }
                    }),
            )
            .child(div().w(px(1.)).h(px(16.)).mx_1().bg(p.divider))
            .child(
                Button::new("video-overscan")
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(IconName::Frame)
                    .label("安全区")
                    .toggled(overscan)
                    .tooltip("电视过扫描安全区")
                    .ghost()
                    .selected(overscan)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"video/show_overscan"}), cx)
                    })),
            );
        // The empty stage follows the theme; a loaded video keeps a dark
        // letterbox so frames and subtitles read against it either way.
        let stage_dark = self.snapshot.video;
        let stage_bg: gpui_kit::Hsla = if stage_dark {
            rgb(0x0e1013).into()
        } else {
            p.inset
        };
        let stage_text: gpui_kit::Hsla = if stage_dark {
            rgb(0xa1aabd).into()
        } else {
            p.muted
        };
        div()
            .v_flex()
            .w_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .when(side_by_side && !fill, |card| card.flex_shrink_0())
            .when(fill, |card| card.flex_1())
            .rounded(px(crate::workspace_theme::RADIUS_LG))
            .border_1()
            .border_color(p.border)
            .when(self.snapshot.video, |v| v.child(controls))
            .child(
                div()
                    .id("video-frame-surface")
                    .when(fill, |surface| surface.flex_1().min_h(px(120.)))
                    .when(!fill, |surface| surface.h(px(view_height)).flex_shrink_0())
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .bg(stage_bg)
                    .child(
                        canvas(
                            move |bounds, _window, _cx| {
                                *viewport.borrow_mut() = bounds;
                                let base = video_base_bounds(bounds, aspect, display_size);
                                let target = zoomed_video_bounds(base, zoom, pan);
                                // Visual overlay owns its layout and resize notification while shown.
                                if !visual_active {
                                    *canvas_region.borrow_mut() = base;
                                    *image_region.borrow_mut() = target;
                                }
                                (target, image)
                            },
                            move |viewport, (target, image), window, _| {
                                if let Some(image) = image {
                                    let _ = window.paint_image(
                                        viewport,
                                        target,
                                        Corners::default(),
                                        image,
                                        0,
                                        false,
                                    );
                                    if overscan {
                                        window.with_content_mask(
                                            Some(gpui_kit::ContentMask { bounds: viewport }),
                                            |window| video_overscan::paint(target, aspect, window),
                                        );
                                    }
                                }
                            },
                        )
                        .absolute()
                        .size_full(),
                    )
                    .when(!has_image, |v| {
                        v.child(
                            div()
                                .absolute()
                                .inset_0()
                                .v_flex()
                                .gap_3()
                                .items_center()
                                .justify_center()
                                .text_color(stage_text)
                                .child(Icon::new(IconName::Play).size(px(32.)))
                                .child(div().text_sm().child(if self.snapshot.video {
                                    "正在加载视频帧"
                                } else {
                                    "打开视频，预览字幕效果"
                                }))
                                .when(!self.snapshot.video, |v| {
                                    v.child(
                                        Button::new("empty-open-video")
                                            .rounded(gpui_kit::px(999.))
                                            .small()
                                            .px(gpui_kit::px(13.))
                                            .py(gpui_kit::px(3.))
                                            .label("打开视频")
                                            .child(Icon::new(IconName::FolderOpen).size(px(15.)))
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.pick_file("video", cx)
                                            })),
                                    )
                                }),
                        )
                    })
                    .when(self.snapshot.video && visual_active, |v| {
                        v.child(self.visual.overlay(aspect, display_size, cx))
                    })
                    .when(self.snapshot.video, |v| {
                        v.on_scroll_wheel(cx.listener(
                            |view, event: &gpui_kit::ScrollWheelEvent, _, cx| {
                                view.handle_video_scroll(event, cx)
                            },
                        ))
                        .on_mouse_down(
                            gpui_kit::MouseButton::Middle,
                            cx.listener(|view, event: &gpui_kit::MouseDownEvent, window, cx| {
                                view.cancel_visual_drag(cx);
                                view.focus_handle.focus(window, cx);
                                view.visual.pan_last = Some(event.position);
                            }),
                        )
                        .on_mouse_up(
                            gpui_kit::MouseButton::Middle,
                            cx.listener(|view, event: &gpui_kit::MouseUpEvent, _, cx| {
                                view.update_video_pan(event.position, cx);
                                view.visual.pan_last = None;
                            }),
                        )
                        .on_mouse_up_out(
                            gpui_kit::MouseButton::Middle,
                            cx.listener(|view, event: &gpui_kit::MouseUpEvent, _, cx| {
                                view.update_video_pan(event.position, cx);
                                view.visual.pan_last = None;
                            }),
                        )
                    }),
            )
            .child(
                div()
                    .h_flex()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .bg(stage_bg)
                    .text_xs()
                    .text_color(stage_text)
                    .child(status)
                    .when(f32::from(window.viewport_size().width) > 1100., |v| {
                        v.child("⌘/Ctrl 滚轮缩放 · Shift 平移")
                    }),
            )
            .child(self.timeline.render(&self.snapshot, cx))
    }

    fn subtitle_column_width(&self, column: SubtitleColumn, secondary: f32) -> f32 {
        self.subtitle_column_widths
            .iter()
            .find(|(key, _)| *key == column)
            .map(|(_, width)| *width)
            .unwrap_or_else(|| column.default_width(secondary))
    }

    fn set_subtitle_column_width(&mut self, column: SubtitleColumn, width: f32) {
        match self
            .subtitle_column_widths
            .iter_mut()
            .find(|(key, _)| *key == column)
        {
            Some(entry) => entry.1 = width,
            None => self.subtitle_column_widths.push((column, width)),
        }
    }

    /// Adds or removes an optional column, keeping the canonical left-to-right
    /// order so a re-added column returns to where it was.
    fn toggle_subtitle_column(&mut self, column: SubtitleColumn) {
        const ORDER: [SubtitleColumn; 7] = [
            SubtitleColumn::IdLayer,
            SubtitleColumn::Time,
            SubtitleColumn::StyleActor,
            SubtitleColumn::Layer,
            SubtitleColumn::Effect,
            SubtitleColumn::Actor,
            SubtitleColumn::Comment,
        ];
        if let Some(index) = self.subtitle_columns.iter().position(|key| *key == column) {
            self.subtitle_columns.remove(index);
            return;
        }
        let rank = |column: SubtitleColumn| {
            ORDER
                .iter()
                .position(|key| *key == column)
                .unwrap_or(usize::MAX)
        };
        let position = self
            .subtitle_columns
            .iter()
            .position(|key| rank(*key) > rank(column))
            .unwrap_or(self.subtitle_columns.len());
        self.subtitle_columns.insert(position, column);
    }

    fn render_subtitle_panel(
        &mut self,
        rail: Option<f32>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let p = Palette::new(dark);
        let appearance = &self.snapshot.appearance;
        let line_bg = appearance.color("Colour/Subtitle Grid/Background/Background", p.surface);
        let foreground = appearance.color("Colour/Subtitle Grid/Standard", p.text);
        let selected_foreground =
            appearance.color("Colour/Subtitle Grid/Selection", p.selected_text);
        let comment_bg = appearance.color("Colour/Subtitle Grid/Background/Comment", p.inset);
        let selected_comment_bg = appearance.color(
            "Colour/Subtitle Grid/Background/Selected Comment",
            p.selected,
        );
        let inframe_bg = appearance.color("Colour/Subtitle Grid/Background/Inframe", p.raised);
        let highlight_visible = appearance.highlight_visible && self.snapshot.video;
        let video_time = self.snapshot.video_time;
        let focus_grid = appearance.focus_grid;
        // The option is a point size. GPUI's logical pixel is a point on macOS
        // but 1/96 inch elsewhere, so only Windows and Linux need the 4/3
        // points-to-pixels conversion the wxWidgets grid got from wxDC.
        let points_to_px = if cfg!(target_os = "macos") {
            1.
        } else {
            4. / 3.
        };
        let grid_size = if appearance.grid_size > 0 {
            (appearance.grid_size as f32 * points_to_px).clamp(4., 56.)
        } else {
            12.
        };
        let grid_font = appearance.grid_font.clone();
        // Secondary columns scale with the configured grid font so the list
        // reads as one type scale instead of 16px text beside 11px labels.
        let secondary_size = (grid_size * 0.8).clamp(10., 20.);
        // Column widths follow the type size, otherwise a larger grid font
        // pushes the time range into the style column. Anything the user has
        // dragged keeps its pixel width instead.
        let columns: std::rc::Rc<Vec<(SubtitleColumn, f32)>> = std::rc::Rc::new(
            self.subtitle_columns
                .iter()
                .map(|column| (*column, self.subtitle_column_width(*column, secondary_size)))
                .collect(),
        );
        let active_columns = self.subtitle_columns.clone();
        // Adding columns must not push the text column off the panel, so once
        // the fixed columns no longer fit they shrink together (down to a per
        // column floor) instead of overflowing.
        let grid_width = self.subtitle_grid_width;
        let columns: std::rc::Rc<Vec<(SubtitleColumn, f32)>> = if grid_width > 0. {
            let room = grid_width - 26. - COLUMN_GAP * columns.len() as f32 - MIN_TEXT_COLUMN;
            let total: f32 = columns.iter().map(|(_, width)| *width).sum();
            if total > room && room > 0. {
                // Widths the user dragged are honoured; only the untouched
                // columns give way, so resizing one column actually resizes it.
                let dragged: Vec<bool> = columns
                    .iter()
                    .map(|(column, _)| {
                        self.subtitle_column_widths
                            .iter()
                            .any(|(other, _)| other == column)
                    })
                    .collect();
                let fixed: f32 = columns
                    .iter()
                    .zip(&dragged)
                    .filter(|(_, dragged)| **dragged)
                    .map(|((_, width), _)| *width)
                    .sum();
                let floors: f32 = columns
                    .iter()
                    .zip(&dragged)
                    .filter(|(_, dragged)| !**dragged)
                    .map(|((column, _), _)| column.min_width())
                    .sum();
                let auto: f32 = total - fixed;
                if fixed + floors <= room {
                    let factor = (room - fixed).max(0.) / auto.max(1.);
                    let factor = factor.min(1.);
                    std::rc::Rc::new(
                        columns
                            .iter()
                            .zip(&dragged)
                            .map(|((column, width), dragged)| {
                                if *dragged {
                                    (*column, *width)
                                } else {
                                    (*column, (*width * factor).max(column.min_width()))
                                }
                            })
                            .collect(),
                    )
                } else {
                    // Even the floors do not fit: everything shrinks so the
                    // text column keeps its minimum instead of running off.
                    let factor = room / total;
                    std::rc::Rc::new(
                        columns
                            .iter()
                            .map(|(column, width)| {
                                (*column, (*width * factor).max(column.min_width()))
                            })
                            .collect(),
                    )
                }
            } else {
                columns
            }
        } else {
            columns
        };
        let menu_weak = cx.weak_entity();
        let size_weak = cx.weak_entity();
        let row_height = (grid_size * 1.6 + 12.).max(36.);
        let stripe = p.surface;
        let border = appearance.color("Colour/Subtitle Grid/Lines", p.divider);
        let header_bg = appearance.color("Colour/Subtitle Grid/Header", p.inset);
        let muted = p.muted;
        let selected_bg = appearance.color("Colour/Subtitle Grid/Background/Selection", p.selected);
        let accent = p.accent;
        let selected = self.snapshot.selected.clone();
        let weak = cx.weak_entity();
        let time_font = Theme::global(cx).mono_font_family.clone();
        let filtered = self.search.read(cx).value().to_string();
        let lower_filter = filtered.to_lowercase();
        let cache_matches =
            self.subtitle_filter_cache
                .as_ref()
                .is_some_and(|(revision, file, query, _)| {
                    *revision == self.snapshot.revision
                        && file == &self.snapshot.file
                        && query == &lower_filter
                });
        if !cache_matches {
            let visible = self
                .snapshot
                .lines
                .iter()
                .filter(|line| {
                    lower_filter.is_empty()
                        || line.text.to_lowercase().contains(&lower_filter)
                        || line.actor.to_lowercase().contains(&lower_filter)
                        || line.style.to_lowercase().contains(&lower_filter)
                })
                .cloned()
                .collect();
            self.subtitle_filter_cache = Some((
                self.snapshot.revision,
                self.snapshot.file.clone(),
                lower_filter.clone(),
                Arc::new(visible),
            ));
        }
        let visible = self
            .subtitle_filter_cache
            .as_ref()
            .expect("subtitle filter cache was just populated")
            .3
            .clone();
        let editor = Textarea::new(&self.editor)
            .aria_label("所选字幕文本")
            .bg(appearance.color("Colour/Subtitle/Background", p.inset))
            .text_color(appearance.color("Colour/Subtitle/Syntax/Normal", p.text))
            .when(appearance.edit_size > 0, |editor| {
                editor.text_size(px((appearance.edit_size as f32 * 4. / 3.).clamp(4., 56.)))
            })
            .when(!appearance.edit_font.is_empty(), |editor| {
                editor.font_family(appearance.edit_font.clone())
            });
        let mut format_toolbar = div().h_flex().flex_wrap().gap_1().items_center();
        for (id, label, icon, name) in [
            ("format-bold", "粗体", IconName::Bold, "edit/style/bold"),
            (
                "format-italic",
                "斜体",
                IconName::Italic,
                "edit/style/italic",
            ),
            (
                "format-underline",
                "下划线",
                IconName::Underline,
                "edit/style/underline",
            ),
            (
                "format-strikeout",
                "删除线",
                IconName::Strikethrough,
                "edit/style/strikeout",
            ),
            ("format-font", "字体", IconName::ALargeSmall, "edit/font"),
            (
                "format-primary",
                "颜色",
                IconName::Palette,
                "edit/color/primary",
            ),
            (
                "split-preserve",
                "光标处分行",
                IconName::Scissors,
                "edit/line/split/preserve",
            ),
            ("copy-lines", "复制行", IconName::Copy, "edit/line/copy"),
            ("cut-lines", "剪切行", IconName::Scissors, "edit/line/cut"),
            ("paste-lines", "粘贴行", IconName::Copy, "edit/line/paste"),
        ] {
            let enabled = self
                .catalog
                .command(name)
                .is_some_and(|command| command.can_invoke());
            format_toolbar = format_toolbar.child(
                Button::new(id)
                    .rounded(gpui_kit::px(999.))
                    .small()
                    .px(gpui_kit::px(13.))
                    .py(gpui_kit::px(3.))
                    .icon(icon)
                    .accessibility_label(label)
                    .tooltip(label)
                    .ghost()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":name}), cx)
                    })),
            );
        }
        let row_count = visible.len();
        let properties_rail =
            rail.map(|width| self.render_properties(Some(width), cx).into_any_element());
        let editor_height = self.subtitle_editor_height;
        // Header, formatting bar and the text box stay in one column so the
        // inspector can sit beside them as a second column.
        let editor_content = div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .gap_2()
            .when(rail.is_some(), |content| content.py_3().pl_3().pr_2())
            .when(rail.is_none(), |content| content.px_3().py_3())
            .child(
                        div()
                            .h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .font_semibold()
                                    .child("文本编辑"),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(muted)
                                    .child(format!("已选 {} 行", self.snapshot.selected.len())),
                            )
                            .child(
                                Button::new("shrink-subtitle-editor").rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                                    .child(Icon::new(IconName::ChevronUp).size(px(15.)))
                                    .accessibility_label("缩小字幕编辑区")
                                    .tooltip("缩小字幕编辑区")
                                    .ghost()

                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.subtitle_editor_height =
                                            (view.subtitle_editor_height - 24.).max(165.);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("grow-subtitle-editor").rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                                    .child(Icon::new(IconName::ChevronDown).size(px(15.)))
                                    .accessibility_label("增大字幕编辑区")
                                    .tooltip("增大字幕编辑区")
                                    .ghost()

                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.subtitle_editor_height =
                                            (view.subtitle_editor_height + 24.).min(360.);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                Button::new("apply-edit").rounded(gpui_kit::px(999.)).small().px(gpui_kit::px(13.)).py(gpui_kit::px(3.))
                                    .icon(IconName::Check)
                                    .label("应用文本")
                                    .ghost()

                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.selected_line;
                                        let text = this.editor.read(cx).value().to_string();
                                        this.invoke(json!({"op":"edit", "id":id, "ids":this.snapshot.selected, "text":text}), cx);
                                    })),
                            ),
                    )
                    .child(format_toolbar)
                    .child(editor.flex_1().min_h(px(40.)));
        let editor_section = div()
            .id("subtitle-editor-section")
            .flex_shrink_0()
            .min_w_0()
            .when(rail.is_some(), |section| section.h_flex().items_stretch())
            .when(rail.is_none(), |section| section.v_flex())
            .rounded_t(px(crate::workspace_theme::RADIUS_LG - 1.))
            .bg(p.surface)
            .border_b_1()
            .border_color(border)
            .child(editor_content)
            .children(properties_rail);
        let subtitle_list = div()
            .v_flex()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .child(
                div()
                    .h_flex()
                    .h(px(44.))
                    .px_3()
                    .gap_2()
                    .items_center()
                    .flex_shrink_0()
                    // The card clips to a rectangle, so a tinted strip needs its
                    // own rounded corners to stay inside the card outline.
                    .rounded_t(px(crate::workspace_theme::RADIUS_LG - 1.))
                    .bg(p.raised)
                    .border_b_1()
                    .border_color(p.divider)
                    .child(
                        Icon::new(IconName::Captions)
                            .size(px(15.))
                            .text_color(p.muted),
                    )
                    .child(div().text_sm().font_semibold().child("字幕列表"))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(p.muted)
                            .child(format!("{} 行", row_count)),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("subtitle-columns")
                            .rounded(gpui_kit::px(999.))
                            .small()
                            .px(gpui_kit::px(13.))
                            .py(gpui_kit::px(3.))
                            .icon(IconName::Columns3)
                            .label("列")
                            .tooltip("添加或移除字幕列表的列")
                            .ghost()
                            .dropdown_menu(move |mut menu, _, _| {
                                let weak = menu_weak.clone();
                                for column in SubtitleColumn::optional() {
                                    let weak = weak.clone();
                                    let active = active_columns.contains(&column);
                                    menu = menu.item(
                                        PopupMenuItem::new(column.label())
                                            .checked(active)
                                            .on_click(move |_, _, cx| {
                                                let _ = weak.update(cx, |view, cx| {
                                                    view.toggle_subtitle_column(column);
                                                    cx.notify();
                                                });
                                            }),
                                    );
                                }
                                menu
                            }),
                    )
                    .child(
                        Input::new(&self.search)
                            .id("subtitle-search")
                            .prefix(Icon::new(IconName::Search).size(px(13.)))
                            .w(px(190.))
                            .min_w_0()
                            .flex_shrink(1.),
                    ),
            )
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_3()
                    .items_center()
                    .rounded_t(px(crate::workspace_theme::RADIUS_LG - 1.))
                    .bg(header_bg)
                    .border_b_1()
                    // Rows reserve 2px on the left for the active-line accent;
                    // the header needs the same inset or its labels sit 2px off.
                    .border_l_2()
                    .border_color(border)
                    .children(columns.iter().map(|(column, width)| {
                        let column = *column;
                        let width = *width;
                        div()
                            .h_flex()
                            // The whole header cell is the resize handle: a
                            // narrow grip inside it never received the drag.
                            .h_full()
                            .items_center()
                            .justify_between()
                            .w(px(width))
                            .flex_shrink_0()
                            .mr(px(COLUMN_GAP))
                            .text_size(px(secondary_size))
                            .text_color(muted)
                            .child(column.label())
                            .cursor_col_resize()
                            .rounded(px(crate::workspace_theme::RADIUS_SM))
                            .hover(|style| style.bg(p.raised))
                            .on_mouse_down(
                                gpui_kit::MouseButton::Left,
                                cx.listener(
                                    move |view, event: &gpui_kit::MouseDownEvent, _, cx| {
                                        view.layout_drag =
                                            Some(WorkspaceResizeDrag::SubtitleColumn {
                                                column,
                                                start_x: event.position.x,
                                                start_width: width,
                                            });
                                        cx.stop_propagation();
                                        cx.notify();
                                    },
                                ),
                            )
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(MIN_TEXT_COLUMN))
                            .text_size(px(secondary_size))
                            .text_color(muted)
                            .child("字幕文本"),
                    ),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(90.))
                    .child(
                        // The grid needs its own width to decide whether the
                        // columns still fit, and layout only knows it here.
                        canvas(
                            move |bounds, _, cx| {
                                let width = f32::from(bounds.size.width);
                                let _ = size_weak.update(cx, |view, cx| {
                                    if (view.subtitle_grid_width - width).abs() > 1. {
                                        view.subtitle_grid_width = width;
                                        cx.notify();
                                    }
                                });
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        uniform_list(
                            "subtitle-rows",
                            row_count,
                            cx.processor(move |_, range: Range<usize>, _, _| {
                                range
                                    .map(|index| {
                                        let line = visible[index].clone();
                                        let is_active = selected.contains(&line.id);
                                        let row_bg = if is_active && line.comment {
                                            selected_comment_bg
                                        } else if is_active {
                                            selected_bg
                                        } else if line.comment {
                                            comment_bg
                                        } else if highlight_visible
                                            && line.start <= video_time
                                            && video_time < line.end
                                        {
                                            inframe_bg
                                        } else if line.row % 2 == 1 {
                                            stripe
                                        } else {
                                            line_bg
                                        };
                                        let weak = weak.clone();
                                        div()
                                        .id(format!("subtitle-row-{}", line.id))
                                        .h(px(row_height))
                                        .text_size(px(grid_size))
                                        .whitespace_nowrap()
                                        .when(!grid_font.is_empty(), |row| {
                                            row.font_family(grid_font.clone())
                                        })
                                        .w_full()
                                        .h_flex()
                                        .items_center()
                                        .px_3()
                                        .bg(row_bg)
                                        .border_b_1()
                                        .border_color(border)
                                        .border_l_2()
                                        .border_color(if is_active { p.accent } else { border })
                                        .cursor_pointer()
                                        .on_mouse_down(gpui_kit::MouseButton::Left, {
                                            let weak = weak.clone();
                                            let id = line.id;
                                            move |event: &gpui_kit::MouseDownEvent, window, cx| {
                                                let _ = weak.update(cx, |view, cx| {
                                                    if focus_grid {
                                                        view.focus_handle.focus(window, cx);
                                                    }
                                                    view.select_lines(
                                                        id,
                                                        event.modifiers.shift,
                                                        event.modifiers.control
                                                            || event.modifiers.platform,
                                                        cx,
                                                    );
                                                });
                                            }
                                        })
                                        .children(columns.iter().map(|(column, width)| {
                                            let column = *column;
                                            subtitle_cell(
                                                column,
                                                *width,
                                                &line,
                                                secondary_size,
                                                time_font.clone(),
                                                muted,
                                                accent,
                                            )
                                        }))
                                        .child(
                                            div()
                                                .flex_1()
                                                .min_w(px(MIN_TEXT_COLUMN))
                                                .overflow_hidden()
                                                .text_ellipsis()
                                                .text_size(px(grid_size))
                                                .text_color(if is_active {
                                                    selected_foreground
                                                } else {
                                                    foreground
                                                })
                                                .child(line.display_text),
                                        )
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.list_scroll_handle)
                        .size_full(),
                    ),
            );
        div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .min_h(px(300.))
            .overflow_hidden()
            .rounded(px(crate::workspace_theme::RADIUS_LG))
            .border_1()
            .border_color(p.border)
            .bg(line_bg)
            .shadow(crate::page::card_shadow(dark))
            .child(editor_section.h(px(editor_height)).flex_shrink_0())
            .child(
                div()
                    .id("subtitle-editor-list-resize-handle")
                    .aria_label("调整字幕编辑区高度")
                    .h(px(8.))
                    .w_full()
                    .flex_shrink_0()
                    .h_flex()
                    .items_center()
                    .justify_center()
                    .cursor_row_resize()
                    .bg(p.surface)
                    .child(div().w(px(36.)).h(px(2.)).rounded(px(1.)).bg(p.divider))
                    .on_mouse_down(
                        gpui_kit::MouseButton::Left,
                        cx.listener(|view, event: &gpui_kit::MouseDownEvent, _, cx| {
                            view.layout_drag = Some(WorkspaceResizeDrag::SubtitleEditor {
                                start_y: event.position.y,
                                start_height: view.subtitle_editor_height,
                            });
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    ),
            )
            .child(subtitle_list.flex_1().min_h_0())
    }
}

fn subtitle_cell(
    column: SubtitleColumn,
    width: f32,
    line: &Line,
    secondary_size: f32,
    time_font: gpui_kit::SharedString,
    muted: gpui_kit::Hsla,
    accent: gpui_kit::Hsla,
) -> gpui_kit::AnyElement {
    let cell = || {
        div()
            .w(px(width))
            .flex_shrink_0()
            .mr(px(COLUMN_GAP))
            .text_size(px(secondary_size))
    };
    match column {
        SubtitleColumn::IdLayer => cell()
            .text_color(muted)
            .child(format!("{:03} / {}", line.row + 1, line.layer))
            .into_any_element(),
        SubtitleColumn::Time => cell()
            .font_family(time_font)
            .text_color(muted)
            .child(format!(
                "{} → {}",
                format_time(line.start),
                format_time(line.end)
            ))
            .into_any_element(),
        SubtitleColumn::StyleActor => {
            // Stack style over actor and centre the block, so a single-line
            // cell lines up with the other columns instead of sitting on the
            // top edge.
            let mut stacked = cell()
                .v_flex()
                .justify_center()
                .min_w_0()
                .overflow_hidden()
                .child(
                    div()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(accent)
                        .child(line.style.clone()),
                );
            if !line.actor.is_empty() {
                stacked = stacked.child(
                    div()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_color(muted)
                        .child(line.actor.clone()),
                );
            }
            stacked.into_any_element()
        }
        SubtitleColumn::Layer => cell()
            .text_color(muted)
            .child(line.layer.to_string())
            .into_any_element(),
        SubtitleColumn::Effect => cell()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .text_color(muted)
            .child(line.effect.clone())
            .into_any_element(),
        SubtitleColumn::Actor => cell()
            .min_w_0()
            .overflow_hidden()
            .text_ellipsis()
            .text_color(muted)
            .child(line.actor.clone())
            .into_any_element(),
        SubtitleColumn::Comment => cell()
            .text_color(if line.comment { accent } else { muted })
            .child(if line.comment { "注释" } else { "对白" })
            .into_any_element(),
    }
}

fn format_time(ms: i64) -> String {
    let total = ms.max(0);
    format!(
        "{}:{:02}:{:02}.{:02}",
        total / 3600000,
        (total / 60000) % 60,
        (total / 1000) % 60,
        (total % 1000) / 10
    )
}

fn parse_time(value: &str) -> Result<i64> {
    let parts: Vec<_> = value.trim().split(':').collect();
    if !(2..=3).contains(&parts.len()) {
        return Err(anyhow!("请输入 时:分:秒.百分秒"));
    }
    let seconds: Vec<_> = parts[parts.len() - 1].split('.').collect();
    if seconds.len() > 2 {
        return Err(anyhow!("无效的时间码"));
    }
    let sec = seconds[0].parse::<i64>()?;
    let min = parts[parts.len() - 2].parse::<i64>()?;
    let hour = if parts.len() == 3 {
        parts[0].parse::<i64>()?
    } else {
        0
    };
    let fraction = seconds.get(1).copied().unwrap_or("");
    if hour < 0
        || min < 0
        || sec < 0
        || min >= 60
        || sec >= 60
        || fraction.len() > 3
        || !fraction.chars().all(|ch| ch.is_ascii_digit())
    {
        return Err(anyhow!("无效的时间码"));
    }
    let ms = if fraction.is_empty() {
        0
    } else {
        fraction.parse::<i64>()? * 10_i64.pow(3 - fraction.len() as u32)
    };
    hour.checked_mul(3600000)
        .and_then(|total| total.checked_add(min * 60000 + sec * 1000 + ms))
        .filter(|total| *total <= i32::MAX as i64 - 10)
        .ok_or_else(|| anyhow!("时间码超出范围"))
}

fn set_status(
    weak: &gpui_kit::WeakEntity<Workspace>,
    cx: &mut gpui_kit::AsyncApp,
    message: String,
) {
    if let Some(view) = weak.upgrade() {
        let _ = view.update(cx, |view, cx| {
            view.status = message;
            cx.notify();
        });
    }
}

async fn invoke_weak(
    bridge: &Bridge,
    weak: &gpui_kit::WeakEntity<Workspace>,
    cx: &mut gpui_kit::AsyncApp,
    payload: Value,
) -> bool {
    match bridge.call(payload).await {
        Ok(_) => match bridge.snapshot().await {
            Ok(snapshot) => {
                if let Some(view) = weak.upgrade() {
                    let _ = view.update(cx, |view, cx| {
                        view.status = "已同步当前项目".into();
                        view.apply_snapshot(snapshot, cx);
                    });
                }
                true
            }
            Err(error) => {
                set_status(weak, cx, format!("同步失败：{error}"));
                false
            }
        },
        Err(error) => {
            set_status(weak, cx, format!("操作失败：{error}"));
            false
        }
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(line) = self
            .snapshot
            .lines
            .iter()
            .find(|line| line.id == self.selected_line)
        {
            if self.editor_line_id != line.id {
                let text = line.text.clone();
                self.editor
                    .update(cx, |state, cx| state.set_value(text, window, cx));
                self.editor_line_id = line.id;
                self.editor_dirty_line = -1;
            } else if self.editor_dirty_line != line.id
                && self.editor.read(cx).value().as_ref() != line.text
            {
                // A change made in Aegisub (undo, Lua, or the original editor) replaces the local draft.
                let text = line.text.clone();
                self.editor
                    .update(cx, |state, cx| state.set_value(text, window, cx));
            }
        }
        if let Some([start, end]) = self.pending_text_selection.take() {
            self.editor
                .update(cx, |state, cx| state.set_selected_range(start..end, cx));
        }
        if let Some(line) = self
            .snapshot
            .lines
            .iter()
            .find(|line| line.id == self.selected_line)
        {
            if self.properties_line_id != line.id || !self.properties_dirty {
                for (key, input) in &self.property_inputs {
                    let value = match *key {
                        "start" => format_time(line.start),
                        "end" => format_time(line.end),
                        "layer" => line.layer.to_string(),
                        "style" => line.style.clone(),
                        "actor" => line.actor.clone(),
                        "effect" => line.effect.clone(),
                        "margin_left" => line.margin[0].to_string(),
                        "margin_right" => line.margin[1].to_string(),
                        _ => line.margin[2].to_string(),
                    };
                    if input.read(cx).value().as_ref() != value {
                        input.update(cx, |state, cx| state.set_value(value, window, cx));
                    }
                }
                self.properties_line_id = line.id;
                self.properties_dirty = false;
            }
        }
        if !self.timeline.dragging
            && !self.timeline.seeking
            && self.timeline.pending_frame.is_none()
        {
            let position =
                self.snapshot.frame as f32 / (self.snapshot.frame_count - 1).max(1) as f32;
            if (self.timeline.slider.read(cx).value().start() - position).abs() > 0.000001 {
                self.timeline
                    .slider
                    .update(cx, |state, cx| state.set_value(position, window, cx));
            }
        }
        if let Some(text) = self.translation.pending_text.take() {
            self.translation
                .editor
                .update(cx, |state, cx| state.set_value(text, window, cx));
        }
        if self.translation.focus_pending && self.active_panel == Panel::Translation {
            self.translation.focus_pending = false;
            self.translation
                .editor
                .update(cx, |state, cx| state.focus(window, cx));
        }
        if self.active_panel == Panel::Styling {
            self.styling.sync(&self.snapshot, window, cx);
        }
        self.document.sync(window, cx);
        self.timing.sync(window, cx);
        self.resample.sync(window, cx);
        self.processor.sync(window, cx);
        self.video_settings.sync(window, cx);
        if self.active_panel == Panel::Spelling {
            self.spelling.sync(window, cx);
        }
        if self.active_panel == Panel::Preferences {
            PreferencesPanel::sync(self, window, cx);
        }
        if self.active_panel == Panel::Hotkeys {
            self.hotkeys.sync(window, cx);
        }
        if self.active_panel == Panel::Search {
            self.find_panel.sync(window, cx);
        }
        let dark = Theme::global(cx).is_dark();
        let p = Palette::new(dark);
        let canvas = p.canvas;
        let border = p.divider;
        let muted = p.muted;
        let root = div()
            .id("aegisub-gpui-workspace")
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(Self::handle_key))
            .on_drop(cx.listener(|view, paths: &ExternalPaths, _, cx| {
                view.open_dropped(paths, cx)
            }))
            .on_mouse_move(cx.listener(|view, event: &gpui_kit::MouseMoveEvent, _, cx| {
                if let Some(drag) = view.layout_drag {
                    if event.pressed_button == Some(gpui_kit::MouseButton::Left) {
                        match drag {
                            WorkspaceResizeDrag::SubtitleEditor {
                                start_y,
                                start_height,
                            } => {
                                view.subtitle_editor_height = (start_height
                                    + f32::from(event.position.y - start_y))
                                    .clamp(165., 360.);
                            }
                            WorkspaceResizeDrag::AudioPanel {
                                start_y,
                                start_height,
                            } => {
                                view.audio_panel_height = (start_height
                                    + f32::from(event.position.y - start_y))
                                    .clamp(60., 320.);
                            }
                            WorkspaceResizeDrag::AudioPage {
                                start_y,
                                start_height,
                            } => {
                                view.audio_page_height = (start_height
                                    + f32::from(event.position.y - start_y))
                                    .clamp(120., 640.);
                            }
                            WorkspaceResizeDrag::ColumnSplit {
                                start_x,
                                start_width,
                            } => {
                                view.column_split = Some(
                                    (start_width + f32::from(event.position.x - start_x))
                                        .clamp(300., 1400.),
                                );
                            }
                            WorkspaceResizeDrag::SubtitleColumn {
                                column,
                                start_x,
                                start_width,
                            } => {
                                let width = (start_width
                                    + f32::from(event.position.x - start_x))
                                    .clamp(40., 600.);
                                view.set_subtitle_column_width(column, width);
                            }
                            WorkspaceResizeDrag::VideoPreview {
                                start_y,
                                start_height,
                                max_height,
                            } => {
                                view.video_preview_height = Some(
                                    (start_height + f32::from(event.position.y - start_y))
                                        .clamp(120., max_height),
                                );
                            }
                        }
                        cx.notify();
                    } else {
                        view.layout_drag = None;
                    }
                }
                if view.visual.pan_last.is_some() {
                    if event.pressed_button == Some(gpui_kit::MouseButton::Middle) {
                        view.update_video_pan(event.position, cx);
                    } else { view.visual.pan_last = None; }
                }
                if view.active_panel == Panel::Visual
                    && view.visual.dragging
                    && event.pressed_button == Some(gpui_kit::MouseButton::Left)
                    && !view.visual.viewport_bounds.borrow().contains(&event.position)
                {
                    let (x, y) = view.visual.local(event.position);
                    view.queue_visual(json!({"action":"move", "x":x,"y":y,"shift":event.modifiers.shift,"control":event.modifiers.control || event.modifiers.platform,"alt":event.modifiers.alt}), cx);
                }
            }))
            .on_mouse_up(
                gpui_kit::MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    if view.layout_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .overflow_hidden()
            .h_flex().items_stretch()
            .bg(canvas)
            .text_color(p.text);
        let mut body = div()
            .v_flex()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_hidden();
        if self.active_panel == Panel::Subtitles {
            let side_by_side = f32::from(window.viewport_size().width) >= 900.;
            let sidebar = if self
                .sidebar_collapsed
                .unwrap_or(f32::from(window.viewport_size().width) < 1150.)
            {
                60.
            } else {
                164.
            };
            let body_width = (f32::from(window.viewport_size().width) - sidebar - 48.).max(320.);
            let column_width = self
                .column_split
                .unwrap_or((body_width - 12.) / 2.)
                .clamp(280., (body_width - 280.).max(280.));
            // The inspector moves beside the text editor once the right column
            // can hold both without squeezing the editor into a sliver.
            // The inspector only turns into a side rail while the editor still
            // keeps a workable width next to it.
            let right_column_width = body_width - column_width - 16.;
            let rail_width = (right_column_width * 0.42).clamp(228., 320.);
            let properties_rail = side_by_side
                .then_some(rail_width)
                .filter(|width| right_column_width - width - 12. >= 288.);
            let divider = p.divider;
            let audio_handle = div()
                .id("audio-panel-resize-handle")
                .aria_label("调整音频波形高度")
                .role(gpui_kit::Role::Splitter)
                .h(px(6.))
                .w_full()
                .flex_shrink_0()
                .h_flex()
                .items_center()
                .justify_center()
                .cursor_row_resize()
                .child(div().w(px(48.)).h(px(3.)).rounded(px(2.)).bg(divider))
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(|view, event: &gpui_kit::MouseDownEvent, _, cx| {
                        view.layout_drag = Some(WorkspaceResizeDrag::AudioPanel {
                            start_y: event.position.y,
                            start_height: view.audio_panel_height,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
            let column_handle = div()
                .id("workspace-column-resize-handle")
                .aria_label("调整视频与字幕列表宽度")
                .role(gpui_kit::Role::Splitter)
                .w(px(6.))
                .h_full()
                .flex_shrink_0()
                .v_flex()
                .items_center()
                .justify_center()
                .cursor_col_resize()
                .child(div().h(px(48.)).w(px(3.)).rounded(px(2.)).bg(divider))
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(move |view, event: &gpui_kit::MouseDownEvent, _, cx| {
                        view.layout_drag = Some(WorkspaceResizeDrag::ColumnSplit {
                            start_x: event.position.x,
                            start_width: column_width,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
            let video_handle = div()
                .id("video-preview-resize-handle")
                .aria_label("调整视频预览高度")
                .role(gpui_kit::Role::Splitter)
                .h(px(6.))
                .w_full()
                .flex_shrink_0()
                .h_flex()
                .items_center()
                .justify_center()
                .cursor_row_resize()
                .child(div().w(px(48.)).h(px(3.)).rounded(px(2.)).bg(divider))
                .on_mouse_down(
                    gpui_kit::MouseButton::Left,
                    cx.listener(|view, event: &gpui_kit::MouseDownEvent, window, cx| {
                        let current = view
                            .video_preview_height
                            .unwrap_or_else(|| view.last_preview_height.get());
                        let max_height =
                            (f32::from(window.viewport_size().height) - 620.).max(140.);
                        view.layout_drag = Some(WorkspaceResizeDrag::VideoPreview {
                            start_y: event.position.y,
                            start_height: current,
                            max_height,
                        });
                        cx.stop_propagation();
                        cx.notify();
                    }),
                );
            let mut columns = div()
                .h_flex()
                .items_stretch()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .gap_2()
                .child(
                    div()
                        .v_flex()
                        .when(side_by_side, |column| column.w(px(column_width)).h_full())
                        .when(!side_by_side, |column| column.flex_1())
                        .flex_shrink_0()
                        .min_w_0()
                        .min_h_0()
                        .gap_1()
                        .child(self.render_player(
                            window,
                            side_by_side,
                            properties_rail.is_some(),
                            cx,
                        ))
                        // The stacked layout has no spare height to trade
                        // between the preview and the properties.
                        .when(side_by_side && properties_rail.is_none(), |column| {
                            column.child(video_handle)
                        })
                        // Filling the column (instead of a fixed height) lets
                        // the preview divider trade space with the properties.
                        .when(properties_rail.is_none(), |column| {
                            column.child(self.render_properties(None, cx))
                        }),
                )
                .when(side_by_side, |row| row.child(column_handle))
                .child(
                    div()
                        .v_flex()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .overflow_hidden()
                        .child(self.render_subtitle_panel(properties_rail, cx)),
                );
            if !side_by_side {
                columns = columns.flex_col();
            }
            body = body.child(
                div()
                    .v_flex()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .overflow_hidden()
                    .gap_2()
                    .p_4()
                    .child(self.audio.render_compact(
                        &self.snapshot.audio,
                        self.snapshot.video,
                        &self.snapshot.appearance,
                        self.audio_panel_height,
                        cx,
                    ))
                    .child(audio_handle)
                    .child(columns),
            );
        } else if self.active_panel == Panel::Visual {
            body = body.child(
                page::page(cx, "visual-panel")
                    .child(self.visual.toolbar(cx))
                    .child(self.render_player(window, false, false, cx))
                    .child(page::hint(
                        cx,
                        "双击十字定位；拖动控制点调整，Shift 约束，Ctrl/⌘ 多选。",
                    )),
            );
        } else if self.active_panel == Panel::Commands {
            body = body.child(self.render_commands(cx));
        } else if self.active_panel == Panel::Audio {
            body = body.child(self.audio.render(
                &self.snapshot.audio,
                self.snapshot.video,
                &self.snapshot.appearance,
                self.audio_page_height,
                cx,
            ));
        } else if self.active_panel == Panel::Styling {
            body = body.child(
                self.styling
                    .render(&self.snapshot, &self.document.details, cx),
            );
        } else if self.active_panel == Panel::Translation {
            body = body.child(self.translation.render(
                self.snapshot.video,
                self.snapshot.audio.loaded,
                cx,
            ));
        } else if self.active_panel == Panel::Automation {
            body = body.child(self.automation.render(&self.catalog, cx));
        } else if self.active_panel == Panel::Spelling {
            body = body.child(self.spelling.render(cx));
        } else if self.active_panel == Panel::Files {
            body = body.child(self.files.render(cx));
        } else if self.active_panel == Panel::Preferences {
            body = body.child(self.preferences.render(cx));
        } else if self.active_panel == Panel::Hotkeys {
            body = body.child(self.hotkeys.render(&self.catalog, cx));
        } else if self.active_panel == Panel::Attachments {
            body = body.child(self.attachments.render(cx));
        } else if self.active_panel == Panel::VideoSettings {
            body = body.child(self.video_settings.render(cx));
        } else if self.active_panel == Panel::Paste {
            body = body.child(self.paste.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Search {
            body = body.child(self.find_panel.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Processor {
            body = body.child(self.processor.render(cx));
        } else if self.active_panel == Panel::Resample {
            body = body.child(self.resample.render(cx));
        } else if self.active_panel == Panel::Timing {
            body = body.child(self.timing.render(&self.snapshot, window, cx));
        } else if self.active_panel == Panel::Styles {
            body = body.child(self.document.render_styles(cx));
        } else {
            body = body.child(self.document.render_project(cx));
        }
        let body = body.child(
            div()
                .h_flex()
                .h(px(26.))
                .flex_shrink_0()
                .px_3()
                .items_center()
                .justify_between()
                .bg(p.surface)
                .border_t_1()
                .border_color(border)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_size(px(11.))
                        .text_color(muted)
                        .child(self.status.clone()),
                )
                .child(
                    div()
                        .h_flex()
                        .gap_3()
                        .text_xs()
                        .text_color(muted)
                        .child(if self.snapshot.video {
                            format!(
                                "视频帧 {} / {}",
                                self.snapshot.frame + 1,
                                self.snapshot.frame_count
                            )
                        } else {
                            "无视频".into()
                        })
                        .child(if self.snapshot.file.is_empty() {
                            "未命名字幕".to_owned()
                        } else {
                            std::path::Path::new(&self.snapshot.file)
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("未命名字幕")
                                .to_owned()
                        }),
                ),
        );
        root.child(self.render_menu(window, cx)).child(
            div()
                .v_flex()
                .h_full()
                .overflow_hidden()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .when(!cfg!(target_os = "macos"), |bar| {
                    bar.child(self.render_legacy_menus(cx))
                })
                .when(self.snapshot.show_toolbar, |v| {
                    v.child(self.render_toolbar(cx))
                })
                .child(body),
        )
    }
}

/// Opens a workspace window wired to the bridge. Every window keeps its own
/// connection, so File > New Window gives an independent view of the document.
fn open_workspace(
    cx: &mut App,
    bridge: Bridge,
    bounds: Option<gpui_kit::Bounds<gpui_kit::Pixels>>,
) -> Result<gpui_kit::WindowHandle<gpui_kit::component::Root>> {
    let window_bounds = match bounds {
        Some(bounds) => WindowBounds::Windowed(bounds),
        None => WindowBounds::centered(size(px(1420.), px(920.)), cx),
    };
    cx.open_window(
        WindowOptions {
            window_bounds: Some(window_bounds),
            titlebar: Some(gpui_kit::TitlebarOptions {
                title: Some("Aegisub".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        move |window, cx| {
            cx.activate(true);
            window.activate_window();
            let workspace = cx.new(|cx| Workspace::new(bridge.clone(), window, cx));
            let menu_target = workspace.downgrade();
            cx.on_action(move |action: &RunCommand, cx| {
                let name = action.name.clone();
                if let Some(view) = menu_target.upgrade() {
                    let _ = view.update(cx, |view, cx| {
                        view.invoke(json!({"op":"command", "name":name}), cx)
                    });
                }
            });
            workspace.update(cx, |view, cx| {
                view.refresh_catalog(cx);
                view.refresh_document(cx);
            });
            let weak = workspace.downgrade();
            let poll_bridge = bridge.clone();
            cx.spawn(async move |cx| {
                let mut consecutive_failures = 0;
                let mut playing = false;
                let mut last_document_snapshot = Instant::now();
                loop {
                    if weak.upgrade().is_none() {
                        break;
                    }
                    let read_document =
                        !playing || last_document_snapshot.elapsed() >= Duration::from_millis(350);
                    let result = if read_document {
                        poll_bridge.snapshot().await.map(Ok)
                    } else {
                        poll_bridge.playback().await.map(Err)
                    };
                    match result {
                        Ok(Ok(snapshot)) => {
                            consecutive_failures = 0;
                            last_document_snapshot = Instant::now();
                            playing = snapshot.playing || snapshot.audio.playing;
                            if let Some(view) = weak.upgrade() {
                                let _ = view.update(cx, |view, cx| {
                                    if view.status.starts_with("Aegisub 连接中断") {
                                        view.status = "已重新连接 Aegisub".into();
                                    }
                                    view.apply_snapshot(snapshot, cx);
                                    view.refresh_frame(cx);
                                });
                            }
                        }
                        Ok(Err(playback)) => {
                            consecutive_failures = 0;
                            playing = playback.playing || playback.audio_playing;
                            if let Some(view) = weak.upgrade() {
                                let _ = view.update(cx, |view, cx| {
                                    view.apply_playback(playback, cx);
                                    view.refresh_frame(cx);
                                });
                            }
                        }
                        Err(error) => {
                            consecutive_failures += 1;
                            set_status(&weak, cx, format!("Aegisub 连接中断：{error}"));
                            if consecutive_failures >= 5 {
                                cx.update(|app| app.quit());
                                break;
                            }
                        }
                    }
                    cx.background_executor()
                        .timer(Duration::from_millis(if playing { 40 } else { 350 }))
                        .await;
                }
            })
            .detach();
            cx.new(|cx| Root::new(workspace, window, cx))
        },
    )
}

fn run(endpoint: String, token: String) -> Result<()> {
    application()
        .with_assets(workspace_theme::StudioAssets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);
            workspace_theme::apply(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            let bridge = Bridge { endpoint, token };
            if let Err(error) = open_workspace(cx, bridge, None) {
                eprintln!("Could not create GPUI window: {error:#}");
                cx.quit();
            }
            cx.activate(true);
        });
    Ok(())
}

fn main() -> Result<()> {
    let mut endpoint = None;
    let mut token = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--bridge" => endpoint = args.next(),
            "--token" => token = args.next(),
            _ => {}
        }
    }
    let endpoint =
        endpoint.context("missing --bridge endpoint; launch this prototype from Aegisub")?;
    let token = token.context("missing --token; launch this prototype from Aegisub")?;
    run(endpoint, token)
}

#[cfg(test)]
mod drop_tests {
    use super::dropped_kind;
    use std::path::Path;

    #[test]
    fn classifies_dropped_files_like_the_open_dialogs() {
        assert_eq!(dropped_kind(Path::new("/tmp/a.ASS")), Some("subtitle"));
        assert_eq!(dropped_kind(Path::new("/tmp/a.srt")), Some("subtitle"));
        assert_eq!(dropped_kind(Path::new("/tmp/a.mkv")), Some("video"));
        assert_eq!(dropped_kind(Path::new("/tmp/a.flac")), Some("audio"));
        assert_eq!(dropped_kind(Path::new("/tmp/a.txt")), Some("timecodes"));
        assert_eq!(dropped_kind(Path::new("/tmp/a.png")), None);
        assert_eq!(dropped_kind(Path::new("/tmp/no-extension")), None);
    }
}
