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

//! Starting FreeX in the TV mode when the user logs in (`freex-desktop --tv`), the way each
//! system does it for the current user:
//!
//! - Windows: the value "FreeX TV" under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//!   (the installer's option "Start in TV mode at login" writes the same value). The Store app
//!   (MSIX package) cannot use that key; it has the startup task "FreeXTV" in its manifest
//!   (packaging/msix), which starts it at login and is switched with the StartupTask API.
//! - Linux: `~/.config/autostart/freex-desktop-tv.desktop` (XDG autostart)
//! - macOS: the launch agent `~/Library/LaunchAgents/de.mirranet.freex.desktop.tv.plist`

use std::path::PathBuf;

/// Name of the value / file of the autostart entry.
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
#[cfg(windows)]
const RUN_VALUE: &str = "FreeX TV";
#[cfg(target_os = "macos")]
const AGENT: &str = "de.mirranet.freex.desktop.tv";

fn exe() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("cannot find the app: {e}"))
}

// ---------------------------------------------------------------------------- Windows

#[cfg(windows)]
fn reg(args: &[&str]) -> std::io::Result<std::process::Output> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
}

#[cfg(windows)]
pub fn enabled() -> bool {
    if packaged::is_packaged() {
        return packaged::enabled();
    }
    reg(&["query", RUN_KEY, "/v", RUN_VALUE]).is_ok_and(|o| o.status.success())
}

