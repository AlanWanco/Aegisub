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

use visual_panel::{VisualPanel, VisualReply, video_base_bounds, zoomed_video_bounds};

use video_timeline::VideoTimeline;

use document_panels::{DocumentDetails, DocumentPanels, LibraryDetails};

use command_catalog::{Catalog, populate_menu};
use std::{collections::BTreeMap, net::SocketAddr, ops::Range, sync::Arc, time::Duration};

use anyhow::{Context as _, Result, anyhow};
use async_net::TcpStream;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use gpui_kit::{
    App, AppContext, Context, Corners, Entity, Focusable, Image, IntoElement, Render, ScrollDelta,
    Subscription, Window, WindowBounds, WindowOptions, canvas, div, prelude::*, px, rgb, size,
    uniform_list,
};
use gpui_kit::{
    base::Disableable,
    component::{
        Root, Sizable, StyledExt, Theme, ThemeMode, WindowExt,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
        menu::DropdownMenu,
        scroll::ScrollableElement,
    },
    platform::application,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::atomic::{AtomicU64, Ordering};

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

#[derive(Clone, Debug, Default, Deserialize)]
struct Appearance {
    #[serde(default)]
    colors: BTreeMap<String, u32>,
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
        self.colors
            .get(name)
            .map(|color| gpui_kit::rgba(*color).into())
            .unwrap_or(fallback)
    }
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

struct Workspace {
    bridge: Bridge,
    focus_handle: gpui_kit::FocusHandle,
    snapshot: Snapshot,
    frame_image: Option<Arc<Image>>,
    frame_image_frame: i64,
    frame_request_inflight: bool,
    frame_request_generation: u64,
    video_epoch: u64,
    list_scroll_handle: gpui_kit::UniformListScrollHandle,
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
    selected_line: i64,
    status: String,
    _editor_events: Subscription,
    _search_events: Subscription,
}

impl Workspace {
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
            self.audio.image = None;
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
        let frame_changed = snapshot.frame != self.snapshot.frame;
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
        if media_changed || snapshot.revision != self.snapshot.revision {
            self.frame_image = None;
            self.frame_image_frame = -1;
            self.frame_request_generation += 1;
            self.frame_request_inflight = false;
        }
        self.video_epoch = snapshot.video_epoch;
        if !media_changed && frame_changed && self.frame_image_frame >= 0 {
            self.frame_image = None;
            self.frame_image_frame = -1;
        }
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
            self.frame_image = None;
            self.frame_image_frame = -1;
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
        if self.active_panel == Panel::Audio {
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

    fn invoke(&mut self, mut payload: Value, cx: &mut Context<Self>) {
        if payload["op"] == "command" {
            match payload["name"].as_str() {
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
                        view.audio.image = image;
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
                    Ok(catalog) => view.catalog = Arc::new(catalog),
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
            self.frame_image = None;
            self.frame_image_frame = -1;
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
                    let bytes = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded) {
                        Ok(bytes) => bytes,
                        Err(error) => {
                            if let Some(view) = weak.upgrade() { let _ = view.update(cx, |view, cx| { if view.frame_request_generation == generation { view.frame_request_inflight = false; view.status = format!("视频帧解码失败：{error}"); cx.notify(); } }); }
                            return;
                        }
                    };
                    let image = Arc::new(Image::from_bytes(gpui_kit::ImageFormat::Png, bytes));
                    let frame = reply.frame.unwrap_or(-1);
                    if let Some(view) = weak.upgrade() {
                        let _ = view.update(cx, |view, cx| {
                            if view.frame_request_generation != generation { return; }
                            view.frame_request_inflight = false;
                            if view.snapshot.video && view.snapshot.frame == frame && view.video_epoch == request_epoch {
                                view.frame_image = Some(image);
                                view.frame_image_frame = frame;
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
        if event.prefer_character_input {
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
        // Preserve editing/navigation shortcuts inside form fields and search inputs.
        if input_focused && !editor_focused && !command_search_focused {
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
        let dark = Theme::global(cx).is_dark();
        let background = if dark { rgb(0x17191d) } else { rgb(0xf3f4f6) };
        let muted = if dark { rgb(0x858b96) } else { rgb(0x69717d) };
        div()
            .h_flex()
            .flex_shrink_0()
            .gap_1()
            .px_3()
            .py_2()
            .bg(background)
            .border_b_1()
            .border_color(if dark { rgb(0x30343b) } else { rgb(0xdde1e7) })
            .child(
                Button::new("open-subtitles")
                    .label("打开字幕")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| view.pick_file("subtitle", cx))),
            )
            .child(
                Button::new("save-subtitles")
                    .label("保存")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.invoke(json!({"op":"command", "name":"subtitle/save"}), cx)
                    })),
            )
            .child(
                Button::new("undo")
                    .label("撤销")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.invoke(json!({"op":"command", "name":"edit/undo"}), cx)
                    })),
            )
            .child(
                Button::new("redo")
                    .label("重做")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.invoke(json!({"op":"command", "name":"edit/redo"}), cx)
                    })),
            )
            .child(div().w(px(1.)).h(px(22.)).mx_2().bg(if dark {
                rgb(0x343942)
            } else {
                rgb(0xd3d8df)
            }))
            .child(
                Button::new("open-video")
                    .label("打开视频")
                    .ghost()
                    .small()
                    .on_click(cx.listener(|view, _, _, cx| view.pick_file("video", cx))),
            )
            .child(
                Button::new("play-video")
                    .label(if self.snapshot.playing {
                        "暂停"
                    } else {
                        "播放"
                    })
                    .primary()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.snapshot.playing {
                            this.invoke(json!({"op":"command", "name":"video/stop"}), cx);
                        } else {
                            this.invoke(json!({"op":"command", "name":"video/play"}), cx);
                        }
                    })),
            )
            .child(
                Button::new("refresh-frame")
                    .label("刷新帧")
                    .ghost()
                    .small()
                    .disabled(!self.snapshot.video)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.frame_image = None;
                        this.frame_image_frame = -1;
                        this.refresh_frame(cx);
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(if self.snapshot.modified {
                        "未保存更改"
                    } else {
                        "已保存"
                    }),
            )
    }

    fn render_legacy_menus(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut bar = div().h_flex().gap_1().px_3().py_1().flex_shrink_0();
        for entry in self.catalog.menu("main") {
            if entry.special == "window" || entry.submenu == "main/window" {
                continue;
            }
            let catalog = self.catalog.clone();
            let weak = cx.weak_entity();
            let refresh_weak = weak.clone();
            let label = entry.text.replace('&', "");
            let submenu = entry.submenu.clone();
            let entries = if entry.special == "automation" {
                vec![entry]
            } else {
                catalog.menu(&submenu)
            };
            bar = bar.child(
                Button::new(format!("legacy-menu-{label}"))
                    .label(label)
                    .ghost()
                    .small()
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
        bar.child(
            Button::new("command-browser")
                .label("全部命令…")
                .ghost()
                .small()
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
        div().v_flex().flex_1().min_h_0().p_3().gap_2()
            .child(div().h_flex().items_center().gap_3()
                .child(div().font_semibold().child(format!("旧版命令 · {count} 项")))
                .child(Input::new(&self.command_search).id("command-search").w(px(360.)))
                .child(Button::new("refresh-command-catalog").label("刷新状态").small().on_click(cx.listener(|view, _, _, cx| view.refresh_catalog(cx)))))
            .child(div().text_xs().text_color(Theme::global(cx).muted_foreground).child("标有旧版对话框的项目会打开原界面；需旧版控件的项目仍在原窗口使用。快捷键栏为旧版配置，可用项目命令按原配置触发；命令搜索：Ctrl/⌘+Shift+P。"))
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
                        .child(Button::new(format!("invoke-{name}")).label("执行").small().disabled(!command.can_invoke()).on_click(move |_, _, cx| {
                            let _ = weak.update(cx, |view, cx| view.invoke(json!({"op":"command", "name":name}), cx));
                        }))
                }).collect()
            })).flex_1().min_h(px(100.)))
    }

    fn render_properties(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut panel = div()
            .id("line-properties")
            .v_flex()
            .w(px(286.))
            .flex_shrink_0()
            .min_h_0()
            .p_3()
            .gap_2()
            .overflow_y_scrollbar()
            .child(
                div()
                    .font_semibold()
                    .child(format!("行属性 · 已选 {} 行", self.snapshot.selected.len())),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(Theme::global(cx).muted_foreground)
                    .child("修改的字段应用到选中行；时间格式为 时:分:秒.百分秒。"),
            );
        for (key, label) in [
            ("start", "开始"),
            ("end", "结束"),
            ("layer", "层"),
            ("style", "样式"),
            ("actor", "角色"),
            ("effect", "效果"),
            ("margin_left", "左边距"),
            ("margin_right", "右边距"),
            ("margin_vertical", "垂直边距"),
        ] {
            panel = panel.child(
                div()
                    .h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().w(px(64.)).text_sm().child(label))
                    .child(
                        Input::new(&self.property_inputs[key])
                            .id(format!("property-{key}"))
                            .flex_1()
                            .small(),
                    ),
            );
        }
        let commented = self
            .snapshot
            .lines
            .iter()
            .find(|line| line.id == self.selected_line)
            .is_some_and(|line| line.comment);
        panel.child(Button::new("apply-line-fields").label("应用修改字段").primary().small().disabled(self.selected_line < 0).on_click(cx.listener(|view, _, _, cx| view.commit_properties(cx))))
            .child(Button::new("toggle-comment").label(if commented { "改为对白" } else { "改为注释" }).small().disabled(self.selected_line < 0).on_click(cx.listener(move |view, _, _, cx| {
                view.invoke(json!({"op":"line_fields", "ids":view.snapshot.selected, "fields":{"comment":!commented}}), cx);
            })))
            .child(Button::new("save-as").label("另存为…").small().on_click(cx.listener(|view, _, _, cx| view.invoke(json!({"op":"command", "name":"subtitle/save/as"}), cx))))
            .child(div().text_xs().text_color(Theme::global(cx).muted_foreground).child("音频面板可调时；排版面板提供视频画布工具。"))
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let background = if dark { rgb(0x202329) } else { rgb(0xffffff) };
        let active = if dark { rgb(0x303640) } else { rgb(0xe9eef6) };
        let panels = [
            (Panel::Subtitles, "字幕"),
            (Panel::Styles, "样式"),
            (Panel::Project, "项目"),
            (Panel::VideoSettings, "视频"),
            (Panel::Audio, "音频"),
            (Panel::Visual, "排版"),
            (Panel::Translation, "翻译"),
            (Panel::Styling, "配样式"),
            (Panel::Timing, "时间"),
            (Panel::Automation, "脚本"),
            (Panel::Resample, "重采样"),
            (Panel::Processor, "后处理"),
            (Panel::Search, "检索"),
            (Panel::Spelling, "拼写"),
            (Panel::Attachments, "附件"),
            (Panel::Files, "文件"),
            (Panel::Preferences, "设置"),
            (Panel::Commands, "命令"),
        ];
        let mut row = div()
            .h_flex()
            .flex_wrap()
            .gap_1()
            .px_3()
            .bg(background)
            .border_b_1()
            .border_color(if dark { rgb(0x30343b) } else { rgb(0xe2e5e9) });
        for (panel, label) in panels {
            let is_active = panel == self.active_panel;
            row = row.child(
                div()
                    .id(format!("panel-{}", label))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if is_active { active } else { background })
                    .text_color(if is_active {
                        Theme::global(cx).foreground
                    } else {
                        Theme::global(cx).muted_foreground
                    })
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.focus_handle.focus(window, cx);
                        if this.active_panel == Panel::Visual && panel != Panel::Visual {
                            this.visual.dragging = false;
                            this.visual.queue.clear();
                            this.queue_visual(json!({"action":"cancel"}), cx);
                        }
                        this.visual.pan_last = None;
                        this.active_panel = panel;
                        if panel == Panel::Commands {
                            this.refresh_catalog(cx);
                        }
                        if panel == Panel::Styles || panel == Panel::Project {
                            this.refresh_document(cx);
                        }
                        if panel == Panel::Styles {
                            this.refresh_library(cx);
                        }
                        if panel == Panel::VideoSettings {
                            this.video_settings_request("state", cx);
                        }
                        if panel == Panel::Preferences {
                            this.preferences_request(json!({"action":"state"}), cx);
                        }
                        if panel == Panel::Files {
                            this.file_tools_request(json!({"action":"state"}), cx);
                        }
                        if panel == Panel::Attachments {
                            this.attachment_request(json!({"action":"state"}), cx);
                        }
                        if panel == Panel::Spelling {
                            this.spelling_request(json!({"action":"state"}), cx);
                        }
                        if panel == Panel::Search {
                            this.find_panel.focus_pending = true;
                            this.search_request("state", cx);
                        }
                        if panel == Panel::Processor {
                            this.processor_request("state", cx);
                        }
                        if panel == Panel::Resample {
                            this.resample_request("state", cx);
                        }
                        if panel == Panel::Automation {
                            this.automation_request(json!({"action":"state"}), cx);
                        }
                        if panel == Panel::Timing {
                            this.shift_times_request("state", cx);
                        }
                        if panel == Panel::Audio {
                            this.refresh_audio(cx);
                        }
                        if panel == Panel::Translation {
                            this.translation_action("start", cx);
                        }
                        if panel == Panel::Styling {
                            this.styling.line_id = -1;
                            this.styling.focus_pending = true;
                            this.refresh_document(cx);
                            this.styling_action("start", None, cx);
                        }
                        if panel == Panel::Visual {
                            this.visual.drawn = None;
                            this.queue_visual(json!({"action":"draw"}), cx);
                        }
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        row
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

    fn render_player(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let aspect = self.snapshot.aspect as f32;
        let zoom = self.visual.zoom;
        let pan = self.visual.pan;
        let height =
            self.snapshot.video_height * self.snapshot.video_window_zoom / window.scale_factor();
        let display_size = (self.snapshot.video && !self.visual.fit_video)
            .then(|| size(px(height * aspect), px(height)));
        let view_height = if self.visual.fit_video || !self.snapshot.video {
            if self.active_panel == Panel::Visual {
                410.
            } else {
                220.
            }
        } else {
            let max_height = f32::from(window.viewport_size().height)
                - if self.active_panel == Panel::Visual {
                    260.
                } else {
                    510.
                };
            height.clamp(80., max_height.max(80.))
        };
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
        let mut controls = div()
            .h_flex()
            .flex_wrap()
            .gap_1()
            .items_center()
            .px_2()
            .py_1()
            .bg(Theme::global(cx).background)
            .text_color(Theme::global(cx).foreground);
        if self.snapshot.video {
            for (name, label) in [
                ("video/zoom/out", "缩小"),
                ("video/zoom/50", "50%"),
                ("video/zoom/100", "100%"),
                ("video/zoom/200", "200%"),
                ("video/zoom/in", "放大"),
            ] {
                controls = controls.child(
                    Button::new(format!("display-{name}"))
                        .label(label)
                        .small()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.invoke(json!({"op":"command", "name":name}), cx)
                        })),
                );
            }
            controls = controls
                .child(
                    Button::new("video-fit")
                        .label("适应")
                        .small()
                        .when(self.visual.fit_video, |b| b.primary())
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.cancel_visual_drag(cx);
                            view.visual.fit_video = true;
                            view.visual.reset_zoom();
                            cx.notify();
                        })),
                )
                .child(div().text_xs().px_2().child(if self.visual.fit_video {
                    "视频框：适应".into()
                } else {
                    format!("视频框：{:.1}%", self.snapshot.video_window_zoom * 100.)
                }))
                .child(
                    Button::new("video-content-out")
                        .label("画面−")
                        .small()
                        .on_click(cx.listener(|view, _, _, cx| {
                            let bounds = *view.visual.viewport_bounds.borrow();
                            view.adjust_video_view(bounds.center(), view.visual.zoom / 1.125, cx);
                        })),
                )
                .child(div().text_xs().px_2().child(format!("{:.1}%", zoom * 100.)))
                .child(
                    Button::new("video-content-in")
                        .label("画面＋")
                        .small()
                        .on_click(cx.listener(|view, _, _, cx| {
                            let bounds = *view.visual.viewport_bounds.borrow();
                            view.adjust_video_view(bounds.center(), view.visual.zoom * 1.125, cx);
                        })),
                )
                .child(
                    Button::new("video-zoom-reset")
                        .label("复位画面")
                        .small()
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.invoke(json!({"op":"command", "name":"video/reset_pan"}), cx);
                        })),
                );
            for (id, label, hint, x, y) in [
                ("left", "←", "向左平移画面", -20., 0.),
                ("right", "→", "向右平移画面", 20., 0.),
                ("up", "↑", "向上平移画面", 0., -20.),
                ("down", "↓", "向下平移画面", 0., 20.),
            ] {
                controls = controls.child(
                    Button::new(format!("video-pan-{id}"))
                        .label(label)
                        .accessibility_label(hint)
                        .tooltip(hint)
                        .small()
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.pan_video(gpui_kit::point(px(x), px(y)), cx)
                        })),
                );
            }
            controls = controls.child(
                Button::new("video-overscan")
                    .label(if overscan {
                        "安全区：开"
                    } else {
                        "安全区：关"
                    })
                    .tooltip("显示电视过扫描安全区遮罩")
                    .small()
                    .when(overscan, |button| button.primary())
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":"video/show_overscan"}), cx);
                    })),
            );
        }
        div()
            .v_flex()
            .flex_shrink_0()
            .overflow_hidden()
            .when(self.snapshot.video, |v| v.child(controls))
            .child(
                div()
                    .id("video-frame-surface")
                    .h(px(view_height))
                    .flex_shrink_0()
                    .w_full()
                    .relative()
                    .overflow_hidden()
                    .bg(rgb(0x0e1013))
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                *viewport.borrow_mut() = bounds;
                                let base = video_base_bounds(bounds, aspect, display_size);
                                let target = zoomed_video_bounds(base, zoom, pan);
                                // Visual overlay owns its layout and resize notification while shown.
                                if !visual_active {
                                    *canvas_region.borrow_mut() = base;
                                    *image_region.borrow_mut() = target;
                                }
                                (
                                    target,
                                    image.and_then(|image| image.use_render_image(window, cx)),
                                )
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
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(rgb(0xb6bcc6))
                                .child("Aegisub GPUI · 实时媒体预览"),
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
                    .px_2()
                    .py_1()
                    .bg(rgb(0x0e1013))
                    .text_xs()
                    .text_color(rgb(0xb6bcc6))
                    .child(status)
                    .child("滚轮调整视频框 · Ctrl/⌘ 滚轮缩放画面 · Shift 滚轮或中键平移"),
            )
    }

    fn render_subtitle_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let appearance = &self.snapshot.appearance;
        let line_bg = appearance.color(
            "Colour/Subtitle Grid/Background/Background",
            Theme::global(cx).background,
        );
        let foreground = appearance.color(
            "Colour/Subtitle Grid/Standard",
            Theme::global(cx).foreground,
        );
        let selected_foreground = appearance.color("Colour/Subtitle Grid/Selection", foreground);
        let comment_bg = appearance.color("Colour/Subtitle Grid/Background/Comment", line_bg);
        let selected_comment_bg =
            appearance.color("Colour/Subtitle Grid/Background/Selected Comment", line_bg);
        let inframe_bg = appearance.color("Colour/Subtitle Grid/Background/Inframe", line_bg);
        let highlight_visible = appearance.highlight_visible && self.snapshot.video;
        let video_time = self.snapshot.video_time;
        let focus_grid = appearance.focus_grid;
        let grid_size = if appearance.grid_size > 0 {
            (appearance.grid_size as f32 * 4. / 3.).clamp(4., 56.)
        } else {
            12.
        };
        let grid_font = appearance.grid_font.clone();
        let row_height = (grid_size * 2.1 + 8.).max(34.);
        let stripe: gpui_kit::Hsla = if dark {
            rgb(0x1e2228).into()
        } else {
            rgb(0xf6f8fa).into()
        };
        let border = appearance.color("Colour/Subtitle Grid/Lines", Theme::global(cx).border);
        let header_bg = appearance.color("Colour/Subtitle Grid/Header", line_bg);
        let muted = if dark { rgb(0x929aa7) } else { rgb(0x727c88) };
        let selected_bg = appearance.color(
            "Colour/Subtitle Grid/Background/Selection",
            Theme::global(cx).selection,
        );
        let accent = if dark { rgb(0x74adff) } else { rgb(0x2868bf) };
        let lines = self.snapshot.lines.clone();
        let selected = self.snapshot.selected.clone();
        let weak = cx.weak_entity();
        let filtered = self.search.read(cx).value().to_string();
        let lower_filter = filtered.to_lowercase();
        let visible: Vec<Line> = lines
            .into_iter()
            .filter(|line| {
                lower_filter.is_empty()
                    || line.text.to_lowercase().contains(&lower_filter)
                    || line.actor.to_lowercase().contains(&lower_filter)
                    || line.style.to_lowercase().contains(&lower_filter)
            })
            .collect();
        let editor = Textarea::new(&self.editor)
            .aria_label("所选字幕文本")
            .bg(appearance.color("Colour/Subtitle/Background", Theme::global(cx).background))
            .text_color(appearance.color(
                "Colour/Subtitle/Syntax/Normal",
                Theme::global(cx).foreground,
            ))
            .when(appearance.edit_size > 0, |editor| {
                editor.text_size(px((appearance.edit_size as f32 * 4. / 3.).clamp(4., 56.)))
            })
            .when(!appearance.edit_font.is_empty(), |editor| {
                editor.font_family(appearance.edit_font.clone())
            });
        let mut format_toolbar = div().h_flex().flex_wrap().gap_1().items_center();
        for (id, label, name) in [
            ("format-bold", "粗体", "edit/style/bold"),
            ("format-italic", "斜体", "edit/style/italic"),
            ("format-underline", "下划线", "edit/style/underline"),
            ("format-strikeout", "删除线", "edit/style/strikeout"),
            ("format-font", "字体…", "edit/font"),
            ("format-primary", "颜色…", "edit/color/primary"),
            ("split-preserve", "光标处分行", "edit/line/split/preserve"),
            ("copy-lines", "复制行", "edit/line/copy"),
            ("cut-lines", "剪切行", "edit/line/cut"),
            ("paste-lines", "粘贴行", "edit/line/paste"),
        ] {
            let enabled = self
                .catalog
                .command(name)
                .is_some_and(|command| command.can_invoke());
            format_toolbar = format_toolbar.child(
                Button::new(id)
                    .label(label)
                    .small()
                    .disabled(!enabled)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.invoke(json!({"op":"command", "name":name}), cx)
                    })),
            );
        }
        let row_count = visible.len();
        div()
            .v_flex()
            .flex_1()
            .min_h(px(260.))
            .overflow_hidden()
            .bg(line_bg)
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .flex_shrink_0()
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(header_bg)
                    .border_b_1()
                    .border_color(border)
                    .child(
                        div()
                            .w(px(54.))
                            .text_xs()
                            .text_color(muted)
                            .child("行 / 层"),
                    )
                    .child(
                        div()
                            .w(px(170.))
                            .text_xs()
                            .text_color(muted)
                            .child("开始 → 结束"),
                    )
                    .child(
                        div()
                            .w(px(120.))
                            .text_xs()
                            .text_color(muted)
                            .child("样式 / 角色"),
                    )
                    .child(div().flex_1().text_xs().text_color(muted).child("字幕文本"))
                    .child(Input::new(&self.search).id("subtitle-search").w(px(190.))),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(90.))
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
                                        } else if highlight_visible && line.start <= video_time && video_time < line.end {
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
                                            .when(!grid_font.is_empty(), |row| row.font_family(grid_font.clone()))
                                            .w_full()
                                            .h_flex()
                                            .gap_3()
                                            .items_center()
                                            .px_3()
                                            .bg(row_bg)
                                            .border_b_1()
                                            .border_color(border)
                                            .cursor_pointer()
                                            .on_mouse_down(gpui_kit::MouseButton::Left, {
                                                let weak = weak.clone();
                                                let id = line.id;
                                                move |event: &gpui_kit::MouseDownEvent, window, cx| {
                                                    let _ = weak.update(cx, |view, cx| {
                                                        if focus_grid { view.focus_handle.focus(window, cx); }
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
                                            .child(
                                                div()
                                                    .w(px(54.))
                                                    .flex_shrink_0()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(format!(
                                                        "{:03} / {}",
                                                        line.row + 1,
                                                        line.layer
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .w(px(170.))
                                                    .flex_shrink_0()
                                                    .text_xs()
                                                    .text_color(muted)
                                                    .child(format!(
                                                        "{} → {}",
                                                        format_time(line.start),
                                                        format_time(line.end)
                                                    )),
                                            )
                                            .child(
                                                div()
                                                    .w(px(120.))
                                                    .flex_shrink_0()
                                                    .overflow_hidden()
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(accent)
                                                            .child(line.style),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_xs()
                                                            .text_color(muted)
                                                            .child(line.actor),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .overflow_hidden()
                                                    .text_size(px(grid_size))
                                                    .text_color(if is_active { selected_foreground } else { foreground })
                                                    .child(line.display_text),
                                            )
                                    })
                                    .collect()
                            }),
                        )
                        .track_scroll(&self.list_scroll_handle)
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .v_flex()
                    .flex_shrink_0()
                    .px_3()
                    .py_2()
                    .gap_1()
                    .bg(stripe)
                    .border_t_1()
                    .border_color(border)
                    .child(
                        div()
                            .h_flex()
                            .justify_between()
                            .items_center()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!("字幕文本编辑 · 应用于所选 {} 行", self.snapshot.selected.len())),
                            )
                            .child(
                                Button::new("apply-edit")
                                    .label("立即提交")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.selected_line;
                                        let text = this.editor.read(cx).value().to_string();
                                        this.invoke(json!({"op":"edit", "id":id, "ids":this.snapshot.selected, "text":text}), cx);
                                    })),
                            ),
                    )
                    .child(format_toolbar)
                    .child(editor.h(px(88.)).w_full()),
            )
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
        let canvas = if dark { rgb(0x17191d) } else { rgb(0xf4f5f7) };
        let border = if dark { rgb(0x30343b) } else { rgb(0xdde1e7) };
        let muted = if dark { rgb(0x8e95a0) } else { rgb(0x747d89) };
        let mut root = div()
            .id("aegisub-gpui-workspace")
            .track_focus(&self.focus_handle)
            .capture_key_down(cx.listener(Self::handle_key))
            .on_mouse_move(cx.listener(|view, event: &gpui_kit::MouseMoveEvent, _, cx| {
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
            .size_full()
            .overflow_hidden()
            .v_flex()
            .bg(canvas)
            .text_color(Theme::global(cx).foreground)
            .child(self.render_legacy_menus(cx))
            .when(self.snapshot.show_toolbar, |root| root.child(self.render_toolbar(cx)))
            .child(self.render_menu(cx));
        if self.active_panel == Panel::Subtitles {
            root = root
                .child(
                    div()
                        .h_flex()
                        .flex_1()
                        .min_h_0()
                        .overflow_hidden()
                        .gap_2()
                        .p_2()
                        .child(
                            div()
                                .v_flex()
                                .flex_1()
                                .min_w_0()
                                .min_h_0()
                                .gap_2()
                                .child(self.render_player(window, cx))
                                .child(self.render_subtitle_panel(cx)),
                        )
                        .child(self.render_properties(cx)),
                )
                .child(self.timeline.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Visual {
            root = root.child(div().v_flex().flex_1().min_h_0().p_3().gap_2()
                .child(self.visual.toolbar(cx))
                .child(self.render_player(window, cx))
                .child(div().text_sm().child("双击十字定位；拖动控制点调整，Shift 约束，Ctrl/⌘ 多选。操作写入当前字幕并可撤销。")))
                .child(self.timeline.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Commands {
            root = root.child(self.render_commands(cx));
        } else if self.active_panel == Panel::Audio {
            root = root.child(self.audio.render(
                &self.snapshot.audio,
                self.snapshot.video,
                &self.snapshot.appearance,
                cx,
            ));
        } else if self.active_panel == Panel::Styling {
            root = root.child(
                self.styling
                    .render(&self.snapshot, &self.document.details, cx),
            );
        } else if self.active_panel == Panel::Translation {
            root = root.child(self.translation.render(
                self.snapshot.video,
                self.snapshot.audio.loaded,
                cx,
            ));
        } else if self.active_panel == Panel::Automation {
            root = root.child(self.automation.render(&self.catalog, cx));
        } else if self.active_panel == Panel::Spelling {
            root = root.child(self.spelling.render(cx));
        } else if self.active_panel == Panel::Files {
            root = root.child(self.files.render(cx));
        } else if self.active_panel == Panel::Preferences {
            root = root.child(self.preferences.render(cx));
        } else if self.active_panel == Panel::Hotkeys {
            root = root.child(self.hotkeys.render(&self.catalog, cx));
        } else if self.active_panel == Panel::Attachments {
            root = root.child(self.attachments.render(cx));
        } else if self.active_panel == Panel::VideoSettings {
            root = root.child(self.video_settings.render(cx));
        } else if self.active_panel == Panel::Paste {
            root = root.child(self.paste.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Search {
            root = root.child(self.find_panel.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Processor {
            root = root.child(self.processor.render(cx));
        } else if self.active_panel == Panel::Resample {
            root = root.child(self.resample.render(cx));
        } else if self.active_panel == Panel::Timing {
            root = root.child(self.timing.render(&self.snapshot, cx));
        } else if self.active_panel == Panel::Styles {
            root = root.child(self.document.render_styles(cx));
        } else {
            root = root.child(self.document.render_project(cx));
        }
        root.child(
            div()
                .h_flex()
                .h(px(26.))
                .flex_shrink_0()
                .px_3()
                .items_center()
                .justify_between()
                .bg(if dark { rgb(0x202329) } else { rgb(0xffffff) })
                .border_t_1()
                .border_color(border)
                .child(div().text_xs().text_color(muted).child(self.status.clone()))
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
                            self.snapshot.file.clone()
                        }),
                ),
        )
    }
}

fn run(endpoint: String, token: String) -> Result<()> {
    application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bridge = Bridge { endpoint, token };
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::centered(size(px(1420.), px(920.)), cx)),
                titlebar: Some(gpui_kit::TitlebarOptions {
                    title: Some("Aegisub · GPUI prototype".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                cx.activate(true);
                window.activate_window();
                let workspace = cx.new(|cx| Workspace::new(bridge.clone(), window, cx));
                workspace.update(cx, |view, cx| {
                    view.refresh_catalog(cx);
                    view.refresh_document(cx);
                });
                let weak = workspace.downgrade();
                let poll_bridge = bridge.clone();
                cx.spawn(async move |cx| {
                    let mut consecutive_failures = 0;
                    loop {
                        if weak.upgrade().is_none() {
                            break;
                        }
                        match poll_bridge.snapshot().await {
                            Ok(snapshot) => {
                                consecutive_failures = 0;
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
                            .timer(Duration::from_millis(350))
                            .await;
                    }
                })
                .detach();
                cx.new(|cx| Root::new(workspace, window, cx))
            },
        );
        if let Err(error) = window {
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
