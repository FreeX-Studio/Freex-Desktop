/*
 * Copyright (c) 2026 MirraNET, Niklas Linz. All rights reserved.
 *
 * This file is part of the MirraNET project and is licensed under the
 * GNU Lesser General Public License v3.0 (LGPLv3).
 *
 * You may use, distribute and modify this code under the terms
 * of the LGPLv3 license. You should have received a copy of the
 * license along with this file. If not, see <https://www.gnu.org/licenses/lgpl-3.0.html>
 * or contact: niklas.linz@mirranet.de
 */

//! TV mode: the PC as a FreeX box on the TV.
//!
//! Instead of FreeX Web, the TV mode shows the interface of the FreeX TV apps (FreeX-TV-Apps,
//! bundled as `ui/tv/`), made for a remote control and a screen across the room, with mpv as its
//! player. It is the same arrangement as on FreeXHT, the FreeX system image:
//!
//! ```text
//!   window "tv"        the TV interface, full screen, transparent while a video plays, on top
//!   window "tv-video"  full screen and black; mpv draws into it (--wid), idle between videos
//! ```
//!
//! Per system:
//!
//! - Windows, Linux on X11: as above; the TV window is owned by the video window, so the window
//!   manager keeps it on top of it.
//! - Linux on Wayland: Wayland neither embeds nor stacks foreign windows, so the TV mode runs the
//!   app through XWayland (`GDK_BACKEND=x11`, started again for it if needed).
//! - macOS: mpv embeds only through libmpv there, so mpv opens its own borderless full-screen
//!   window (without Dock icon and without taking the focus) and the TV window floats above it.
//!
//! The interface drives mpv through `window.FreeXNative` (`tv_bridge.js`), the same bridge as
//! on FreeXHT and in the Android app: open → `ready` (paused at the start position) → play; a
//! `time` event every half second; `buffering`, `state`, `ended` and `error` as they come. mpv
//! runs as long as the TV mode does and is started again if it dies.

use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewUrl, WebviewWindowBuilder};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;

use crate::server::AppState;

/// Label of the window with the TV interface.
pub const UI: &str = "tv";
/// Label of the window mpv draws into.
pub const VIDEO: &str = "tv-video";
/// Name of the event with mpv's state for the TV interface.
pub const EVENT: &str = "freex-tv";

/// The bridge for the TV interface; `__FREEX_TV_INFO__` is replaced by [`bridge_info`].
const BRIDGE_JS: &str = include_str!("tv_bridge.js");

/// True on the systems with a TV mode: Windows, Linux and macOS.
pub fn available() -> bool {
    cfg!(any(windows, target_os = "linux", target_os = "macos"))
}

/// True if the TV mode can be offered here: a system with a TV mode and a build that bundles
/// the TV interface (`ui/tv`, see packaging/tv-ui.sh).
pub fn ready<R: Runtime>(app: &AppHandle<R>) -> bool {
    available() && app.asset_resolver().get("tv/index.html".into()).is_some()
}

/// Linux: true if this process draws through Wayland, where the TV mode cannot stack mpv under
/// its window; it needs the X11 backend (XWayland) instead.
#[cfg(target_os = "linux")]
pub fn needs_x11() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("DISPLAY").is_some()
        && std::env::var("GDK_BACKEND").map_or(true, |b| b != "x11")
}

/// Linux on Wayland: starts the app again through XWayland, directly in the TV mode. The new
/// process waits a moment, so this one has ended (one instance only) before it starts.
#[cfg(target_os = "linux")]
fn restart_in_x11<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    std::process::Command::new("sh")
        .args(["-c", "sleep 1; exec \"$0\" --tv"])
        .arg(&exe)
        .env("GDK_BACKEND", "x11")
        .spawn()
        .map_err(|e| e.to_string())?;
    app.exit(0);
    Ok(())
}

/// The TV mode while it is on: the channel to its mpv.
#[derive(Default)]
pub struct Tv {
    player: Mutex<Option<mpsc::UnboundedSender<Command>>>,
}

impl Tv {
    pub fn active(&self) -> bool {
        self.player.lock().unwrap().is_some()
    }

