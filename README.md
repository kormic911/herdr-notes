# herdr-notes

Persistent Markdown notes for [Herdr](https://herdr.dev), with a separate note
and Notes pane in each tab. Fork of [alexarthurs/herdr-notes](https://github.com/alexarthurs/herdr-notes).

Notes stays in its tab. Switching tabs does not move panes or resize your CLI.
Each tab saves its own note, pane width, and shortcut visibility preference.

## Install

```sh
herdr plugin install kormic911/herdr-notes --yes
```

Requires Herdr 0.9.3 or newer and Git. **Rust is not required for installation.**
The installer downloads the binary matching the plugin version from this fork's
GitHub Releases and verifies its SHA-256 checksum before installing it.

Supported binaries: Windows x64 (also usable on Windows ARM64 via x64 emulation),
Linux x64/ARM64 (static musl builds), and macOS Intel/Apple Silicon. Windows uses
PowerShell 5.1 or later. Linux/macOS use Bash, curl, and sha256sum or shasum.
Downloads require access to GitHub Release assets. A missing release, unsupported
platform, or checksum failure stops installation; it never silently compiles.

For development or unsupported platforms, explicitly opt into a source build:

```powershell
$env:HERDR_NOTES_BUILD_FROM_SOURCE = '1'
herdr plugin install kormic911/herdr-notes --yes
Remove-Item Env:HERDR_NOTES_BUILD_FROM_SOURCE
```

```sh
HERDR_NOTES_BUILD_FROM_SOURCE=1 herdr plugin install kormic911/herdr-notes --yes
```

Source builds require Rust/Cargo and the platform's native build tools. For a
local checkout, `cargo build --release --locked` followed by `herdr plugin link .`
continues to work. Installing an older plugin revision follows that revision's
installation instructions; versions before 0.4.1 still compile during install.

## Open and close

```sh
herdr plugin action invoke herdr-notes.open-notes-windows  # Windows
herdr plugin action invoke herdr-notes.open-notes          # Linux/macOS
```

The action opens Notes in the current tab, or saves and closes that tab's Notes
if it is already open. Other tabs are unaffected.

Add this to your Herdr config for Ctrl+Alt+N (use `herdr-notes.open-notes` on Unix):

```toml
[[keys.command]]
key = "ctrl+alt+n"
type = "plugin_action"
command = "herdr-notes.open-notes-windows"
description = "Tab notes"
```

Reload the Herdr config after changing keybindings.

## Settings

Create `config.json` in the plugin config directory printed by installation.
On Windows this is `%APPDATA%/herdr/plugins/config/herdr-notes/config.json`.

```json
{
  "auto_open": true,
  "remember_width": true,
  "width_columns": 42
}
```

- `auto_open` defaults to `false`. When enabled, Notes opens for existing tabs
  at server startup and for new tabs. It does not steal keyboard focus.
- `remember_width` defaults to `true`. Resizing Notes saves that tab's preferred
  width within about half a second. Zooming or resizing the outer terminal does
  not replace it.
- `width_columns` defaults to `42` and accepts 20–160. It supplies the initial
  width when the tab has no remembered width. Available space may constrain it.

A tab closed using the toggle stays closed, including after restart, until you
open it with the toggle again. The old `follow_tabs` setting is accepted for
compatibility but ignored: Notes never follows tabs.

To apply auto-open to an already-running server:

```sh
herdr plugin action invoke herdr-notes.apply-settings-windows  # Windows
herdr plugin action invoke herdr-notes.apply-settings          # Linux/macOS
```

Opening or closing a split resizes that tab once. Switching between tabs does
not change their layouts. Notes is still a normal split, not a native workspace
sidebar. Manually moving a Notes pane is unsupported; close it and open Notes
in the destination tab instead.

## Editing

Press `e` or Enter to edit; Escape saves and returns to preview. Edits autosave
after two seconds of inactivity. Use arrow keys, Page Up/Down, or `g`/`G` to
scroll in preview. `x` asks before clearing a note; `q` saves and quits.

## Storage and migration

Notes uses Herdr's plugin state directory (`%LOCALAPPDATA%/herdr/plugins/herdr-notes`
on Windows). Files use stable tab IDs encoded as hexadecimal, so tab renames
keep the same note. Note files and width preferences survive plugin updates.
Deleting a tab leaves its saved files available for recovery; a new tab has a
new identity and a new note.

Version 0.4 imports an old workspace note into the first tab opened for that
workspace. The original `<workspace-id>.json` is retained unchanged as a backup;
other tabs start empty. Migration is serialized and happens only once per
workspace. During an installed upgrade, existing open Notes panes can be used
to select which tab receives the old note. Back up the state directory before
manual migrations or changing plugin versions.

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
python tools/test_workspace_follow.py
```

The integration test uses an isolated Herdr session and temporary config/state
roots. It checks independent tab notes and widths, stable pane rectangles when
switching tabs, shortcut isolation, migration, and server restart recovery.

## Publishing binaries

The `Release binaries` workflow builds and tests all five targets on a `v*` tag,
checks that the tag matches both manifests, and publishes binaries with SHA-256
checksum files only after every build succeeds. Set the same version in
`Cargo.toml`, `Cargo.lock`, and `herdr-plugin.toml` before tagging. A manual workflow
run builds artifacts without publishing a release. Release assets must remain
available for users installing that version.
