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
 * The bridge of the TV mode (tv.rs): window.FreeXNative for the FreeX TV interface, with the
 * calls of the FreeXHT bridge (platform "freexht", shell "desktop"), so the interface drives mpv
 * as it does on the FreeX box. Commands go to the app through Tauri's IPC; mpv's events come
 * back as the event "freex-tv" to window.FreeXNativeEvent. What the page asks for synchronously
 * (capabilities, device name) comes with this script.
 */
(() => {
    "use strict";
    if (window.FreeXNative || window.top !== window) return;

    const INFO = __FREEX_TV_INFO__;
    // For native.js: this window is the TV mode (Esc is the remote's Back here).
    window.__FREEX_TV__ = true;

    const tauri = () => window.__TAURI__;
    const invoke = (cmd, args) => {
        const t = tauri();
        return t && t.core ? t.core.invoke(cmd, args) : Promise.reject(new Error("no app bridge"));
    };
    const player = (command, args) => {
        invoke("tv_player", {command, args: args || null}).catch((e) => {
            if (command === "open" && typeof window.FreeXNativeEvent === "function") {
                window.FreeXNativeEvent({type: "error", message: String(e)});
            }
        });
    };

    window.FreeXNative = {
        platform: "freexht",
        open: (json) => player("open", JSON.parse(json)),
        play: () => player("play"),
        pause: () => player("pause"),
        seek: (ms) => player("seek", {ms: Number(ms) || 0}),
        stop: () => player("stop"),
        exit: () => player("exit"),
        deviceName: () => INFO.deviceName,
        capabilities: () => JSON.stringify(INFO.caps),
        info: () => INFO,
        defaultServer: () => INFO.server || "",
        call: (method, args) => invoke("tv_call", {method, args: args || {}}),
    };

    function listen() {
        const t = tauri();
        if (!t || !t.event) return setTimeout(listen, 50);
        t.event.listen("freex-tv", (e) => {
            if (typeof window.FreeXNativeEvent === "function") window.FreeXNativeEvent(e.payload);
        });
    }
    listen();
})();
