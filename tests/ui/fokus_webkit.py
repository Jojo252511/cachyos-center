#!/usr/bin/env python3
"""Keyboard focus checks in WebKitGTK, the engine of the app on Linux.

Loads the browser preview of the interface (mock data) in a WebKitGTK 4.1
view on a private Xvfb display, presses real keys via XTest and checks where
the focus goes when a state change removes or disables the focused control.
Never touches the user's session: the display is started by this script.

Usage (from the repository root):
    python3 tests/ui/fokus_webkit.py              # starts Xvfb and the Vite dev server
    python3 tests/ui/fokus_webkit.py --url http://127.0.0.1:1420/   # running dev server

Requires python-gobject, webkit2gtk-4.1, xorg-server-xvfb, libxtst and the
npm dependencies of apps/desktop. Exit code 1 when a check fails.
"""
import argparse
import ctypes
import json
import os
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

LOGGER = r"""(() => {
  window.__f = [];
  const t0 = performance.now();
  const d = (el) => el ? el.tagName + (el.id ? '#' + el.id : '') + '[' + (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 28) + ']' : 'null';
  for (const type of ['focusin', 'focusout']) {
    document.addEventListener(type, (e) => __f.push(Math.round(performance.now() - t0) + 'ms ' + type + ' ' + d(e.target) + ' -> ' + d(e.relatedTarget)), true);
  }
  window.__a = () => d(document.activeElement);
  window.__button = (text) => [...document.querySelectorAll('button')].find((b) => b.textContent.trim() === text);
  return true;
})()"""

# Steps: ("until", expr, timeout) waits, ("do", expr) acts, ("key", names) presses real keys,
# ("wait", seconds), ("expect", expr, description) checks.
SCENARIOS = [
    {
        "name": "Updates: erste Prüfung aus dem Leerzustand",
        "query": "scenario=neverChecked#/updates",
        "steps": [
            ("until", "document.querySelector('.state-view--empty button')", 20),
            ("do", "document.querySelector('.state-view--empty button').focus() ?? true"),
            ("key", ["Return"]),
            ("wait", 0.6),
            ("expect", "document.activeElement.id === 'updates-check' && document.activeElement.getAttribute('aria-busy') === 'true'",
             "während der Prüfung: Fokus auf dem Kopf-Button „Prüfung läuft …“"),
            ("until", "document.getElementById('updates-check').getAttribute('aria-busy') === null", 15),
            ("wait", 0.3),
            ("expect", "document.activeElement.id === 'updates-check'", "nach der Prüfung: Fokus auf „Jetzt prüfen“"),
            ("expect", "!__f.some((e) => e.includes('focusin H1#page-title'))", "kein Umweg über die Seitenüberschrift"),
        ],
    },
    {
        "name": "Übersicht: erste Prüfung findet Updates",
        "query": "scenario=neverChecked#/",
        "steps": [
            ("until", "document.querySelector('.page-header__actions button')", 20),
            ("do", "document.querySelector('.page-header__actions button').focus() ?? true"),
            ("key", ["Return"]),
            ("wait", 0.6),
            ("expect", "document.activeElement.matches('.page-header__actions button[aria-busy=true]')",
             "während der Prüfung: Fokus auf „Prüfung läuft …“"),
            ("until", "document.querySelector('.page-header__actions a')", 15),
            ("wait", 0.3),
            ("expect", "document.activeElement === document.querySelector('.page-header__actions a')",
             "nach der Prüfung: Fokus auf „Updates ansehen“ (der Button wurde durch einen Link ersetzt)"),
        ],
    },
    {
        "name": "Einstellungen: „Übernehmen“ wird nach dem Speichern deaktiviert",
        "query": "scenario=default#/settings",
        "steps": [
            ("until", "__button('Übernehmen')", 20),
            ("do", "document.getElementById([...document.querySelectorAll('label.radio-card__label')]"
                   ".find((l) => l.textContent.includes('Nur benachrichtigen')).htmlFor).focus() ?? true"),
            ("key", ["space"]),
            ("wait", 0.3),
            ("do", "__button('Übernehmen').focus() ?? !__button('Übernehmen').disabled"),
            ("key", ["Return"]),
            ("wait", 0.4),
            ("expect", "document.activeElement.getAttribute('aria-busy') === 'true'", "während des Speicherns: Fokus auf „Wird übernommen …“"),
            ("until", "document.body.textContent.includes('Richtlinie übernommen.')", 10),
            ("wait", 1.0),
            ("expect", "document.activeElement.matches('.apply-row [role=status]')",
             "nach dem Speichern: Fokus auf „Richtlinie übernommen.“"),
        ],
    },
    {
        "name": "Vorabprüfungen: News als gelesen markieren",
        "query": "scenario=newsUnread#/updates",
        "steps": [
            ("until", "__button('Als gelesen markieren')", 20),
            ("do", "__button('Als gelesen markieren').focus() ?? true"),
            ("key", ["Return"]),
            ("until", "!__button('Als gelesen markieren')", 10),
            ("wait", 0.4),
            ("expect", "document.activeElement.tagName === 'P' && document.activeElement.textContent.includes('Keine ungelesenen Meldungen')",
             "Fokus auf der News-Statuszeile"),
            ("expect", "!__f.some((e) => e.includes('focusin H1#page-title'))", "kein Umweg über die Seitenüberschrift"),
        ],
    },
    {
        "name": "Upgrade per Tastatur starten und abbrechen",
        "query": "scenario=default#/updates",
        "steps": [
            ("until", "__button('Installieren') && !__button('Installieren').disabled", 20),
            ("do", "__button('Installieren').focus() ?? true"),
            ("key", ["Return"]),
            ("until", "[...document.querySelectorAll('[role=dialog] button')].some((b) => b.textContent.includes('Upgrade starten') && !b.disabled)", 10),
            ("do", "[...document.querySelectorAll('[role=dialog] button')].find((b) => b.textContent.includes('Upgrade starten')).focus() ?? true"),
            ("key", ["Return"]),
            ("until", "!document.querySelector('[role=dialog]') && document.querySelector('#operation-panel .operation-panel__cancel button')", 15),
            ("wait", 0.3),
            ("expect", "__button('Installieren').disabled && document.activeElement.id === 'updates-check'",
             "nach dem Start: „Installieren“ gesperrt, Fokus auf „Jetzt prüfen“"),
            ("do", "(window.__mark = __f.length, document.querySelector('#operation-panel .operation-panel__cancel button').focus()) ?? true"),
            ("key", ["Return"]),
            ("until", "!document.querySelector('#operation-panel .operation-panel__cancel button')", 15),
            ("wait", 0.4),
            ("expect", "document.activeElement.id === 'operation-panel-title'", "nach dem Abbrechen: Fokus auf dem Titel des Vorgangs"),
            ("expect", "!__f.slice(__mark).some((e) => e.includes('focusin H1#page-title'))", "kein Umweg über die Seitenüberschrift"),
        ],
    },
]