    fn send(&self, command: Command) {
        if let Some(tx) = self.player.lock().unwrap().as_ref() {
            let _ = tx.send(command);
        }
    }

    /// Ends mpv of the TV mode (the windows are closed by [`leave`]).
    pub fn stop(&self) {
        if let Some(tx) = self.player.lock().unwrap().take() {
            let _ = tx.send(Command::Quit);
        }
    }
}

/// What the interface asks the player to do.
#[derive(Debug)]
enum Command {
    Open { url: String, start_ms: u64 },
    Play,
    Pause,
    Seek(u64),
    Stop,
    Quit,
}

// ---------------------------------------------------------------------------- windows

/// Turns the TV mode on: the video window with mpv, the TV interface on top. The normal window
/// is closed (FreeX Web does not keep running behind the TV mode); [`leave`] opens it again. If
/// the TV mode is on already, its window comes to the front.
pub fn enter(app: &AppHandle) -> Result<(), String> {
    if !available() {
        return Err("the TV mode is not available on this system".into());
    }
    let state = app.state::<AppState>();
    if focus(app) {
        return Ok(());
    }
    let mpv = crate::mpv::find().ok_or("mpv not found")?;
    if !ready(app) {
        return Err("this build has no TV interface (ui/tv)".into());
    }
    #[cfg(target_os = "linux")]
    if needs_x11() {
        return restart_in_x11(app);
    }
    state.mpv.stop();
    // The screen the app is on (or the main screen): both windows cover it exactly, also where
    // the window manager does not do full screen itself.
    let screen = app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let cover = |w: &tauri::WebviewWindow| {
        if let Some(m) = &screen {
            let _ = w.set_position(*m.position());
            let _ = w.set_size(*m.size());
        }
        // macOS: the native full screen would move the window to a Space of its own, away
        // from mpv's window; the simple full screen keeps it on the desktop, above mpv.
        if cfg!(target_os = "macos") {
            let _ = w.set_simple_fullscreen(true);
        } else {
            let _ = w.set_fullscreen(true);
        }
    };

    // macOS: mpv in its own window (see the top of the file); elsewhere the video window. Both
    // windows are built hidden and only shown once everything is in place, so a failure on the
    // way never leaves a black full-screen window behind and keeps the normal window.
    let video = if cfg!(target_os = "macos") {
        None
    } else {
        let builder =
            WebviewWindowBuilder::new(app, VIDEO, WebviewUrl::App("tv-video.html".into()))
                .title("FreeX TV")
                .decorations(false)
                .visible(false)
                .skip_taskbar(true)
                .focused(false)
                .background_color(tauri::window::Color(0, 0, 0, 0xff));
        Some(
            crate::shared_webview(builder)
                .build()
                .map_err(|e| e.to_string())?,
        )
    };
    let abort = |video: Option<tauri::WebviewWindow>, e: String| -> Result<(), String> {
        if let Some(video) = video {
            let _ = video.destroy();
        }
        if let Some(ui) = app.get_webview_window(UI) {
            let _ = ui.destroy();
        }
        Err(e)
    };

    let bridge = BRIDGE_JS.replace("__FREEX_TV_INFO__", &bridge_info(app).to_string());
    let builder = WebviewWindowBuilder::new(app, UI, WebviewUrl::App("tv/index.html".into()))
        .title("FreeX")
        .decorations(false)
        .visible(false)
        .shadow(false)
        .transparent(true)
        .background_color(tauri::window::Color(0, 0, 0, 0))
        .initialization_script(crate::NATIVE_JS)
        .initialization_script(&bridge)
        .zoom_hotkeys_enabled(false);
    let builder = crate::shared_webview(builder);
    let builder = match &video {
        // Owned by the video window: always above it, minimized and restored together.
        Some(video) => match builder.parent(video) {
            Ok(b) => b,
            Err(e) => return abort(video.clone().into(), e.to_string()),
        },
        // mpv's own window (macOS): the TV window floats above it.
        None => builder.always_on_top(true),
    };
    let ui = match builder.build() {
        Ok(ui) => ui,
        Err(e) => return abort(video, e.to_string()),
    };

    // The video window on screen: only now X11 has a window for mpv to draw into.
    if let Some(video) = &video {
        let _ = video.show();
        cover(video);
    }
    let wid = video.as_ref().and_then(crate::mpv::window_id);
    if video.is_some() && wid.is_none() {
        return abort(video, "mpv cannot draw into a window on this system".into());
    }

    let (tx, rx) = mpsc::unbounded_channel();
    *state.tv.player.lock().unwrap() = Some(tx);
    let player_app = app.clone();
    tauri::async_runtime::spawn(async move { supervise(player_app, mpv, wid, rx).await });

    // Everything is in place: the normal window goes (its place is kept for later), the TV
    // interface comes on top.
    if let Some(main) = app.get_webview_window("main") {
        crate::save_geometry(app);
        let _ = main.destroy();
        state.forget_open_server();
    }
    let _ = ui.show();
    cover(&ui);
    let _ = ui.set_focus();
    crate::refresh_tray(app);
    Ok(())
}

