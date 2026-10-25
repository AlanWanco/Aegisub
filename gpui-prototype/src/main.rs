use std::{net::SocketAddr, ops::Range, sync::Arc, time::Duration};

use anyhow::{Context as _, Result, anyhow};
use async_net::TcpStream;
use futures_lite::io::{AsyncReadExt, AsyncWriteExt};
use gpui_kit::{
    App, AppContext, Context, Entity, Image, IntoElement, Render, Subscription, Window,
    WindowBounds, WindowOptions, div, img, prelude::*, px, rgb, size, uniform_list,
};
use gpui_kit::{
    base::Disableable,
    component::{
        Root, Sizable, StyledExt, Theme, ThemeMode,
        button::{Button, ButtonVariants},
        input::{Input, InputEvent, InputState, Textarea, TextareaState},
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
    layer: i64,
    comment: bool,
    style: String,
    actor: String,
    text: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Snapshot {
    #[serde(default)]
    file: String,
    #[serde(default)]
    modified: bool,
    #[serde(default)]
    video: bool,
    #[serde(default)]
    playing: bool,
    #[serde(default)]
    frame: i64,
    #[serde(default)]
    frame_count: i64,
    #[serde(default)]
    video_epoch: u64,
    fps: Option<f64>,
    #[serde(default = "no_active_line")]
    active: i64,
    #[serde(default)]
    lines: Vec<Line>,
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
}

struct Workspace {
    bridge: Bridge,
    snapshot: Snapshot,
    frame_image: Option<Arc<Image>>,
    frame_image_frame: i64,
    frame_request_inflight: bool,
    frame_request_generation: u64,
    video_epoch: u64,
    list_scroll_handle: gpui_kit::UniformListScrollHandle,
    editor: Entity<TextareaState>,
    search: Entity<InputState>,
    editor_line_id: i64,
    editor_dirty_line: i64,
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
                        .call(json!({"op":"edit", "id":line_id, "text":text}))
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

        Self {
            bridge,
            snapshot: Snapshot {
                file: "正在连接 Aegisub…".into(),
                modified: false,
                video: false,
                playing: false,
                frame: 0,
                frame_count: 0,
                video_epoch: 0,
                fps: None,
                active: -1,
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
            editor_line_id: -1,
            editor_dirty_line: -1,
            active_panel: Panel::Subtitles,
            selected_line: -1,
            status: "已连接当前 Aegisub 文档".into(),
            _editor_events: editor_events,
            _search_events: search_events,
        }
    }

    fn apply_snapshot(&mut self, snapshot: Snapshot, cx: &mut Context<Self>) {
        let active_exists = snapshot.lines.iter().any(|line| line.id == snapshot.active);
        let selected = if active_exists { snapshot.active } else { -1 };
        let active_changed = selected != self.selected_line;
        self.selected_line = selected;
        let media_changed =
            snapshot.video_epoch != self.video_epoch || snapshot.video != self.snapshot.video;
        let frame_changed = snapshot.frame != self.snapshot.frame;
        if media_changed {
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
        cx.notify();
    }

    fn select_line(&mut self, id: i64, cx: &mut Context<Self>) {
        self.selected_line = id;
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(
            async move |_, cx| match bridge.call(json!({"op":"select", "id":id})).await {
                Ok(_) => match bridge.snapshot().await {
                    Ok(snapshot) => {
                        if let Some(view) = weak.upgrade() {
                            let _ = view.update(cx, |view, cx| {
                                view.status = "已同步字幕选择".into();
                                view.apply_snapshot(snapshot, cx);
                            });
                        }
                    }
                    Err(error) => set_status(&weak, cx, format!("读取字幕失败：{error}")),
                },
                Err(error) => set_status(&weak, cx, format!("选择字幕失败：{error}")),
            },
        )
        .detach();
        cx.notify();
    }

    fn invoke(&mut self, payload: Value, cx: &mut Context<Self>) {
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| match bridge.call(payload).await {
            Ok(_) => match bridge.snapshot().await {
                Ok(snapshot) => {
                    if let Some(view) = weak.upgrade() {
                        let _ = view.update(cx, |view, cx| {
                            view.status = "已同步当前项目".into();
                            view.apply_snapshot(snapshot, cx);
                        });
                    }
                }
                Err(error) => set_status(&weak, cx, format!("同步失败：{error}")),
            },
            Err(error) => set_status(&weak, cx, format!("操作失败：{error}")),
        })
        .detach();
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

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let background = if dark { rgb(0x17191d) } else { rgb(0xf3f4f6) };
        let muted = if dark { rgb(0x858b96) } else { rgb(0x69717d) };
        let bridge = self.bridge.clone();
        div().h_flex().gap_1().px_3().py_2().bg(background).border_b_1().border_color(if dark { rgb(0x30343b) } else { rgb(0xdde1e7) })
            .child(Button::new("open-subtitles").label("打开字幕").ghost().small().on_click({ let bridge = bridge.clone(); let weak = cx.weak_entity(); move |_, _, cx| {
                let bridge = bridge.clone(); let weak = weak.clone();
                cx.spawn(async move |cx| {
                    let result = rfd::AsyncFileDialog::new().add_filter("字幕文件", &["ass", "ssa", "srt", "vtt"]).pick_file().await;
                    if let Some(file) = result { invoke_weak(&bridge, &weak, cx, json!({"op":"open", "kind":"subtitle", "path":file.path().to_string_lossy()})).await; }
                }).detach();
            }}))
            .child(Button::new("save-subtitles").label("保存").ghost().small().on_click(cx.listener(|this, _, _, cx| this.invoke(json!({"op":"command", "name":"subtitle/save"}), cx))))
            .child(Button::new("undo").label("撤销").ghost().small().on_click(cx.listener(|this, _, _, cx| this.invoke(json!({"op":"command", "name":"edit/undo"}), cx))))
            .child(Button::new("redo").label("重做").ghost().small().on_click(cx.listener(|this, _, _, cx| this.invoke(json!({"op":"command", "name":"edit/redo"}), cx))))
            .child(div().w(px(1.)).h(px(22.)).mx_2().bg(if dark { rgb(0x343942) } else { rgb(0xd3d8df) }))
            .child(Button::new("open-video").label("打开视频").ghost().small().on_click({ let bridge = bridge.clone(); let weak = cx.weak_entity(); move |_, _, cx| {
                let bridge = bridge.clone(); let weak = weak.clone();
                cx.spawn(async move |cx| {
                    let result = rfd::AsyncFileDialog::new().add_filter("视频文件", &["mp4", "mkv", "mov", "avi", "webm"]).pick_file().await;
                    if let Some(file) = result { invoke_weak(&bridge, &weak, cx, json!({"op":"open", "kind":"video", "path":file.path().to_string_lossy()})).await; }
                }).detach();
            }}))
            .child(Button::new("play-video").label(if self.snapshot.playing { "暂停" } else { "播放" }).primary().small().on_click(cx.listener(|this, _, _, cx| {
                if this.snapshot.playing { this.invoke(json!({"op":"command", "name":"video/stop"}), cx); }
                else { this.invoke(json!({"op":"command", "name":"video/play"}), cx); }
            })))
            .child(Button::new("refresh-frame").label("刷新帧").ghost().small().disabled(!self.snapshot.video).on_click(cx.listener(|this, _, _, cx| {
                this.frame_image = None;
                this.frame_image_frame = -1;
                this.refresh_frame(cx);
            })))
            .child(div().flex_1())
            .child(div().text_sm().text_color(muted).child(if self.snapshot.modified { "未保存更改" } else { "已保存" }))
    }

    fn render_menu(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let background = if dark { rgb(0x202329) } else { rgb(0xffffff) };
        let active = if dark { rgb(0x303640) } else { rgb(0xe9eef6) };
        let panels = [
            (Panel::Subtitles, "字幕"),
            (Panel::Styles, "样式"),
            (Panel::Project, "项目"),
        ];
        let mut row = div()
            .h_flex()
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
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.active_panel = panel;
                        cx.notify();
                    }))
                    .child(label),
            );
        }
        row
    }

    fn render_player(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let panel = if dark { rgb(0x0e1013) } else { rgb(0x20242b) };
        let text = if dark { rgb(0xb6bcc6) } else { rgb(0xe1e5ec) };
        let image = self.frame_image.clone();
        let status = if self.snapshot.video {
            format!(
                "帧 {:05}   {} fps",
                self.snapshot.frame,
                self.snapshot
                    .fps
                    .map(|fps| format!("{fps:.3}"))
                    .unwrap_or_else(|| "VFR".into())
            )
        } else {
            "未加载视频 · 通过左上角打开视频以显示当前 Aegisub 解码帧".into()
        };
        div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .max_h(px(460.))
            .bg(panel)
            .items_center()
            .justify_center()
            .relative()
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(image, |this, image| {
                        this.child(
                            img(image)
                                .size_full()
                                .object_fit(gpui_kit::ObjectFit::Contain),
                        )
                    })
                    .when(self.frame_image.is_none(), |this| {
                        this.child(div().text_color(text).child("Aegisub GPUI · 实时媒体预览"))
                    }),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .left_0()
                    .right_0()
                    .h_flex()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .bg(gpui_kit::rgba(0x090b0dcc))
                    .text_sm()
                    .text_color(rgb(0xe1e5ec))
                    .child(status)
                    .child("使用 Aegisub AsyncVideoProvider · 字幕已由 libass 合成"),
            )
    }

    fn render_timeline(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let border = if dark { rgb(0x30343b) } else { rgb(0xdde1e7) };
        let background = if dark { rgb(0x1c1f24) } else { rgb(0xf7f8fa) };
        let accent = if dark { rgb(0x70a9ff) } else { rgb(0x2768bf) };
        let width = self.snapshot.frame_count.max(1) as f32;
        let fps = self.snapshot.fps.unwrap_or(24.) as f32;
        let mut content = div()
            .h_flex()
            .h(px(42.))
            .gap_1()
            .items_center()
            .px_3()
            .bg(background)
            .border_t_1()
            .border_color(border);
        if self.snapshot.lines.is_empty() {
            return content.child(
                div()
                    .text_sm()
                    .text_color(if dark { rgb(0x8e95a0) } else { rgb(0x78818d) })
                    .child("音频波形与时间轴交互尚未接入；字幕时间码仍使用当前 Aegisub 项目数据。"),
            );
        }
        for line in self.snapshot.lines.iter().take(24) {
            let left = (line.start as f32 * fps / 1000. / width).clamp(0., 1.);
            let span =
                (((line.end - line.start).max(1) as f32 * fps / 1000.) / width).clamp(0.025, 0.65);
            let id = line.id;
            let start = line.start;
            let row = line.row;
            content = content.child(
                div()
                    .id(format!("timeline-{}", id))
                    .h(px(18.))
                    .w(px(760. * span))
                    .flex_shrink_0()
                    .rounded_sm()
                    .bg(if id == self.selected_line {
                        accent
                    } else {
                        accent.alpha(0.48)
                    })
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let seek_frame = (start as f32 * fps / 1000.).round() as i64;
                        this.select_line(id, cx);
                        this.invoke(json!({"op":"seek", "frame":seek_frame}), cx);
                    }))
                    .child(
                        div()
                            .relative()
                            .left(px(760. * left))
                            .text_xs()
                            .text_color(rgb(0xffffff))
                            .child(format!("{}", row + 1)),
                    ),
            );
        }
        content
    }

    fn render_subtitle_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let dark = Theme::global(cx).is_dark();
        let line_bg = if dark { rgb(0x22262c) } else { rgb(0xffffff) };
        let stripe = if dark { rgb(0x1e2228) } else { rgb(0xf6f8fa) };
        let border = if dark { rgb(0x333841) } else { rgb(0xe2e5e9) };
        let muted = if dark { rgb(0x929aa7) } else { rgb(0x727c88) };
        let selected_bg = if dark { rgb(0x24334a) } else { rgb(0xe5efff) };
        let accent = if dark { rgb(0x74adff) } else { rgb(0x2868bf) };
        let lines = self.snapshot.lines.clone();
        let active = self.selected_line;
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
        let bridge = self.bridge.clone();
        let editor = Textarea::new(&self.editor);
        let row_count = visible.len();
        div()
            .v_flex()
            .flex_1()
            .min_h_0()
            .bg(line_bg)
            .child(
                div()
                    .h_flex()
                    .h(px(32.))
                    .px_3()
                    .gap_3()
                    .items_center()
                    .bg(stripe)
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
                    .h(px(260.))
                    .flex_shrink_0()
                    .overflow_y_scrollbar()
                    .child(
                        uniform_list(
                            "subtitle-rows",
                            row_count,
                            cx.processor(move |_, range: Range<usize>, _, _| {
                                range
                                    .map(|index| {
                                        let line = visible[index].clone();
                                        let is_active = line.id == active;
                                        let row_bg = if is_active {
                                            selected_bg
                                        } else if line.row % 2 == 1 {
                                            stripe
                                        } else {
                                            line_bg
                                        };
                                        let bridge = bridge.clone();
                                        let weak = weak.clone();
                                        div()
                                            .id(format!("subtitle-row-{}", line.id))
                                            .h(px(34.))
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
                                                let bridge = bridge.clone();
                                                let weak = weak.clone();
                                                move |_, _, cx| {
                                                    let bridge = bridge.clone();
                                                    let weak = weak.clone();
                                                    let id = line.id;
                                                    cx.spawn(async move |cx| {
                                                        match bridge
                                                            .call(json!({"op":"select", "id":id}))
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                if let Some(view) = weak.upgrade() {
                                                                    if let Ok(snapshot) =
                                                                        bridge.snapshot().await
                                                                    {
                                                                        let _ = view.update(
                                                                            cx,
                                                                            |view, cx| {
                                                                                view.apply_snapshot(
                                                                                    snapshot, cx,
                                                                                )
                                                                            },
                                                                        );
                                                                    }
                                                                }
                                                            }
                                                            Err(error) => set_status(
                                                                &weak,
                                                                cx,
                                                                format!("选择字幕失败：{error}"),
                                                            ),
                                                        }
                                                    })
                                                    .detach();
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
                                                    .text_sm()
                                                    .text_color(if line.comment {
                                                        muted
                                                    } else {
                                                        rgb(0xe5e8ee)
                                                    })
                                                    .child(line.text),
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
                                    .child("字幕文本编辑 · 输入后自动提交到 Aegisub 撤销栈"),
                            )
                            .child(
                                Button::new("apply-edit")
                                    .label("立即提交")
                                    .primary()
                                    .small()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let id = this.selected_line;
                                        let text = this.editor.read(cx).value().to_string();
                                        this.invoke(json!({"op":"edit", "id":id, "text":text}), cx);
                                    })),
                            ),
                    )
                    .child(editor.h(px(88.)).w_full()),
            )
    }
}

