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

/*
 * The app's own settings window (src-tauri/src/settings.rs): TV mode and the normal window,
 * starting in the TV mode at login, the player, the server, quitting. Everything here is stored
 * in the app on this PC. With a remote control: arrows move between the controls, OK (Enter)
 * presses or ticks, Back (Esc, Backspace) closes the window.
 */
(() => {
    "use strict";
    const {invoke} = window.__TAURI__.core;
    const $ = (id) => document.getElementById(id);

    const DE = {
        "Settings": "Einstellungen",
        "For this PC – stored in the app, not on the server.": "Für diesen PC – in der App gespeichert, nicht auf dem Server.",
        "Display": "Darstellung",
        "Switch to TV mode": "Zum TV-Modus wechseln",
        "Back to the normal window": "Zurück zum normalen Fenster",
        "Start in TV mode at login": "Bei der Anmeldung im TV-Modus starten",
        "FreeX is in the TV mode: the interface of the FreeX TV apps in full screen, for a remote control.":
            "FreeX ist im TV-Modus: die Oberfläche der FreeX-TV-Apps im Vollbild, für die Fernbedienung.",
        "FreeX shows FreeX Web in a normal window. The TV mode shows the interface of the FreeX TV apps in full screen, for a remote control (also Ctrl+Shift+T).":
            "FreeX zeigt FreeX Web in einem normalen Fenster. Der TV-Modus zeigt die Oberfläche der FreeX-TV-Apps im Vollbild, für die Fernbedienung (auch Strg+Umschalt+T).",
        "Player": "Player",
        "FreeX player": "FreeX-Player",
        "The player of FreeX Web, with skip intro, up next and Watch Together.":
            "Der Player von FreeX Web, mit Intro überspringen, nächster Folge und Watch Together.",
        "mpv plays every file directly, without conversion on the server.":
            "mpv spielt jede Datei direkt ab, ohne Umwandlung auf dem Server.",
        "mpv was not found. Install mpv to use it.": "mpv wurde nicht gefunden. Installiere mpv, um es zu nutzen.",
        "Play inside the FreeX window": "Im FreeX-Fenster abspielen",
        "Start mpv in full screen": "mpv im Vollbild starten",
        "The TV mode always plays with mpv.": "Der TV-Modus spielt immer mit mpv ab.",
        "Server": "Server",
        "Connected to {0} ({1}).": "Verbunden mit {0} ({1}).",
        "Last server: {0} ({1}).": "Letzter Server: {0} ({1}).",
        "No server yet.": "Noch kein Server.",
        "Switch server": "Server wechseln",
        "Connect to the last server on start": "Beim Start mit dem letzten Server verbinden",
        "Quit FreeX": "FreeX beenden",
        "Close": "Schließen",
    };
    const german = (navigator.language || "").toLowerCase().startsWith("de");
    document.documentElement.lang = german ? "de" : "en";
    const t = (text, ...args) => (german && DE[text] || text).replace(/\{(\d)}/g, (_, i) => args[i]);
    document.querySelectorAll("[data-t]").forEach((e) => {
        e.textContent = t(e.dataset.t);
    });

    let info = null;

    function showError(message) {
        $("error").textContent = message;
        $("error").hidden = !message;
    }

    const close = () => window.__TAURI__.window.getCurrentWindow().close();

    /** Runs an app command; problems show at the top instead of failing silently. */
    async function run(cmd, args) {
        showError("");
        try {
            return await invoke(cmd, args);
        } catch (e) {
            showError(String(e));
            throw e;
        }
    }

    // ------------------------------------------------------------------ display (TV mode)

    function renderTv() {
        $("tv-section").hidden = !info.tvAvailable;
        if (!info.tvAvailable) return;
        $("tv-state").textContent = info.tvActive
            ? t("FreeX is in the TV mode: the interface of the FreeX TV apps in full screen, for a remote control.")
            : t("FreeX shows FreeX Web in a normal window. The TV mode shows the interface of the FreeX TV apps in full screen, for a remote control (also Ctrl+Shift+T).");
        $("tv-enter").hidden = info.tvActive;
        $("tv-leave").hidden = !info.tvActive;
        invoke("autostart_get").then((on) => {
            $("tv-autostart").checked = !!on;
        }).catch(() => {
            $("tv-autostart").closest("label").hidden = true;
        });
    }

    $("tv-enter").addEventListener("click", async () => {
        await run("tv_enter").catch(() => null);
        if (!$("error").textContent) close();
    });
    $("tv-leave").addEventListener("click", async () => {
        await run("tv_leave").catch(() => null);
        if (!$("error").textContent) close();
    });
    $("tv-autostart").addEventListener("change", async (e) => {
        try {
            e.target.checked = await run("autostart_set", {on: e.target.checked});
        } catch (err) {
            e.target.checked = !e.target.checked;
        }
    });

    // ------------------------------------------------------------------ player

    function renderPlayer() {
        const mpv = info.player === "mpv";
        for (const b of $("player").querySelectorAll("button")) {
            b.setAttribute("aria-checked", String(b.dataset.player === info.player));
            b.disabled = b.dataset.player === "mpv" && !info.mpvAvailable && !mpv;
        }
        const hints = [];
        if (!info.mpvAvailable) hints.push(t("mpv was not found. Install mpv to use it."));
        else hints.push(mpv ? t("mpv plays every file directly, without conversion on the server.")
            : t("The player of FreeX Web, with skip intro, up next and Watch Together."));
        if (info.tvAvailable) hints.push(t("The TV mode always plays with mpv."));
        $("player-hint").textContent = hints.join(" ");
        document.querySelectorAll(".mpv-only").forEach((e) => {
            e.hidden = !mpv;
        });
        $("embed-row").hidden = !mpv || !info.mpvCanEmbed;
        $("mpv-embed").checked = info.mpvEmbed;
        $("mpv-fullscreen").checked = info.mpvFullscreen;
    }

    async function savePlayer(changes) {
        Object.assign(info, changes);
        renderPlayer();
        await run("set_player", {player: info.player, mpvFullscreen: info.mpvFullscreen, mpvEmbed: info.mpvEmbed})
            .catch(() => null);
    }

    for (const b of $("player").querySelectorAll("button")) {
        b.addEventListener("click", () => savePlayer({player: b.dataset.player}));
    }
    $("mpv-embed").addEventListener("change", (e) => savePlayer({mpvEmbed: e.target.checked}));
    $("mpv-fullscreen").addEventListener("change", (e) => savePlayer({mpvFullscreen: e.target.checked}));

    // ------------------------------------------------------------------ server

    async function renderServer() {
        const s = info.openServer;
        $("server").textContent = !s ? t("No server yet.")
            : info.tvActive ? t("Last server: {0} ({1}).", s.name, s.url) : t("Connected to {0} ({1}).", s.name, s.url);
        try {
            const list = await invoke("servers_list");
            $("auto-connect").checked = !!list.autoConnect;
        } catch (e) {
            $("auto-connect").closest("label").hidden = true;
        }
    }

    $("servers").addEventListener("click", async () => {
        await run("show_servers").catch(() => null);
        if (!$("error").textContent) close();
    });
    $("auto-connect").addEventListener("change", (e) => run("set_auto_connect", {enabled: e.target.checked}).catch(() => null));

    // ------------------------------------------------------------------ quit, close

    $("quit").addEventListener("click", () => run("app_quit").catch(() => null));
    $("close").addEventListener("click", close);

    // ------------------------------------------------------------------ remote control

    /** The controls a remote moves between, in reading order. */
    function controls() {
        return [...document.querySelectorAll("button, input")]
            .filter((e) => !e.disabled && e.offsetParent !== null && !e.closest("[hidden]"));
    }

    document.addEventListener("keydown", (e) => {
        const list = controls();
        const at = list.indexOf(document.activeElement);
        const move = (step) => {
            e.preventDefault();
            const next = at < 0 ? (step > 0 ? 0 : list.length - 1) : Math.min(list.length - 1, Math.max(0, at + step));
            if (list[next]) list[next].focus();
        };
        switch (e.key) {
            case "ArrowDown":
            case "ArrowRight":
                move(1);
                break;
            case "ArrowUp":
            case "ArrowLeft":
                move(-1);
                break;
            case "Enter":
                // OK on a checkbox ticks it, like Space.
                if (document.activeElement && document.activeElement.type === "checkbox") {
                    e.preventDefault();
                    document.activeElement.click();
                }
                break;
            case "Escape":
            case "Backspace":
            case "BrowserBack":
            case "GoBack":
                e.preventDefault();
                close();
                break;
        }
    });

    // ------------------------------------------------------------------ start

    (async () => {
        info = await invoke("desktop_info");
        $("version").textContent = `${info.app} ${info.version}`;
        renderTv();
        renderPlayer();
        await renderServer();
        const first = controls()[0];
        if (first) first.focus();
    })().catch((e) => showError(String(e)));
})();
