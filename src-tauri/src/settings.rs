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

//! The app's own settings window (`ui/settings.html`).
//!
//! Everything that belongs to this PC rather than to a server lives here, bundled with the app
//! and independent of FreeX Web and of the TV interface: switching between the normal window and
//! the TV mode, starting in the TV mode at login, the player, the server, quitting. It opens from
//! the tray menu, with Ctrl+, (Cmd+, on macOS) in every window and from the TV interface's
//! Settings → FreeX Desktop (`app.settings`). It works with a remote control as well: arrow
//! keys, OK and Back.

use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::server::AppState;

/// Label of the settings window.
pub const LABEL: &str = "settings";

/// Opens the settings window, or brings it to the front if it is open. In the TV mode it lies
/// above the TV interface.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    let tv = app.try_state::<AppState>().is_some_and(|s| s.tv.active());
    let german = sys_locale::get_locale().is_some_and(|l| l.to_lowercase().starts_with("de"));
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("settings.html".into()))
        .title(if german {
            "FreeX – Einstellungen"
        } else {
            "FreeX – Settings"
        })
        .inner_size(540.0, 760.0)
        .min_inner_size(420.0, 480.0)
        .center()
        .theme(Some(tauri::Theme::Dark))
        .background_color(tauri::window::Color(0x12, 0x18, 0x21, 0xff))
        .initialization_script(crate::NATIVE_JS)
        .zoom_hotkeys_enabled(true);
    // In the TV mode: owned by the TV window and above it (it is full screen and on macOS
    // floats above everything).
    let builder = crate::shared_webview(builder);
    let builder = match app.get_webview_window(crate::tv::UI).filter(|_| tv) {
        Some(ui) => builder.parent(&ui)?.always_on_top(true),
        None => builder,
    };
    let window = builder.build()?;
    let _ = window.set_focus();
    Ok(())
}

/// Async: opening a window from a synchronous command deadlocks on Windows (WebView2).
#[tauri::command]
pub async fn settings_open<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    open(&app).map_err(|e| e.to_string())
}

/// Quits the app (from the settings window), keeping the normal window's place.
#[tauri::command]
pub fn app_quit(app: AppHandle) {
    if let Some(state) = app.try_state::<AppState>() {
        state.tv.stop();
        state.mpv.stop();
    }
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        crate::save_geometry(&app2);
        app2.exit(0);
    });
}