fn format_time(ms: i64) -> String {
    let total = ms.max(0);
    format!(
        "{:02}:{:02}.{:02}",
        total / 60000,
        (total / 1000) % 60,
        (total % 1000) / 10
    )
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
) {
    match bridge.call(payload).await {
        Ok(_) => match bridge.snapshot().await {
            Ok(snapshot) => {
                if let Some(view) = weak.upgrade() {
                    let _ = view.update(cx, |view, cx| {
                        view.status = "已同步当前项目".into();
                        view.apply_snapshot(snapshot, cx);
                    });
                }
            }
            Err(error) => set_status(weak, cx, format!("同步失败：{error}")),
        },
        Err(error) => set_status(weak, cx, format!("操作失败：{error}")),
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
        let dark = Theme::global(cx).is_dark();
        let canvas = if dark { rgb(0x17191d) } else { rgb(0xf4f5f7) };
        let border = if dark { rgb(0x30343b) } else { rgb(0xdde1e7) };
        let muted = if dark { rgb(0x8e95a0) } else { rgb(0x747d89) };
        let bridge = self.bridge.clone();
        let weak = cx.weak_entity();
        let mut root = div()
            .id("aegisub-gpui-workspace")
            .size_full()
            .v_flex()
            .bg(canvas)
            .text_color(Theme::global(cx).foreground)
            .child(self.render_toolbar(cx))
            .child(self.render_menu(cx));
        if self.active_panel == Panel::Subtitles {
            root = root.child(div().h_flex().flex_1().min_h_0().gap_2().p_2()
                .child(div().v_flex().flex_1().min_w_0().min_h_0().gap_2()
                    .child(self.render_player(cx))
                    .child(self.render_subtitle_panel(cx))
                )
                .child(div().v_flex().w(px(286.)).flex_shrink_0().bg(if dark { rgb(0x202329) } else { rgb(0xffffff) }).border_l_1().border_color(border).p_3().gap_3()
                    .child(div().text_base().font_semibold().child("当前行"))
                    .child(div().text_xs().text_color(muted).child(format!("{} · {}", self.snapshot.file, if self.snapshot.modified { "已修改" } else { "已保存" })))
                    .child(div().h(px(1.)).bg(border))
                    .child(div().text_xs().text_color(muted).child("时间码"))
                    .child(div().text_sm().child(self.snapshot.lines.iter().find(|line| line.id == self.selected_line).map(|line| format!("{} → {}", format_time(line.start), format_time(line.end))).unwrap_or_else(|| "未选择字幕行".into())))
                    .child(div().text_xs().text_color(muted).child("样式"))
                    .child(div().text_sm().child(self.snapshot.lines.iter().find(|line| line.id == self.selected_line).map(|line| line.style.clone()).unwrap_or_else(|| "—".into())))
                    .child(div().text_xs().text_color(muted).child("角色"))
                    .child(div().text_sm().child(self.snapshot.lines.iter().find(|line| line.id == self.selected_line).map(|line| line.actor.clone()).unwrap_or_else(|| "—".into())))
                    .child(div().h(px(1.)).bg(border))
                    .child(div().text_xs().text_color(muted).child("音频波形与音频控制尚未接入 GPUI。当前音频仍由 Aegisub 原窗口管理。"))
                    .child(Button::new("save-as").label("另存为…").secondary().small().on_click({ let bridge = bridge.clone(); move |_, _, cx| {
                        let bridge = bridge.clone(); let weak = weak.clone();
                        cx.spawn(async move |cx| {
                            let result = rfd::AsyncFileDialog::new().add_filter("ASS 字幕", &["ass", "ssa"]).save_file().await;
                            if let Some(file) = result { invoke_weak(&bridge, &weak, cx, json!({"op":"save_as", "path":file.path().to_string_lossy()})).await; }
                        }).detach();
                    }}))
                )
            ).child(self.render_timeline(cx));
        } else {
            root = root.child(div().flex_1().min_h_0().p_6().text_color(muted).child(
                match self.active_panel {
                    Panel::Styles => "样式管理面板尚未接入；可继续使用 Aegisub 的 Styles Manager。",
                    Panel::Project => {
                        "项目属性面板尚未接入；项目与媒体状态由 Aegisub C++ 核心持有。"
                    }
                    Panel::Subtitles => "",
                },
            ));
        }
        root.child(
            div()
                .h_flex()
                .h(px(26.))
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
                        .child("Demo 资源仅作为示例 · 当前编辑数据来自 Aegisub"),
                ),
        )
    }
}

fn run(endpoint: String, token: String) -> Result<()> {
    application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
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
                let workspace = cx.new(|cx| Workspace::new(bridge.clone(), window, cx));
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
                                        view.status = "已同步当前 Aegisub 项目".into();
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
