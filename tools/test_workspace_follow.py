"""Integration test against an isolated named Herdr session (never the user's).

Run after cargo build --release: python tools/test_workspace_follow.py
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


with tempfile.TemporaryDirectory(prefix="herdr-notes-test-") as directory:
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
            initial_count = len(panes())
            auto_open()
            assert len(panes()) == initial_count, "Disabled auto-open changed the layout"
            configure()
            auto_open()
            notes = next(p["pane_id"] for p in panes() if "herdr-notes" in p.get("tokens", {}))
            wait_for(lambda: any(p["pane_id"] == notes and "herdr-notes" in p.get("tokens", {})
                                 for p in panes()), "Notes did not start")
            # The launcher stamps before the process starts. Wait for the TUI.
            time.sleep(0.5)
            auto_open()
            assert len([p for p in panes() if "herdr-notes" in p.get("tokens", {})]) == 1
            assert next(p for p in panes() if p["focused"])["pane_id"] == original
            cli("pane", "send-keys", notes, "e")
            cli("pane", "send-text", notes, "unsaved text survives tab switches")
            second = cli("tab", "create", "--workspace", workspace, "--cwd", directory, "--focus")
            target = second["tab"]["tab_id"]
            wait_for(lambda: next(p for p in panes() if p["pane_id"] == notes)["tab_id"] == target,
                     "Notes did not follow the new tab")
            assert not next(p for p in panes() if p["pane_id"] == notes)["focused"], "Notes stole focus"
            cli("tab", "focus", first)
            wait_for(lambda: next(p for p in panes() if p["pane_id"] == notes)["tab_id"] == first,
                     "Notes did not return to the first tab")
            other = cli("workspace", "create", "--label", "other-test", "--cwd", directory, "--focus")
            wait_for(lambda: len([p for p in panes() if "herdr-notes" in p.get("tokens", {})]) == 2,
                     "workspace.created hook did not auto-open Notes")
            auto_open()
            assert len([p for p in panes() if "herdr-notes" in p.get("tokens", {})]) == 2
            time.sleep(1)
            own = next(p for p in panes() if p["pane_id"] == notes)
            assert own["workspace_id"] == workspace and own["tab_id"] == first
            assert next(p for p in panes() if p["focused"])["workspace_id"] == other["workspace"]["workspace_id"]
            configure(follow_tabs=False)
            time.sleep(0.6)
            cli("tab", "focus", target)
            time.sleep(1)
            assert next(p for p in panes() if p["pane_id"] == notes)["tab_id"] == first
            configure(follow_tabs=True)
            wait_for(lambda: next(p for p in panes() if p["pane_id"] == notes)["tab_id"] == target,
                     "Notes did not follow when returning to its workspace")
            cli("pane", "send-keys", notes, "Escape", "q")
            wait_for(lambda: all(p["pane_id"] != notes for p in panes()), "Notes did not quit")
            saved = json.loads((state / f"{workspace}.json").read_text())
            assert saved["text"] == "unsaved text survives tab switches", saved
            # A restored Notes shell has its label but no live identity token.
            corpse = cli("pane", "split", original, "--direction", "right", "--no-focus")["pane"]["pane_id"]
            cli("pane", "rename", corpse, "Notes")
            auto_open()
            assert all(p["pane_id"] != corpse for p in panes())
            assert len([p for p in panes() if p["workspace_id"] == workspace and
                        "herdr-notes" in p.get("tokens", {})]) == 1
            # Real server restart exercises the manifest startup hook and
            # recovery of restored Notes shells, without touching any user session.
            cli("server", "stop")
            server.wait(timeout=10)
            server = subprocess.Popen(prefix + ["server"], env=env, stdout=log, stderr=log,
                                      creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            wait_for(ready, "Isolated server did not restart")
            wait_for(lambda: len([p for p in panes() if "herdr-notes" in p.get("tokens", {})]) == 2,
                     "Startup hook did not restore both workspace editors")
            print("PASS: configurable auto-open, idempotence, restored shells, tab following, focus, workspace isolation and saved text")
        finally:
            # This server was created by this test; never stop the default session.
            subprocess.run(prefix + ["server", "stop"], env=env, capture_output=True, timeout=10)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.terminate()
                server.wait(timeout=10)
