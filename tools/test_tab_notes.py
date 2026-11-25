"""Test Notes in an isolated Herdr session after cargo build --release."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/release" / ("herdr-notes.exe" if os.name == "nt" else "herdr-notes")
HERDR = shutil.which("herdr")
assert HERDR and BINARY.exists(), "Build Notes and put Herdr 0.9.5+ on PATH first"


def wait_for(predicate, description):
    deadline = time.monotonic() + 12
    while time.monotonic() < deadline:
        if predicate():
            return
        time.sleep(0.1)
    raise AssertionError(description)


with tempfile.TemporaryDirectory(prefix="herdr-notes-test-") as directory:
    base = Path(directory)
    config = base / "herdr-config.toml"
    env = {
        **os.environ,
        "APPDATA": str(base / "appdata"),
        "LOCALAPPDATA": str(base / "localappdata"),
        "XDG_CONFIG_HOME": str(base / "config"),
        "XDG_STATE_HOME": str(base / "state"),
        "HERDR_CONFIG_PATH": str(config),
    }
    for variable in (
        "HERDR_SOCKET_PATH", "HERDR_PANE_ID", "HERDR_TAB_ID", "HERDR_WORKSPACE_ID",
    ):
        env.pop(variable, None)
    session = f"notes-test-{uuid.uuid4().hex[:8]}"
    args = [HERDR, "--session", session]

    def raw(*command):
        result = subprocess.run(
            args + list(command), env=env, capture_output=True, text=True, timeout=10,
        )
        assert result.returncode == 0, (command, result.stdout, result.stderr)
        return result.stdout

    def cli(*command):
        output = raw(*command)
        return json.loads(output)["result"] if output.strip() else {}

    def panes():
        return cli("pane", "list")["panes"]

    def ready():
        try:
            panes()
        except (AssertionError, json.JSONDecodeError):
            return False
        return True

    def pane(pane_id):
        return cli("pane", "get", pane_id)["pane"]

    def visible(pane_id, text):
        return text in raw("pane", "read", pane_id, "--source", "visible")

    def invoke(action):
        cli("plugin", "action", "invoke", action, "--plugin", "herdr-notes")

    config.write_text("# Existing configuration\n", encoding="utf-8")
    cli("plugin", "link", str(ROOT))
    with (base / "server.log").open("w") as log:
        def start():
            return subprocess.Popen(args + ["server"], env=env, stdout=log, stderr=log)

        server = start()
        try:
            wait_for(ready, "Herdr did not start")
            invoke("install-keybindings")
            wait_for(lambda: "prefix+f4" in config.read_text(), "Missing Notes binding")
            installed = config.read_text()
            assert installed.startswith("# Existing configuration\n")
            wrapper = ROOT / "scripts" / "install-keybindings.sh"
            command = [str(wrapper)]
            if os.name == "nt":
                wrapper = wrapper.with_suffix(".ps1")
                command = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(wrapper)]
            rerun = subprocess.run(
                command, env=env | {"HERDR_SESSION": session, "HERDR_BIN_PATH": HERDR},
                capture_output=True, text=True, timeout=10,
            )
            assert rerun.returncode == 0 and "Notes already uses" in rerun.stdout, rerun
            assert config.read_text() == installed

            created = cli("workspace", "create", "--label", "notes-test", "--cwd", directory, "--focus")
            workspace = created["workspace"]["workspace_id"]
            tab = created["tab"]["tab_id"]
            original = created["root_pane"]["pane_id"]
            host = cli("pane", "split", original, "--direction", "down", "--focus")["pane"]["pane_id"]
            cli("pane", "rename", host, "Scratch")
            state = (
                Path(env["LOCALAPPDATA"]) if os.name == "nt" else Path(env["XDG_STATE_HOME"])
            ) / "herdr/plugins/herdr-notes"
            state.mkdir(parents=True, exist_ok=True)
            note_key = tab.lower() if os.name == "nt" else tab
            note = state / f"tab{note_key.encode().hex()}.json"
            note.write_text(json.dumps({"text": "Existing tab note", "mode": "preview"}))
            invoke("open-notes")
            wait_for(lambda: visible(host, "Existing tab note"), "Existing tab note did not load")
            assert pane(host)["label"] == "Notes"
            assert {p["pane_id"] for p in panes() if p["tab_id"] == tab} == {original, host}

            other = cli("tab", "create", "--workspace", workspace, "--cwd", directory, "--focus")
            other_tab = other["tab"]["tab_id"]
            old = state / f"{workspace}.json"
            old.write_text(json.dumps({"text": "Legacy workspace note", "mode": "preview"}))
            other_pane = next(p["pane_id"] for p in panes() if p["tab_id"] == other_tab)
            invoke("open-notes")
            wait_for(lambda: visible(other_pane, "Legacy workspace note"), "Old workspace note did not migrate")
            assert json.loads(old.read_text())["text"] == "Legacy workspace note"
            cli("pane", "send-keys", other_pane, "q")
            wait_for(lambda: pane(other_pane).get("label") != "Notes", "Quit did not return to the shell")

            cli("server", "stop")
            server.wait(timeout=10)
            server = start()
            wait_for(ready, "Herdr did not restart")
            wait_for(lambda: visible(host, "Existing tab note"), "Notes did not restore in its pane")
            assert pane(other_pane).get("label") != "Notes"
            assert config.read_text() == installed

            cli("pane", "send-keys", host, "q")
            wait_for(lambda: pane(host).get("label") == "Scratch", "Original pane label was not restored")
            assert json.loads(note.read_text())["text"] == "Existing tab note"
            print("PASS: binding installation, selected pane, note reuse, migration, restart, and quit")
        finally:
            if server.poll() is None:
                subprocess.run(args + ["server", "stop"], env=env, capture_output=True, timeout=10)
                server.wait(timeout=10)
