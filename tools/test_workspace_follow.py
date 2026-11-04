"""Integration test against an isolated named Herdr session (never the user's).

Run after cargo build --release: python tools/test_tab_notes.py
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid

root = Path(__file__).resolve().parents[1]
binary = root / "target/release" / ("herdr-notes.exe" if os.name == "nt" else "herdr-notes")
herdr = shutil.which("herdr")
assert herdr and binary.exists(), "Build the plugin and put herdr on PATH first"
session = "notes-test-" + uuid.uuid4().hex[:10]
env = os.environ.copy()
for key in ("HERDR_SOCKET_PATH", "HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID"):
    env.pop(key, None)
prefix = [herdr, "--session", session]


def cli(*args):
    result = subprocess.run(prefix + list(args), env=env, capture_output=True, text=True, timeout=10)
    assert result.returncode == 0, result.stderr
    return json.loads(result.stdout)["result"] if result.stdout.strip() else {}


def panes():
    return cli("pane", "list")["panes"]


def wait_for(predicate, description):
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError(description)


class TestDirectory(tempfile.TemporaryDirectory):
    def cleanup(self):
        # Windows child shells may release inherited log handles just after
        # the isolated server exits.
        for attempt in range(50):
            try:
                return super().cleanup()
            except PermissionError:
                if attempt == 49:
                    raise
                time.sleep(0.1)


with TestDirectory(prefix="herdr-notes-test-") as directory:
    # Isolate the plugin registry too: globally installed startup hooks must
    # never run against real note files during this test.
    env["XDG_CONFIG_HOME"] = str(Path(directory) / "config-home")
    env["XDG_STATE_HOME"] = str(Path(directory) / "state-home")
    env.pop("HERDR_CONFIG_PATH", None)
    state = Path(env["XDG_STATE_HOME"]) / "herdr/plugins/herdr-notes"
    state.mkdir(parents=True)
    config_dir = Path(env["XDG_CONFIG_HOME"]) / "herdr/plugins/config/herdr-notes"
    config_dir.mkdir(parents=True)
    settings = config_dir / "config.json"
    def configure(auto_open=True, follow_tabs=True):
        settings.write_text(json.dumps({"auto_open": auto_open, "follow_tabs": follow_tabs,
                                       "width_columns": 50}))
    configure(auto_open=False)
    # Directly exercise the same one-shot binary used by startup/event hooks.
    # Use only this test server's socket and temporary settings/note directories.
    base_config = Path(env["XDG_CONFIG_HOME"])
    auto_env = env | {"HERDR_ENV": "1", "HERDR_BIN_PATH": herdr,
                      "HERDR_SOCKET_PATH": str(base_config / "herdr/sessions" / session / "herdr.sock"),
                      "HERDR_PLUGIN_STATE_DIR": str(state), "HERDR_PLUGIN_CONFIG_DIR": str(config_dir)}
    def auto_open():
        result = subprocess.run([str(binary), "--auto-open"], env=auto_env,
                                capture_output=True, text=True, timeout=20)
        assert result.returncode == 0, result.stderr
    cli("plugin", "link", str(root))
    with (Path(directory) / "server.log").open("w") as log:
        server = subprocess.Popen(prefix + ["server"], env=env, stdout=log, stderr=log,
                                  creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        try:
            def ready():
                try:
                    panes()
                    return True
                except (AssertionError, json.JSONDecodeError):
                    return False
            wait_for(ready, "Isolated server did not start")
            created = cli("workspace", "create", "--label", "notes-test", "--cwd", directory, "--focus")
            workspace = created["workspace"]["workspace_id"]
            first = created["tab"]["tab_id"]
            original = created["root_pane"]["pane_id"]
            def key(tab):
                return "tab" + tab.lower().encode().hex()
            def notes_in(tab):
                return [p for p in panes() if p["tab_id"] == tab and "herdr-notes" in p.get("tokens", {})]
            def wait_notes(tab):
                wait_for(lambda: len(notes_in(tab)) == 1, "Missing tab Notes: " + tab)
                pane = notes_in(tab)[0]
                token = pane["tokens"]["herdr-notes"]
                wait_for(lambda: notes_in(tab)[0]["tokens"]["herdr-notes"] != token, "Editor heartbeat missing")
                return pane["pane_id"]
            def layout(pane):
                return cli("pane", "layout", "--pane", pane)["layout"]
            def rects(pane):
                return {p["pane_id"]: p["rect"] for p in layout(pane)["panes"]}
            def width(pane):
                return rects(pane)[pane]["width"]
            def saved(tab):
                path = state / (key(tab) + ".json")
                return json.loads(path.read_text())["text"] if path.exists() else None
            def toggle():
                cli("plugin", "action", "invoke", "herdr-notes.open-notes-windows" if os.name == "nt" else "herdr-notes.open-notes")
            auto_open()
            assert not notes_in(first), "Disabled auto-open opened Notes"
            old = state / f"{workspace}.json"
            old.write_text(json.dumps({"text":"workspace backup", "mode":"preview"}))
            configure()
            auto_open()
            notes_a = wait_notes(first)
            assert saved(first) == "workspace backup"
            cli("pane", "send-keys", notes_a, "e")
            cli("pane", "send-text", notes_a, "TAB A: ")
            second = cli("tab", "create", "--workspace", workspace, "--cwd", directory, "--focus")
            target = second["tab"]["tab_id"]
            notes_b = wait_notes(target)
            assert notes_a != notes_b
            assert next(p for p in panes() if p["pane_id"] == notes_a)["tab_id"] == first
            cli("pane", "send-keys", notes_b, "e")
            cli("pane", "send-text", notes_b, "TAB B only")
            wait_for(lambda: saved(first) == "TAB A: workspace backup" and saved(target) == "TAB B only", "Tab edits were not isolated")
            cli("pane", "resize", "--pane", notes_a, "--direction", "left", "--amount", "0.1")
            a_width = width(notes_a)
            b_width = width(notes_b)
            assert a_width != b_width
            width_file = state / ("layout-" + key(first) + ".json")
            wait_for(lambda: width_file.exists() and json.loads(width_file.read_text())["width_columns"] == a_width, "Tab width not saved")
            a_rects, b_rects = rects(notes_a), rects(notes_b)
            for _ in range(3):
                cli("tab", "focus", first)
                time.sleep(0.6)
                cli("tab", "focus", target)
                time.sleep(0.6)
                assert rects(notes_a) == a_rects and rects(notes_b) == b_rects, "Tab switch resized terminals"
            auto_open()
            assert len(notes_in(first)) == len(notes_in(target)) == 1
            # Toggle the second tab from its CLI: the first note remains open.
            anchor_b = next(p["pane_id"] for p in panes() if p["tab_id"] == target and p["pane_id"] != notes_b)
            cli("pane", "zoom", anchor_b, "--on")
            cli("pane", "zoom", anchor_b, "--off")
            toggle()
            wait_for(lambda: not notes_in(target), "Toggle did not close this tab")
            third = cli("tab", "create", "--workspace", workspace, "--cwd", directory, "--focus")["tab"]["tab_id"]
            wait_notes(third)
            assert len(notes_in(first)) == 1 and not notes_in(target), "Creation hook reopened a closed tab"
            cli("tab", "focus", target)
            toggle()
            notes_b = wait_notes(target)
            assert saved(target) == "TAB B only" and width(notes_b) == b_width
            # Close again and verify the per-tab preference survives restart.
            toggle()
            wait_for(lambda: not notes_in(target), "Focused toggle failed")
            cli("pane", "send-keys", notes_a, "Escape")
            assert json.loads(old.read_text())["text"] == "workspace backup"
            cli("server", "stop")
            server.wait(timeout=10)
            server = subprocess.Popen(prefix + ["server"], env=env, stdout=log, stderr=log,
                                      creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            wait_for(ready, "Isolated server did not restart")
            restored = wait_notes(first)
            wait_notes(third)
            assert not notes_in(target)
            assert saved(first) == "TAB A: workspace backup" and saved(target) == "TAB B only"
            assert abs(width(restored) - a_width) <= 1
            assert json.loads(old.read_text())["text"] == "workspace backup"
            print("PASS: per-tab auto-open, independent notes and widths, stable layouts during tab switches, toggle isolation, migration backup and restart")
        finally:
            # This server was created by this test; never stop the default session.
            subprocess.run(prefix + ["server", "stop"], env=env, capture_output=True, timeout=10)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.terminate()
                server.wait(timeout=10)