def free_display() -> int:
    for number in range(90, 120):
        if not Path(f"/tmp/.X11-unix/X{number}").exists() and not Path(f"/tmp/.X{number}-lock").exists():
            return number
    sys.exit("no free X display number")


def wait_http(url: str, timeout: float) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=2):
                return
        except OSError:
            time.sleep(0.3)
    sys.exit(f"dev server not reachable: {url}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--url", help="running dev server (default: start one on 127.0.0.1:1421)")
    args = parser.parse_args()

    children: list[subprocess.Popen] = []
    try:
        number = free_display()
        children.append(subprocess.Popen(["Xvfb", f":{number}", "-screen", "0", "1280x860x24", "-nolisten", "tcp"],
                                         stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
        deadline = time.monotonic() + 10
        while not Path(f"/tmp/.X11-unix/X{number}").exists():
            if time.monotonic() > deadline:
                sys.exit("Xvfb did not start")
            time.sleep(0.1)
        url = args.url
        if not url:
            url = "http://127.0.0.1:1421/"
            children.append(subprocess.Popen(["npx", "vite", "--host", "127.0.0.1", "--port", "1421", "--strictPort"],
                                             cwd=ROOT / "apps/desktop", stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL))
        wait_http(url, 30)
        os.environ.update(DISPLAY=f":{number}", GDK_BACKEND="x11")
        os.environ.pop("WAYLAND_DISPLAY", None)
        return run(url.rstrip("/") + "/")
    finally:
        for child in reversed(children):
            child.terminate()
            child.wait(timeout=10)


def run(base: str) -> int:
    import gi

    gi.require_version("Gtk", "3.0")
    gi.require_version("WebKit2", "4.1")
    from gi.repository import Gtk, WebKit2

    x11 = ctypes.CDLL("libX11.so.6")
    xtst = ctypes.CDLL("libXtst.so.6")
    x11.XOpenDisplay.restype = ctypes.c_void_p
    x11.XOpenDisplay.argtypes = [ctypes.c_char_p]
    x11.XFlush.argtypes = [ctypes.c_void_p]
    x11.XStringToKeysym.restype = ctypes.c_ulong
    x11.XStringToKeysym.argtypes = [ctypes.c_char_p]
    x11.XKeysymToKeycode.restype = ctypes.c_ubyte
    x11.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
    xtst.XTestFakeMotionEvent.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong]
    xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
    display = x11.XOpenDisplay(None)

    window = Gtk.Window(title="cachyos-center focus check")
    window.set_default_size(1200, 820)
    window.move(0, 0)
    view = WebKit2.WebView()
    window.add(view)
    window.show_all()
    print(f"WebKitGTK {WebKit2.get_major_version()}.{WebKit2.get_minor_version()}.{WebKit2.get_micro_version()}", flush=True)

    class Aborted(Exception):
        pass

    # Never block: a hanging or crashed web process ends the run with an error.
    crashed: list[str] = []
    view.connect("web-process-terminated", lambda _view, reason: crashed.append(str(reason)))
    overall = time.monotonic() + 300

    def iterate_until(done, timeout: float) -> bool:
        deadline = min(time.monotonic() + timeout, overall)
        while not done():
            if crashed:
                raise Aborted(f"web process terminated ({crashed[0]})")
            if time.monotonic() > deadline:
                if deadline == overall:
                    raise Aborted("overall time limit reached")
                return False
            while Gtk.events_pending():
                Gtk.main_iteration_do(False)
            time.sleep(0.005)
        return True

    def press(names: list[str]) -> None:
        # Without a window manager the keyboard focus follows the pointer.
        xtst.XTestFakeMotionEvent(display, -1, 600, 400, 0)
        for name in names:
            code = x11.XKeysymToKeycode(display, x11.XStringToKeysym(name.encode()))
            xtst.XTestFakeKeyEvent(display, code, 1, 0)
            xtst.XTestFakeKeyEvent(display, code, 0, 0)
        x11.XFlush(display)

    def evaluate(expr: str, boolean: bool = True):
        body = f"Boolean({expr})" if boolean else expr
        script = f"(() => {{ try {{ return JSON.stringify({body}); }} catch (e) {{ return JSON.stringify('error: ' + e); }} }})()"
        result: dict = {}

        def finished(webview, task):
            try:
                result["value"] = json.loads(webview.evaluate_javascript_finish(task).to_string())
            except Exception as error:  # noqa: BLE001
                result["value"] = f"error: {error}"

        view.evaluate_javascript(script, -1, None, None, None, finished)
        if not iterate_until(lambda: "value" in result, 10):
            return "error: no answer from the web process"
        return result["value"]

    def pump(seconds: float) -> None:
        end = time.monotonic() + seconds
        iterate_until(lambda: time.monotonic() >= end, seconds + 1)

    def load(url: str) -> bool:
        loaded = {"done": False}

        def changed(_view, event):
            if event == WebKit2.LoadEvent.FINISHED:
                loaded["done"] = True

        handler = view.connect("load-changed", changed)
        view.load_uri(url)
        ok = iterate_until(lambda: loaded["done"], 30)
        view.disconnect(handler)
        return ok

    failures = 0
    try:
        failures = run_scenarios(base, evaluate, pump, load, press)
    except Aborted as error:
        print(f"\nABBRUCH  {error}")
        failures += 1
    window.destroy()
    print(f"\n{'Alle Prüfungen bestanden' if failures == 0 else f'{failures} Prüfung(en) fehlgeschlagen'}")
    return 0 if failures == 0 else 1


def run_scenarios(base: str, evaluate, pump, load, press) -> int:
    failures = 0
    for index, scenario in enumerate(SCENARIOS):
        print(f"\n{scenario['name']}", flush=True)
        query, _, fragment = scenario["query"].partition("#")
        if not load(f"{base}?{query}&run={index}#{fragment}"):
            print("  FEHLER  Seite nicht geladen")
            failures += 1
            continue
        if evaluate(LOGGER) is not True:
            print("  FEHLER  Fokusprotokoll nicht installiert")
            failures += 1
        for step in scenario["steps"]:
            kind = step[0]
            if kind == "until":
                deadline = time.monotonic() + step[2]
                while evaluate(step[1]) is not True:
                    if time.monotonic() > deadline:
                        print(f"  FEHLER  Zeitüberschreitung: {step[1]}")
                        failures += 1
                        break
                    pump(0.05)
            elif kind == "do":
                value = evaluate(step[1])
                if value is not True:
                    print(f"  FEHLER  Aktion fehlgeschlagen ({value}): {step[1]}")
                    failures += 1
            elif kind == "key":
                press(step[1])
                pump(0.08)
            elif kind == "wait":
                pump(step[1])
            elif kind == "expect":
                ok = evaluate(step[1]) is True
                failures += 0 if ok else 1
                print(f"  {'ok     ' if ok else 'FEHLER '} {step[2]}", flush=True)
        report = evaluate("({active: __a(), focus: __f})", boolean=False)
        if isinstance(report, dict):
            print(f"  Fokusprotokoll: {' | '.join(report['focus'])}")
            print(f"  aktives Element am Ende: {report['active']}")
    return failures


if __name__ == "__main__":
    sys.exit(main())