/// Turns the TV mode off: the normal window opens again (directly with the last server), then mpv
/// ends and both windows of the TV mode close.
pub fn leave(app: &AppHandle) {
    let state = app.state::<AppState>();
    let was_on = state.tv.active();
    state.tv.stop();
    if was_on {
        match app.get_webview_window("main") {
            Some(main) => {
                let _ = main.show();
                let _ = main.set_focus();
            }
            None => {
                if let Err(e) = crate::create_main_window(app, true) {
                    eprintln!("FreeX Desktop: no window after the TV mode: {e}");
                }
                // Straight back to the server it showed before (the picker checks it first).
                let last = state.settings.lock().unwrap().last_server.clone();
                if let Some(id) = last {
                    let _ = crate::server::open_picker(app, &state, Some(&id));
                }
            }
        }
    }
    for label in [UI, VIDEO] {
        if let Some(w) = app.get_webview_window(label) {
            let _ = w.destroy();
        }
    }
    if was_on {
        crate::refresh_tray(app);
    }
}

/// What the TV interface knows synchronously (see `tv_bridge.js`): the platform, the codecs
/// mpv plays without conversion and the server of the app.
fn bridge_info<R: Runtime>(app: &AppHandle<R>) -> Value {
    let state = app.state::<AppState>();
    let settings = state.settings.lock().unwrap();
    let server = state
        .current_server()
        .or_else(|| settings.last_server.clone())
        .and_then(|id| settings.servers.iter().find(|s| s.id == id))
        .map(|s| s.url.clone())
        .unwrap_or_default();
    json!({
        "platform": "freexht",
        "shell": "desktop",
        "version": env!("CARGO_PKG_VERSION"),
        "model": "FreeX Desktop",
        "board": "desktop",
        "deviceName": format!("FreeX Desktop TV ({})", computer_name()),
        "caps": capabilities(),
        "server": server,
        "lite": false,
    })
}

/// The name of the computer, as the server lists the device. Desktop apps on Linux and macOS
/// usually do not get HOSTNAME, so the name is read where the system keeps it.
fn computer_name() -> String {
    let from_env = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok();
    let from_system = || -> Option<String> {
        if cfg!(target_os = "macos") {
            // The name from System Settings → General → About ("Niklas' Mac mini").
            let out = std::process::Command::new("scutil")
                .args(["--get", "ComputerName"])
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
        } else {
            std::fs::read_to_string("/etc/hostname")
                .or_else(|_| std::fs::read_to_string("/proc/sys/kernel/hostname"))
                .ok()
        }
    };
    from_env
        .or_else(from_system)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "PC".into())
}

/// mpv decodes practically everything (with the GPU where it can), so the server sends files as
/// they are.
fn capabilities() -> Value {
    json!({
        "containers": ["mp4", "mkv", "webm", "mpegts", "ts", "avi", "mov", "flv", "ogg"],
        "video": ["h264", "hevc", "vp9", "av1", "mpeg4", "mpeg2video", "vc1"],
        "audio": ["aac", "mp3", "ac3", "eac3", "dts", "truehd", "flac", "alac", "opus", "vorbis", "mp2",
                  "pcm_s16le", "pcm_s24le", "pcm_s16be"],
        "mseVideo": ["h264", "hevc"],
        "mseAudio": ["aac", "mp3", "ac3", "eac3", "flac", "opus"],
    })
}