#[cfg(windows)]
pub fn set(on: bool) -> Result<(), String> {
    if packaged::is_packaged() {
        return packaged::set(on);
    }
    let out = if on {
        let command = format!("\"{}\" --tv", exe()?.display());
        reg(&[
            "add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &command, "/f",
        ])
    } else {
        if !enabled() {
            return Ok(());
        }
        reg(&["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
    }
    .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The Store app: runs from an MSIX package, starts at login through its startup task.
#[cfg(windows)]
pub mod packaged {
    use windows::core::HSTRING;
    use windows::ApplicationModel::Activation::ActivationKind;
    use windows::ApplicationModel::{AppInstance, StartupTask, StartupTaskState};
    use windows::Win32::Foundation::ERROR_INSUFFICIENT_BUFFER;
    use windows::Win32::Storage::Packaging::Appx::GetCurrentPackageFullName;

    /// `TaskId` of the startup task in packaging/msix/AppxManifest.xml.
    const TASK: &str = "FreeXTV";

    /// The app runs from an MSIX package (the Store app), not from the installer.
    pub fn is_packaged() -> bool {
        let mut len = 0u32;
        // Without a package: APPMODEL_ERROR_NO_PACKAGE; with one: the name does not fit.
        unsafe { GetCurrentPackageFullName(&mut len, None) == ERROR_INSUFFICIENT_BUFFER }
    }

    /// The Store app was started by its startup task (at login): it starts in the TV mode, as
    /// the installed app does with `--tv`.
    pub fn started_at_login() -> bool {
        is_packaged()
            && AppInstance::GetActivatedEventArgs()
                .and_then(|a| a.Kind())
                .is_ok_and(|k| k == ActivationKind::StartupTask)
    }

    fn task() -> Result<StartupTask, String> {
        StartupTask::GetAsync(&HSTRING::from(TASK))
            .and_then(|op| op.join())
            .map_err(|e| format!("startup task: {e}"))
    }

    pub fn enabled() -> bool {
        task()
            .and_then(|t| t.State().map_err(|e| e.to_string()))
            .is_ok_and(|s| s == StartupTaskState::Enabled || s == StartupTaskState::EnabledByPolicy)
    }

    pub fn set(on: bool) -> Result<(), String> {
        let task = task()?;
        if !on {
            return task.Disable().map_err(|e| e.to_string());
        }
        let state = task
            .RequestEnableAsync()
            .and_then(|op| op.join())
            .map_err(|e| e.to_string())?;
        match state {
            StartupTaskState::Enabled | StartupTaskState::EnabledByPolicy => Ok(()),
            // Only the user can switch it on again (Settings > Apps > Startup).
            StartupTaskState::DisabledByUser => Err(
                "switched off in the Windows settings (Apps > Startup); switch on FreeX TV there"
                    .into(),
            ),
            _ => Err("not allowed on this PC (policy)".into()),
        }
    }
}

// ---------------------------------------------------------------------------- Linux

#[cfg(target_os = "linux")]
fn desktop_file() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("autostart").join("freex-desktop-tv.desktop"))
}

/// The XDG autostart entry for `exe`.
#[cfg(any(target_os = "linux", test))]
fn desktop_entry(exe: &std::path::Path) -> String {
    // Exec quoting (desktop entry spec): the path in double quotes, with ", `, $ and \ escaped.
    let mut quoted = String::new();
    for c in exe.to_string_lossy().chars() {
        if matches!(c, '"' | '`' | '$' | '\\') {
            quoted.push('\\');
        }
        quoted.push(c);
    }
    format!(
        "[Desktop Entry]\nType=Application\nName=FreeX TV\nComment=FreeX in TV mode\n\
         Exec=\"{quoted}\" --tv\nIcon=freex-desktop\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

#[cfg(target_os = "linux")]
pub fn enabled() -> bool {
    desktop_file().is_some_and(|p| p.is_file())
}

#[cfg(target_os = "linux")]
pub fn set(on: bool) -> Result<(), String> {
    let path = desktop_file().ok_or("no home directory")?;
    if on {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, desktop_entry(&exe()?)).map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------- macOS

#[cfg(target_os = "macos")]
fn agent_file() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|h| {
        PathBuf::from(h)
            .join("Library/LaunchAgents")
            .join(format!("{AGENT}.plist"))
    })
}

/// The launch agent for `exe`: runs it with `--tv` at login, in the graphical session.
#[cfg(any(target_os = "macos", test))]
fn launch_agent(label: &str, exe: &std::path::Path) -> String {
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--tv</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>LimitLoadToSessionType</key>
    <string>Aqua</string>
</dict>
</plist>
"#,
        esc(label),
        esc(&exe.to_string_lossy())
    )
}

#[cfg(target_os = "macos")]
pub fn enabled() -> bool {
    agent_file().is_some_and(|p| p.is_file())
}

#[cfg(target_os = "macos")]
pub fn set(on: bool) -> Result<(), String> {
    let path = agent_file().ok_or("no home directory")?;
    if on {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, launch_agent(AGENT, &exe()?)).map_err(|e| e.to_string())
    } else {
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
            _ => Ok(()),
        }
    }
}

// ---------------------------------------------------------------------------- elsewhere

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn enabled() -> bool {
    false
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub fn set(_on: bool) -> Result<(), String> {
    Err("not available on this system".into())
}

// ---------------------------------------------------------------------------- commands

// Async: the Store app waits for the StartupTask API there, which must not block the window.
#[tauri::command]
pub async fn autostart_get() -> bool {
    enabled()
}

#[tauri::command]
pub async fn autostart_set(on: bool) -> Result<bool, String> {
    set(on)?;
    Ok(enabled())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn the_desktop_entry_quotes_the_path() {
        let e = super::desktop_entry(Path::new("/opt/Free X/$bin/freex-desktop"));
        assert!(e.contains("Exec=\"/opt/Free X/\\$bin/freex-desktop\" --tv\n"));
        assert!(e.starts_with("[Desktop Entry]\n"));
    }

    #[test]
    fn the_launch_agent_runs_the_app_with_tv() {
        let p = super::launch_agent(
            "x.y",
            Path::new("/Applications/FreeX.app/Contents/MacOS/freex-desktop"),
        );
        assert!(p.contains("<string>/Applications/FreeX.app/Contents/MacOS/freex-desktop</string>"));
        assert!(p.contains("<string>--tv</string>"));
        assert!(p.contains("<key>RunAtLoad</key>\n    <true/>"));
    }
}
