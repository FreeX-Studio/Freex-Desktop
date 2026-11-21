<p align="center">
  <img src="docs/brand/freex-logo.svg" alt="FreeX" width="128">
</p>

<h1 align="center">FreeX Desktop</h1>

<p align="center">
  <strong>Your Media. Your Server. Your Freedom.</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: LGPL-3.0" src="https://img.shields.io/badge/license-LGPL--3.0-blue"></a>
  <img alt="Windows, Linux and macOS" src="https://img.shields.io/badge/platform-Windows%20%7C%20Linux%20%7C%20macOS-0078d4">
  <img alt="Tauri 2" src="https://img.shields.io/badge/tauri-2-ffc131">
</p>

FreeX Desktop is the desktop app for [FreeX Media Server](https://github.com/FreeX-Studio/Freex-Media-Server),
for Windows, Linux and macOS. It connects to your server and shows FreeX Web, the interface of the server itself, in its
own window – so the app always matches the version of the server it talks to. Around it, the app behaves like a
native program:

- **Your servers in one place** – add a server by its address (`192.168.1.10`, `nas`, `https://media.example.org`),
  switch between several servers (also straight from the tray menu), and connect to the last one right away on
  start. A server on the same PC is found automatically.
- **Plays every file directly with mpv, inside the app window** – optionally, videos play in [mpv](https://mpv.io),
  which decodes practically every format itself. The server does not have to convert anything; your position and
  watch state are saved on the server as usual, and the playback shows up in the activity view. mpv draws into
  the FreeX window (on Windows and on Linux with X11; on Wayland and macOS it opens its own window).
- **Media keys** for mpv (Windows media overlay; on Linux through `mpv-mpris`; on macOS built into mpv).
- **An app, not a browser** – dark title bar, F11 for full screen, Alt+← / Alt+→ (Cmd+[ / Cmd+] on macOS) for
  back and forward, no
  browser context menu and no browser shortcuts (print, find, view source), a tray icon, one running instance and
  remembered window size.

- **TV mode** – the PC as a FreeX box on the TV: the interface of the FreeX TV apps (the same as on Samsung, LG,
  Fire TV and FreeXHT, not FreeX Web) in full screen, for a remote control, an air mouse or a keyboard, with mpv as
  its player. Start and leave it from the tray menu, with **Ctrl+Shift+T** (Cmd+Shift+T on macOS), in the app's settings
  window or with `freex-desktop --tv`; *Start in TV mode at login* turns the PC into a TV box (also an option of
  the Windows installer).

The family: **FreeX Media Server** (core), **FreeX Web** (built into the server), **FreeX Desktop** (this
repository), and the planned **FreeX iOS**, **FreeX Android** and **FreeX TV**.

## TV mode

The TV mode shows the interface of [FreeX-TV-Apps](https://github.com/FreeX-Studio/FreeX-TV-Apps), bundled into the
app at build time (`packaging/tv-ui.sh` puts its `web/` into `ui/tv/`). It works like the FreeXHT system image:
mpv plays in a black full-screen window, the interface lies on top of it in a transparent full-screen window and
drives mpv through the same bridge as on the TV apps (`window.FreeXNative`, `src-tauri/src/tv_bridge.js`). The seek
bar, seek previews, *Skip intro*, *Next episode* and text subtitles come from the interface; the server sends files
as they are (mpv plays them).

- Text fields open an on-screen keyboard (for remotes); typing on a keyboard hides it.
- **Back** on the home screen asks: close FreeX, leave the TV mode, or cancel. **Settings → FreeX Desktop**: leave
  the TV mode, quit, and *Start in TV mode at login*; with a keyboard, **Ctrl+,** opens the app's settings window
  above the TV mode.
- If mpv cannot be started (it ends right away four times in a row, e.g. an old version), the TV mode stops trying
  and shows the error with the first video instead of restarting it forever.
- The TV mode replaces the normal window instead of running on top of it: switching closes FreeX Web and opens the
  TV interface, and leaving the TV mode opens FreeX Web again, directly with the last server. **Ctrl+Shift+T**
  (Cmd+Shift+T) switches both ways.
- The TV mode signs in separately (it is its own interface); the server comes from the app (the last one opened).

| System | How |
|---|---|
| Windows | mpv draws into the app's video window (`--wid`); the TV window is owned by it and stays on top. Autostart: `HKCU\…\Run`, value "FreeX TV". |
| Linux | the same on X11 (needs a compositing window manager for the transparency, which every current desktop has). On Wayland the app runs the TV mode through XWayland (`GDK_BACKEND=x11`, started again for it). Autostart: `~/.config/autostart/freex-desktop-tv.desktop`. |
| macOS | mpv opens its own borderless full-screen window (without Dock icon, without taking the keyboard), the TV window floats above it. Autostart: the launch agent `~/Library/LaunchAgents/de.mirranet.freex.desktop.tv.plist`. |

## Install

**Windows:** run `freex-desktop-<version>-setup.exe` from the build. It installs the app with mpv into
`C:\Program Files\FreeX Desktop`, adds FreeX to the start menu (and, if chosen, to the desktop) and installs the
Microsoft Edge WebView2 Runtime if it is missing (only needed on older Windows 10 installations).

> [!NOTE]
> **"Windows protected your PC" / unknown publisher:** the installer is not code-signed yet, so Microsoft Defender
> SmartScreen warns when you start it after downloading. This is expected and not a sign of a problem with the
> file. Click **More info** and then **Run anyway** to install. (A signed installer is planned; even then, SmartScreen
> may warn for new versions until they have been downloaded often enough.)

**Windows, Microsoft Store:** every build also makes `freex-desktop-<version>-x64.msix`, the same app with mpv as
an MSIX package for the Store. The Store signs it on publishing, so the Store version starts without any SmartScreen
warning. It needs Windows 10 2004 or later; "Start in TV mode at login" is a startup task there, which can also be
switched in Windows under *Settings → Apps → Startup*.

**Linux (Debian, Ubuntu):** install from the MirraNET APT repository at `apt.mirranet.de` (Debian 12+, Ubuntu
22.04+). First import its signing key, then add the repository and install; every new build arrives with
`sudo apt upgrade`:

```bash
# Signing key of the repository
sudo install -d /etc/apt/keyrings
sudo curl -fsSL https://apt.mirranet.de/mirranet.gpg -o /etc/apt/keyrings/mirranet.gpg

# Repository and package
echo "deb [signed-by=/etc/apt/keyrings/mirranet.gpg] https://apt.mirranet.de stable main" \
    | sudo tee /etc/apt/sources.list.d/mirranet.list
sudo apt update && sudo apt install freex-desktop
```

The repository is the same as for FreeX Media Server; if it is already set up, `sudo apt install freex-desktop` is
enough. Without the repository, install a package file from your CI with
`sudo apt install ./freex-desktop_<version>_amd64.deb`.

The package brings mpv and WebKitGTK from the distribution and recommends the GStreamer plugins for the built-in
FreeX player and `mpv-mpris` for the media keys. FreeX then is in the application menu.

**macOS:** open `freex-desktop-<version>-macos.dmg` and drag **FreeX** to **Applications**. The app runs on Apple
silicon and Intel Macs. For playback in mpv, install it with [Homebrew](https://brew.sh) (`brew install mpv`) or put
`mpv.app` into Applications; without mpv, FreeX uses its built-in player. On the first connection macOS asks
whether FreeX may find devices in your local network – allow it, otherwise FreeX cannot reach a server at home.

> [!NOTE]
> **"FreeX" cannot be opened / Apple could not verify it:** the app is not signed with an Apple Developer ID yet, so
> Gatekeeper blocks the first start after a download. Open **System Settings → Privacy & Security**, scroll down to
> the message about FreeX and click **Open Anyway** (then confirm with your password). This is needed only once
> per version.

Start FreeX, enter the address of your server and sign in. Sign-ins are kept per server, as in a browser.

| What | Windows | Linux | macOS |
|---|---|---|---|
| Program | `C:\Program Files\FreeX Desktop` (with `mpv\`) | `/usr/bin/freex-desktop` (mpv from the system) | `/Applications/FreeX.app` (mpv from Homebrew or `mpv.app`) |
| Settings (servers, player, window) | `%APPDATA%\de.mirranet.freex.desktop\settings.json` | `~/.config/de.mirranet.freex.desktop/settings.json` | `~/Library/Application Support/de.mirranet.freex.desktop/settings.json` |
| Web view data (sign-ins, cache) | `%LOCALAPPDATA%\de.mirranet.freex.desktop` | `~/.local/share/de.mirranet.freex.desktop` | `~/Library/WebKit/de.mirranet.freex.desktop` |

Updates install over the old version and keep all settings.

## Using it

- **App settings:** the tray menu → *Settings …* or **Ctrl+,** (Cmd+, on macOS) opens FreeX's own settings window,
  in the normal window as well as in the TV mode: switch between the normal window and the TV mode, *Start in TV
  mode at login*, the player, the server and quitting. These settings belong to this PC and are stored in the app,
  not on the server; the window works with a remote control too (arrows, OK, Back).
- **Switch server:** the tray menu (one click per server), account menu → *Switch server*, *Settings → FreeX
  Desktop*, or the sign-in page. The tray menu also works when a server is unreachable.
- **Client options in FreeX Web:** while signed in, *Settings → Client options* (also in the account menu) switches
  to the TV mode, opens the app's settings window and sets the player. Like the settings window, it stores
  everything in the app on this PC, not on the server.
- **Player:** the app's settings window, *Settings → Client options* in FreeX Web, or the app settings below the
  server list. *FreeX player* is the player
  of FreeX Web with skip intro, up next and Watch Together. *mpv* plays every file directly; when an episode has
  played to the end, the next one starts after a short countdown. Watch Together always uses the FreeX player.
- **mpv in the window:** mpv's keys work as usual (Space pause, ← → seek, F full screen, Q stop). Esc leaves full
  screen and, pressed again, stops; the position is saved. *Play inside the FreeX window* in the app settings
  switches back to mpv's own window.
- mpv reads your own `mpv.conf` and scripts as usual (`%APPDATA%\mpv`, `~/.config/mpv`).
- **macOS:** closing the window keeps FreeX running, as Mac apps do; click its Dock icon to bring it back, and
  quit with Cmd+Q. The FreeX icon in the menu bar has the same menu as the tray on Windows and Linux.

## Requirements

- Windows 10 (1809 or later) or Windows 11, 64-bit
- Linux: 64-bit with WebKitGTK 4.1 (2.40 or newer), e.g. Debian 12, Ubuntu 22.04 or newer
- macOS 11 (Big Sur) or newer, Apple silicon or Intel
- FreeX Media Server 0.14.0 or newer (for the mpv player; older servers work with the FreeX player)

## Building

The app is built with [Tauri 2](https://tauri.app) (Rust). The Linux and Windows versions are built on Linux, so they
fit into the same Jenkins as the server; the macOS version is built on a Mac:

```bash
sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev \
    librsvg2-dev librsvg2-bin dpkg gcc-mingw-w64-x86-64 nsis curl unzip git cmake clang
rustup target add x86_64-pc-windows-gnu

# Linux: the app and the Debian package
(cd src-tauri && cargo build --release --locked --features custom-protocol)
packaging/deb/build-deb.sh 1.1.1 src-tauri/target/release/freex-desktop dist

# Windows (cross-compiled): the app and the installer
packaging/windows/build.sh            # dist/freex-desktop-<version>-setup.exe
packaging/windows/build.sh --app      # only dist/freex-desktop.exe
packaging/msix/build.sh               # afterwards: dist/freex-desktop-<version>-x64.msix (Microsoft Store)

# macOS (on a Mac with Xcode or the Command Line Tools): universal app, .dmg and .zip
packaging/macos/build.sh
```

The Windows script downloads mpv and Microsoft's WebView2 bootstrapper once into `~/.cache/freex-build`; set
`MPV_SHA256` and `WEBVIEW2_SHA256` to pin them. A distribution toolchain without the Windows standard library but
with `rust-src` also works (the script then builds the standard library from source). The `Jenkinsfile` checks
(rustfmt, Clippy), tests and builds both and archives the `.deb`, the setup, the `.msix` and the plain Windows app. Every build
gets its own package version (`1.1.1+202611142030.gitabc1234`), and builds of `main` go into the APT repository with
`packaging/apt/publish.sh` (set up in Jenkins with `APT_REPO`, `APT_HOST` and optionally `APT_SUITE`). The package
asks for the glibc version the binary needs; for Debian 12 and Ubuntu 22.04 the agent must not be newer than those.
The last stage builds the Mac app on the Jenkins node `Spark` and archives its `.dmg` and `.zip`. Without settings
the Mac app is signed ad hoc; with `MACOS_SIGN_IDENTITY` (a Developer ID) and `MACOS_NOTARY_PROFILE` (stored with
`xcrun notarytool store-credentials`) the script signs and notarizes it, and Gatekeeper no longer asks.

#### Microsoft Store

`packaging/msix/build.sh` packs the Windows app as laid out for the installer (`dist/windows`) into an unsigned MSIX
package with `makemsix` from Microsoft's [msix-packaging](https://github.com/microsoft/msix-packaging), which the
script builds once into `~/.cache/freex-build` (needs git, cmake 3.29+ and clang). The manifest is
`packaging/msix/AppxManifest.xml`: a full-trust desktop app (`runFullTrust`) with the startup task `FreeXTV` for
"Start in TV mode at login".

To publish:

1. In [Partner Center](https://partner.microsoft.com/dashboard), reserve the app name and open *Product identity*.
2. Set its values in Jenkins (*Manage Jenkins → System → Global properties*): `MSIX_IDENTITY` =
   *Package/Identity/Name*, `MSIX_PUBLISHER` = *Package/Identity/Publisher* (`CN=…`), `MSIX_PUBLISHER_NAME` =
   *Package/Properties/PublisherDisplayName*. Without them the package has placeholders and the Store refuses it.
3. Upload the `.msix` of a build to a submission under *Packages*. `runFullTrust` is a restricted capability: give
   as reason that FreeX Desktop is a desktop app that plays media with the bundled mpv.
4. Every upload needs a higher version: raise `version` in `src-tauri/Cargo.toml` (the package gets
   `<version>.0`, e.g. `1.2.0.0`; the Store wants the fourth number to be 0).

To try the package before uploading, it must be signed with a certificate the PC trusts (e.g. a self-signed one with
the same `CN=…` as `MSIX_PUBLISHER`, signed with `signtool` on Windows), or unpacked and registered in developer mode
with `Add-AppxPackage -Register AppxManifest.xml`.

### Project structure

| Path | |
|---|---|
| `src-tauri/src/main.rs` | Window, tray menu, single instance, window geometry, Linux specifics |
| `src-tauri/src/server.rs` | Finding a server, opening it, the commands its pages may call |
| `src-tauri/src/mpv.rs` | Playback in mpv over its JSON IPC, inside the app window |
| `src-tauri/src/native.js` | Runs in every page: app instead of browser behaviour, keys for mpv |
| `src-tauri/src/settings.rs` | The app's own settings window (TV mode, start at login, player, server) |
| `src-tauri/src/tv.rs`, `tv_bridge.js` | TV mode: its windows, mpv for the TV interface, the `FreeXNative` bridge |
| `src-tauri/src/autostart.rs` | Starting in the TV mode at login (Run key or the Store app's startup task, XDG autostart, launch agent) |
| `src-tauri/src/store.rs` | `settings.json` |
| `src-tauri/icons/` | App icons; `app-icon.svg` is the source of the PNG icons, `app-icon-macos.svg` of `icon.icns` |
| `ui/` | The server picker and the settings window (plain HTML, CSS and JavaScript); `ui/tv/` comes from FreeX-TV-Apps at build time |
| `packaging/windows/` | Installer (NSIS) and build script |
| `packaging/msix/` | MSIX package for the Microsoft Store: manifest and build script |
| `packaging/macos/` | `Info.plist` and build script of the Mac app (`.app`, `.dmg`) |
| `packaging/deb/` | Debian package: menu entry, AppStream data, build script |
| `packaging/apt/` | Publishing to the MirraNET APT repository (same script as FreeX Media Server) |

### How it works

The window starts with the bundled picker. When a server is opened, the app grants the pages of exactly that
origin a small capability at runtime – app info, player choice, back to the picker, mpv, window full screen – and
loads FreeX Web from the server. FreeX Web notices the app (`window.__TAURI__`, see `desktop.js` in the server)
and adds the desktop features. Links to other sites open in the default browser. `native.js` is injected into
every page before its own scripts and turns off what makes a web view feel like a browser.

For mpv, FreeX Web announces the playback with `POST /api/items/{id}/playback` and `"player": "external"` (the
server then always plans direct play) and hands the stream URL to the app. The app only accepts URLs of the
page's own server, starts mpv with a JSON IPC pipe, sends the sign-in token over that pipe (never on the command
line), and reports the position back as `freex-mpv` events, which FreeX Web saves on the server. Inside the app
window, mpv gets the window's native handle (`--wid`); its full screen becomes the window's full screen, and as
the keyboard stays with the web view, `native.js` passes key presses on to mpv while it plays.

## License

FreeX Desktop is free software under the [GNU Lesser General Public License v3.0](LICENSE). The installer bundles
mpv (GPL v2+), see [`packaging/windows/THIRD-PARTY.txt`](packaging/windows/THIRD-PARTY.txt). The picker uses the font Manrope (SIL
Open Font License).