// ---------------------------------------------------------------------------- commands

// Commands that open windows are async: from a synchronous command (on the main thread) that
// deadlocks on Windows (WebView2).
#[tauri::command]
pub async fn tv_enter(app: AppHandle) -> Result<(), String> {
    enter(&app).inspect_err(|e| eprintln!("FreeX Desktop: no TV mode: {e}"))
}

/// Leaves the TV mode (from the app's settings window).
#[tauri::command]
pub fn tv_leave(app: AppHandle) {
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || leave(&app2));
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerArgs {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    start_ms: Option<f64>,
    #[serde(default)]
    ms: Option<f64>,
}

/// The player commands of the bridge: `open`, `play`, `pause`, `seek`, `stop`, `exit`.
#[tauri::command]
pub fn tv_player(
    state: State<'_, AppState>,
    command: String,
    args: Option<PlayerArgs>,
) -> Result<(), String> {
    let args = args.unwrap_or_default();
    let command = match command.as_str() {
        "open" => {
            let url = args.url.ok_or("no url")?;
            // Only streams over HTTP(S), never local files or other protocols.
            if !(url.starts_with("http://") || url.starts_with("https://")) {
                return Err("only http(s) streams can be played".into());
            }
            Command::Open {
                url,
                start_ms: args.start_ms.unwrap_or(0.0).max(0.0) as u64,
            }
        }
        "play" => Command::Play,
        "pause" => Command::Pause,
        "seek" => Command::Seek(args.ms.unwrap_or(0.0).max(0.0) as u64),
        "stop" => Command::Stop,
        // The interface's "close the app" is decided by the interface itself (tv_call).
        "exit" => return Ok(()),
        _ => return Err("unknown player command".into()),
    };
    state.tv.send(command);
    Ok(())
}

/// The other calls of the interface: `info`, `tv.leave`, `app.quit`, `app.settings` (the app's
/// own settings window), `autostart.get`, `autostart.set` (`{on}`).
#[tauri::command]
pub async fn tv_call(app: AppHandle, method: String, args: Option<Value>) -> Result<Value, String> {
    match method.as_str() {
        "autostart.get" => Ok(Value::Bool(crate::autostart::enabled())),
        "autostart.set" => {
            let on = args
                .as_ref()
                .and_then(|a| a.get("on"))
                .and_then(Value::as_bool)
                .ok_or("on missing")?;
            crate::autostart::set(on)?;
            Ok(Value::Bool(crate::autostart::enabled()))
        }
        "info" => Ok(json!({
            "hostname": computer_name(),
            "model": "FreeX Desktop",
            "version": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "addresses": [],
            "network": {"kind": "none", "name": "", "wifiAvailable": false},
        })),
        "tv.leave" => {
            // After the answer: the window that asked is closed by it.
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || leave(&app2));
            Ok(Value::Bool(true))
        }
        "app.quit" => {
            // Straight out, without showing the normal window first; mpv ends with the app.
            app.state::<AppState>().tv.stop();
            let app2 = app.clone();
            let _ = app.run_on_main_thread(move || app2.exit(0));
            Ok(Value::Bool(true))
        }
        "app.settings" => {
            crate::settings::open(&app).map_err(|e| e.to_string())?;
            Ok(Value::Bool(true))
        }
        _ => Err(format!("{method} is not available in FreeX Desktop")),
    }
}

// ---------------------------------------------------------------------------- mpv

static NEXT_PIPE: AtomicU64 = AtomicU64::new(1);

