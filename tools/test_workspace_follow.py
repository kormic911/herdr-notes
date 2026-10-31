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
    state = Path(directory) / "state"
    state.mkdir()
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
            split = cli("pane", "split", original, "--direction", "right", "--no-focus",
                        "--env", f"HERDR_PLUGIN_STATE_DIR={state}")
            notes = split["pane"]["pane_id"]
            command = ("& '" + str(binary).replace("'", "''") + "'; exit") if os.name == "nt" else (
                "exec " + __import__("shlex").quote(str(binary)))
            # Wait until the shell is ready to accept a command.
            time.sleep(1)
            cli("pane", "run", notes, command)
            wait_for(lambda: any(p["pane_id"] == notes and "herdr-notes" in p.get("tokens", {})
                                 for p in panes()), "Notes did not start")
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
            time.sleep(1)
            own = next(p for p in panes() if p["pane_id"] == notes)
            assert own["workspace_id"] == workspace and own["tab_id"] == first
            assert next(p for p in panes() if p["focused"])["workspace_id"] == other["workspace"]["workspace_id"]
            cli("tab", "focus", target)
            wait_for(lambda: next(p for p in panes() if p["pane_id"] == notes)["tab_id"] == target,
                     "Notes did not follow when returning to its workspace")
            cli("pane", "send-keys", notes, "Escape", "q")
            wait_for(lambda: all(p["pane_id"] != notes for p in panes()), "Notes did not quit")
            saved = json.loads((state / f"{workspace}.json").read_text())
            assert saved["text"] == "unsaved text survives tab switches", saved
            print("PASS: one editor follows tabs, preserves focus and text, stays in its workspace, and saves on quit")
        finally:
            # This server was created by this test; never stop the default session.
            subprocess.run(prefix + ["server", "stop"], env=env, capture_output=True, timeout=10)
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.terminate()
                server.wait(timeout=10)