/// mpv's command line for the TV mode: idle and black in the video window (or in its own
/// borderless full-screen window without `wid`), everything on screen besides the picture comes
/// from the interface. `compat` leaves out the options only newer mpv versions know (older ones
/// stop at unknown options), see [`supervise`].
fn mpv_args(ipc: &str, wid: Option<i64>, compat: bool) -> Vec<String> {
    let mut args: Vec<String> = match wid {
        Some(wid) => vec![format!("--wid={wid}")],
        None => vec![
            "--fs".into(),
            "--native-fs=no".into(),
            "--border=no".into(),
            "--keepaspect-window=no".into(),
        ],
    };
    if wid.is_none() && !compat {
        // The TV window keeps the keyboard; mpv stays out of the Dock and Cmd+Tab (mpv 0.38+).
        args.push("--focus-on=never".into());
        args.push("--macos-app-activation-policy=accessory".into());
    }
    args.extend([
        format!("--input-ipc-server={ipc}"),
        "--idle=yes".into(),
        "--force-window=yes".into(),
        "--keep-open=no".into(),
        "--no-terminal".into(),
        "--ytdl=no".into(),
        "--hwdec=auto-safe".into(),
        "--save-position-on-quit=no".into(),
        "--sid=no".into(),
        "--sub-auto=no".into(),
        "--osc=no".into(),
        "--osd-level=0".into(),
        "--osd-bar=no".into(),
        "--input-default-bindings=no".into(),
        "--input-vo-keyboard=no".into(),
        "--input-cursor=no".into(),
        "--cursor-autohide=always".into(),
        // The interface takes the media keys; mpv must not answer them as well.
        "--input-media-keys=no".into(),
        "--load-scripts=no".into(),
        "--network-timeout=75".into(),
        "--cache=yes".into(),
        "--demuxer-max-bytes=150MiB".into(),
        "--demuxer-readahead-secs=20".into(),
        "--hls-bitrate=max".into(),
        format!("--user-agent=FreeXDesktop/{}", env!("CARGO_PKG_VERSION")),
    ]);
    // The Windows media overlay (only the mpv bundled for Windows is new enough to know it;
    // older versions stop at unknown options).
    if cfg!(windows) && !compat {
        args.push("--media-controls=no".into());
    }
    args
}

#[cfg(windows)]
fn pipe_path() -> String {
    format!(
        r"\\.\pipe\freex-tv-{}-{}",
        std::process::id(),
        NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
    )
}

#[cfg(not(windows))]
fn pipe_path() -> String {
    std::env::temp_dir()
        .join(format!(
            "freex-tv-{}-{}.sock",
            std::process::id(),
            NEXT_PIPE.fetch_add(1, Ordering::Relaxed)
        ))
        .to_string_lossy()
        .into_owned()
}

/// Where the page's playback stands, as mpv reports it.
#[derive(Default)]
struct Playback {
    /// Playlist entry of the file the page opened (from `start-file`).
    entry: Option<i64>,
    /// The page opened a file and did not stop it yet.
    open: bool,
    loaded: bool,
    ready: bool,
    position: f64,
    duration: f64,
    paused: bool,
    buffering: bool,
}

impl Playback {
    fn event(&self, kind: &str) -> Value {
        json!({
            "type": kind,
            "position": (self.position * 1000.0) as u64,
            "duration": (self.duration * 1000.0) as u64,
            "playing": self.loaded && !self.paused,
        })
    }

    /// mpv's commands for a page command, and events to send at once.
    fn command(&mut self, command: Command) -> (Vec<Value>, Vec<Value>) {
        match command {
            Command::Open { url, start_ms } => {
                *self = Playback {
                    open: true,
                    position: start_ms as f64 / 1000.0,
                    paused: true,
                    ..Playback::default()
                };
                let start = if start_ms > 0 {
                    format!("{:.3}", start_ms as f64 / 1000.0)
                } else {
                    "none".into()
                };
                (
                    vec![
                        json!(["set_property", "pause", true]),
                        json!(["set_property", "start", start]),
                        json!(["loadfile", url, "replace"]),
                    ],
                    vec![],
                )
            }
            Command::Play => (vec![json!(["set_property", "pause", false])], vec![]),
            Command::Pause => (vec![json!(["set_property", "pause", true])], vec![]),
            Command::Seek(ms) => {
                self.position = ms as f64 / 1000.0;
                (
                    vec![json!(["seek", format!("{:.3}", self.position), "absolute"])],
                    vec![],
                )
            }
            Command::Stop => {
                *self = Playback::default();
                (vec![json!(["stop"])], vec![])
            }
            Command::Quit => (vec![json!(["quit"])], vec![]),
        }
    }

    /// Events for the page from one message of mpv, and commands to send back.
    fn message(&mut self, msg: &Value) -> (Vec<Value>, Vec<Value>) {
        let mut events = Vec::new();
        let mut commands = Vec::new();
        match msg.get("event").and_then(Value::as_str) {
            Some("property-change") => {
                let data = msg.get("data");
                match msg.get("name").and_then(Value::as_str) {
                    Some("time-pos") => {
                        if let Some(s) = data.and_then(Value::as_f64) {
                            self.position = s.max(0.0);
                        }
                    }
                    Some("duration") => {
                        if let Some(s) = data.and_then(Value::as_f64) {
                            self.duration = s.max(0.0);
                        }
                    }
                    Some("pause") => {
                        if let Some(p) = data.and_then(Value::as_bool) {
                            let changed = p != self.paused;
                            self.paused = p;
                            if changed && self.loaded {
                                events.push(self.event("state"));
                            }
                        }
                    }
                    Some("paused-for-cache") | Some("seeking") => {
                        let on = data.and_then(Value::as_bool).unwrap_or(false);
                        if self.loaded && on != self.buffering {
                            self.buffering = on;
                            events.push(json!({"type": "buffering", "on": on}));
                        }
                    }
                    _ => {}
                }
            }
            Some("start-file") => {
                if self.open {
                    self.entry = msg.get("playlist_entry_id").and_then(Value::as_i64);
                }
            }
            Some("file-loaded") => {
                if self.open {
                    self.loaded = true;
                    // The start position was only for this file.
                    commands.push(json!(["set_property", "start", "none"]));
                }
            }
            Some("playback-restart") => {
                if self.loaded && !self.ready {
                    // First picture at the start position: ready, paused until the page says play.
                    self.ready = true;
                    events.push(self.event("ready"));
                    events.push(json!({"type": "buffering", "on": false}));
                } else if self.loaded {
                    events.push(self.event("time"));
                }
            }
            Some("end-file") => {
                let entry = msg.get("playlist_entry_id").and_then(Value::as_i64);
                // Only the file the page opened (not the one it replaced).
                if !self.open || self.entry.is_none() || entry != self.entry {
                    return (events, commands);
                }
                let was_loaded = self.loaded;
                self.loaded = false;
                match msg.get("reason").and_then(Value::as_str) {
                    Some("eof") if was_loaded => events.push(self.event("ended")),
                    Some("error") => {
                        let detail = msg
                            .get("file_error")
                            .and_then(Value::as_str)
                            .unwrap_or("cannot play this file");
                        events.push(json!({"type": "error", "message": format!("mpv: {detail}")}));
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        (events, commands)
    }
}

fn emit<R: Runtime>(app: &AppHandle<R>, event: Value) {
    let _ = app.emit_to(UI, EVENT, event);
}

/// Keeps mpv running for the TV mode: started again if it dies (a video that was open then ends
/// with an error), ended by [`Command::Quit`] or when the TV mode's channel closes.
async fn supervise<R: Runtime>(
    app: AppHandle<R>,
    mpv: PathBuf,
    wid: Option<i64>,
    mut commands: mpsc::UnboundedReceiver<Command>,
) {
    let mut playback = Playback::default();
    // mpv that ends right after its start will not get better by starting it again (an option
    // it does not know, no video output): after two such starts it runs without the newer
    // options, after four the TV mode gives up and says so instead of trying forever.
    let mut quick_failures = 0;
    loop {
        let compat = quick_failures >= 2;
        let started = Instant::now();
        match run(&app, &mpv, wid, compat, &mut commands, &mut playback).await {
            Ended::Quit => return,
            Ended::Died(reason) => {
                eprintln!("FreeX Desktop: mpv of the TV mode ended ({reason})");
                if playback.open {
                    emit(
                        &app,
                        json!({"type": "error", "message": "The player stopped unexpectedly."}),
                    );
                }
                playback = Playback::default();
                if started.elapsed() < Duration::from_secs(10) {
                    quick_failures += 1;
                } else {
                    quick_failures = 0;
                }
                if quick_failures >= 4 {
                    let message = format!("mpv cannot be started: {reason}");
                    eprintln!("FreeX Desktop: {message}; the TV mode plays nothing");
                    refuse_playback(&app, &mut commands, &message).await;
                    return;
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

/// Without a working mpv: answers every video with an error until the TV mode ends.
async fn refuse_playback<R: Runtime>(
    app: &AppHandle<R>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    message: &str,
) {
    emit(app, json!({"type": "error", "message": message}));
    while let Some(c) = commands.recv().await {
        match c {
            Command::Quit => break,
            Command::Open { .. } => emit(app, json!({"type": "error", "message": message})),
            _ => {}
        }
    }
}

enum Ended {
    Quit,
    Died(String),
}

async fn run<R: Runtime>(
    app: &AppHandle<R>,
    mpv: &PathBuf,
    wid: Option<i64>,
    compat: bool,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    playback: &mut Playback,
) -> Ended {
    let ipc = pipe_path();
    let mut cmd = tokio::process::Command::new(mpv);
    cmd.args(mpv_args(&ipc, wid, compat));
    cmd.kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            // Without mpv there is nothing to restart: wait for the TV mode to end.
            let message = format!("mpv could not be started: {e}");
            eprintln!("FreeX Desktop: {message}");
            refuse_playback(app, commands, &message).await;
            return Ended::Quit;
        }
    };

    let deadline = Instant::now() + Duration::from_secs(15);
    let stream = loop {
        match crate::mpv::connect(&ipc).await {
            Ok(stream) => break stream,
            Err(e) => {
                if let Ok(Some(status)) = child.try_wait() {
                    return Ended::Died(format!("exited at start ({status})"));
                }
                if Instant::now() >= deadline {
                    let _ = child.kill().await;
                    return Ended::Died(format!("no IPC: {e}"));
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    };
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    for (n, name) in [
        "time-pos",
        "duration",
        "pause",
        "paused-for-cache",
        "seeking",
    ]
    .iter()
    .enumerate()
    {
        let line = format!(
            "{}\n",
            json!({"command": ["observe_property", n + 1, name]})
        );
        if writer.write_all(line.as_bytes()).await.is_err() {
            return Ended::Died("IPC closed".into());
        }
    }

    let mut ticker = tokio::time::interval(Duration::from_millis(500));
    let ended = loop {
        tokio::select! {
            command = commands.recv() => {
                let quit = matches!(command, None | Some(Command::Quit));
                let (out, events) = playback.command(command.unwrap_or(Command::Quit));
                for e in events {
                    emit(app, e);
                }
                for c in out {
                    let line = format!("{}\n", json!({"command": c}));
                    if writer.write_all(line.as_bytes()).await.is_err() {
                        break;
                    }
                }
                if quit {
                    break Ended::Quit;
                }
            }
            line = lines.next_line() => {
                let Ok(Some(line)) = line else { break Ended::Died("IPC closed".into()) };
                let Ok(msg) = serde_json::from_str::<Value>(&line) else { continue };
                let (events, out) = playback.message(&msg);
                for e in events {
                    emit(app, e);
                }
                for c in out {
                    let line = format!("{}\n", json!({"command": c}));
                    let _ = writer.write_all(line.as_bytes()).await;
                }
            }
            _ = ticker.tick() => {
                if playback.loaded && playback.ready {
                    emit(app, playback.event("time"));
                }
            }
            status = child.wait() => {
                break Ended::Died(format!("{status:?}"));
            }
        }
    };
    if matches!(ended, Ended::Quit)
        && tokio::time::timeout(Duration::from_secs(3), child.wait())
            .await
            .is_err()
    {
        let _ = child.kill().await;
    }
    #[cfg(not(windows))]
    let _ = std::fs::remove_file(&ipc);
    ended
}

/// Brings the TV interface to the front if the TV mode is on; false otherwise.
pub fn focus<R: Runtime>(app: &AppHandle<R>) -> bool {
    match app.get_webview_window(UI) {
        Some(ui) => {
            if let Some(video) = app.get_webview_window(VIDEO) {
                let _ = video.unminimize();
            }
            let _ = ui.show();
            let _ = ui.set_focus();
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Command, Playback};
    use serde_json::json;

    fn types(events: &[serde_json::Value]) -> Vec<String> {
        events
            .iter()
            .map(|e| e["type"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn a_playback_from_open_to_its_end() {
        let mut p = Playback::default();
        let (out, _) = p.command(Command::Open {
            url: "http://nas:32500/stream?token=x".into(),
            start_ms: 30_000,
        });
        assert_eq!(out[0], json!(["set_property", "pause", true]));
        assert_eq!(out[1], json!(["set_property", "start", "30.000"]));
        assert_eq!(out[2][0], "loadfile");
        // The file before (replaced) ends: nothing for the page.
        let (ev, _) =
            p.message(&json!({"event": "end-file", "reason": "stop", "playlist_entry_id": 1}));
        assert!(ev.is_empty());
        p.message(&json!({"event": "start-file", "playlist_entry_id": 2}));
        p.message(&json!({"event": "property-change", "name": "duration", "data": 120.0}));
        p.message(&json!({"event": "property-change", "name": "time-pos", "data": 30.0}));
        let (_, cmds) = p.message(&json!({"event": "file-loaded"}));
        assert_eq!(cmds, vec![json!(["set_property", "start", "none"])]);
        let (ev, _) = p.message(&json!({"event": "playback-restart"}));
        assert_eq!(types(&ev), ["ready", "buffering"]);
        assert_eq!(ev[0]["position"], 30_000);
        assert_eq!(ev[0]["duration"], 120_000);
        assert_eq!(ev[0]["playing"], false);
        let (ev, _) =
            p.message(&json!({"event": "property-change", "name": "pause", "data": false}));
        assert_eq!(types(&ev), ["state"]);
        assert_eq!(ev[0]["playing"], true);
        let (ev, _) = p.message(
            &json!({"event": "property-change", "name": "paused-for-cache", "data": true}),
        );
        assert_eq!(ev, vec![json!({"type": "buffering", "on": true})]);
        let (ev, _) =
            p.message(&json!({"event": "end-file", "reason": "eof", "playlist_entry_id": 2}));
        assert_eq!(types(&ev), ["ended"]);
    }

    #[test]
    fn a_broken_file_is_an_error_and_a_stop_is_quiet() {
        let mut p = Playback::default();
        p.command(Command::Open {
            url: "http://nas/broken".into(),
            start_ms: 0,
        });
        p.message(&json!({"event": "start-file", "playlist_entry_id": 5}));
        let (ev, _) = p.message(&json!({"event": "end-file", "reason": "error", "playlist_entry_id": 5, "file_error": "loading failed"}));
        assert_eq!(
            ev,
            vec![json!({"type": "error", "message": "mpv: loading failed"})]
        );

        p.command(Command::Open {
            url: "http://nas/a".into(),
            start_ms: 0,
        });
        p.message(&json!({"event": "start-file", "playlist_entry_id": 6}));
        p.message(&json!({"event": "file-loaded"}));
        let (out, _) = p.command(Command::Stop);
        assert_eq!(out, vec![json!(["stop"])]);
        let (ev, _) =
            p.message(&json!({"event": "end-file", "reason": "stop", "playlist_entry_id": 6}));
        assert!(ev.is_empty());
    }

    #[test]
    fn the_command_line_never_has_mpv_answer_keys() {
        let own = super::mpv_args("pipe", None, false);
        assert!(own.iter().any(|x| x == "--focus-on=never"));
        assert!(!own.iter().any(|x| x.starts_with("--wid")));
        // For older mpv versions without the newer options.
        let compat = super::mpv_args("pipe", None, true);
        assert!(!compat
            .iter()
            .any(|x| x.starts_with("--focus-on") || x.starts_with("--macos-app")));
        assert!(compat.iter().any(|x| x == "--fs"));
        let args = super::mpv_args("pipe", Some(4242), false);
        assert!(!args.iter().any(|x| x == "--fs"));
        for a in [
            "--wid=4242",
            "--idle=yes",
            "--input-default-bindings=no",
            "--input-media-keys=no",
            "--sid=no",
        ] {
            assert!(args.iter().any(|x| x == a), "{a}");
        }
    }
}
