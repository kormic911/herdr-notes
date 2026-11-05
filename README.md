# herdr-notes

Persistent Markdown notes for [Herdr](https://herdr.dev), with a separate note
and Notes pane in each tab. Fork of [alexarthurs/herdr-notes](https://github.com/alexarthurs/herdr-notes).

Notes stays in its tab. Switching tabs does not move panes or resize your CLI.
Each tab saves its own note, pane width, and whether Notes was closed using the
toggle. Markdown preview, plain-text editing, and automatic saving are included.

## What changed in this fork

Compared with the upstream version this fork started from:

| Behavior | Upstream at the fork point | This fork |
| --- | --- | --- |
| Note ownership | One note per workspace | Independent note per tab |
| Toggle | Open, focus, then close when focused | Open or save and close the current tab's note, regardless of pane focus |
| Automatic opening | Manually opened | Optional startup and new-tab opening |
| Pane width | Initial dock width | Remembered separately for each tab |
| Installation | Compile with Cargo | Download a verified prebuilt binary; source builds are optional |

Earlier releases of this fork moved one workspace editor between tabs. Since
v0.4.0, every Notes pane stays in its own tab. There is no shared live note across
tabs and no movement or terminal resizing caused by switching tabs.

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

### Updating or switching from upstream

Save and close running Notes panes before replacing the plugin, then rerun the
install command above. Existing plugin settings and saved notes are retained.
This fork uses the same `herdr-notes` plugin ID as upstream; installing it replaces
the registered plugin rather than adding a second one. See [migration](#storage-and-migration)
before moving from workspace notes to per-tab notes.

Installing or updating does not configure a shortcut or turn on auto-open.
Configure those below. An already-running editor may continue using its old
executable until closed and reopened.

## Open and close

```sh
herdr plugin action invoke herdr-notes.open-notes-windows  # Windows
herdr plugin action invoke herdr-notes.open-notes          # Linux/macOS
```

The action opens Notes in the current tab, or saves and closes that tab's Notes
if it is already open. Other tabs are unaffected.

### Keyboard shortcut

Ctrl+Alt+N is a suggested binding, not automatically installed. Add this to your
Herdr `config.toml` (normally `%APPDATA%/herdr/config.toml` on Windows):

```toml
[[keys.command]]
key = "ctrl+alt+n"
type = "plugin_action"
command = "herdr-notes.open-notes-windows"
description = "Tab notes"
```

On Linux/macOS, change `command` to `"herdr-notes.open-notes"`. Choose another
`key` if Ctrl+Alt+N conflicts with your terminal or desktop. Validate with
`herdr config check`, then reload your Herdr configuration.

The toggle affects the currently focused tab. Closing Notes this way records a
preference to keep it closed; reopening it clears that preference.

## Settings

Find the plugin settings directory with:

```sh
herdr plugin config-dir herdr-notes
```

Create `config.json` there. Its default Windows location is
`%APPDATA%/herdr/plugins/config/herdr-notes/config.json`; environment overrides
can change it. This JSON file is separate from Herdr's `config.toml`.

The following example **enables automatic opening**:

```json
{
  "auto_open": true,
  "remember_width": true,
  "width_columns": 42
}
```

| Setting | Default | Behavior |
| --- | --- | --- |
| `auto_open` | `false` | Open Notes for eligible tabs at server startup and when tabs or workspaces are created; preserve keyboard focus |
| `remember_width` | `true` | Save each tab's resized width within about half a second |
| `width_columns` | `42` | Initial width, from 20 to 160 columns; used when there is no remembered width or width memory is disabled |

Available space may constrain the actual width. Zooming or resizing the outer
terminal does not overwrite the saved preference. Settings are read when needed;
changing `width_columns` affects the next opening, not an already-open pane.
Turning off `auto_open` leaves existing panes open. Unknown settings or invalid
values are rejected rather than silently ignored.

Notes closed using the toggle stays closed across creation hooks and restarts,
until you reopen it with the toggle. The old `follow_tabs` setting is accepted
for compatibility but ignored. Server startup means starting the Herdr server,
not attaching another terminal to an already-running server.

To apply auto-open to an already-running server:

```sh
herdr plugin action invoke herdr-notes.apply-settings-windows  # Windows
herdr plugin action invoke herdr-notes.apply-settings          # Linux/macOS
```

The apply-settings action opens missing eligible Notes panes only when
`auto_open` is enabled. It respects tabs closed using the toggle and skips zoomed
tabs; unzoom and invoke it again if needed.

Opening or closing a split resizes that tab once. Switching between tabs does
not change their layouts. Notes is still a normal split, not a native workspace
sidebar. Manually moving a Notes pane is unsupported; close it and open Notes
in the destination tab instead.

## Editing

| Mode | Key | Action |
| --- | --- | --- |
| Preview | `e` or Enter | Start editing |
| Edit | Escape | Save and return to preview |
| Edit | Ctrl+S | Save without leaving edit mode |
| Preview | Arrows, Page Up/Down, `g` / `G` | Scroll or jump to top/bottom |
| Preview | `x`, then `y` | Clear the note after confirmation |
| Preview | `q` | Save and quit |

Edits autosave after two seconds of inactivity. In edit mode, `q` types a letter;
press Escape first to return to preview. Quitting with `q` does **not** record the
toggle's keep-closed preference, so auto-open may restore Notes on a later startup
or tab/workspace creation event. Use the toggle when you want it to stay closed.

## Storage and migration

Notes uses Herdr's plugin state directory (`%LOCALAPPDATA%/herdr/plugins/herdr-notes`
on Windows). Files use stable tab IDs encoded as hexadecimal, so tab renames
keep the same note. Note files and width preferences survive plugin updates.
These are local files, not cloud-synced notes: installing the plugin on another
computer does not transfer your notes or settings.
Deleting a tab leaves its saved files available for recovery; a new tab has a
new identity and a new note.

Version 0.4 imports an old workspace note into the first tab opened for that
workspace. The original `<workspace-id>.json` is retained unchanged as a backup;
other tabs start empty. Migration is serialized and happens only once per
workspace. The destination is the first eligible tab whose new editor loads the
old note, not necessarily the currently selected tab. There is no migration picker.
Back up the state directory before manual migrations or changing plugin versions.

Files in the state directory include:

| File | Purpose |
| --- | --- |
| `tab<hex-id>.json` | Tab note text and last editor mode |
| `layout-tab<hex-id>.json` | Remembered tab width |
| `closed-tab<hex-id>` | Keep Notes closed until explicitly reopened |
| `<workspace-id>.json` | Retained workspace note from older versions |
| `migration-<workspace-id>.done` | Records the tab selected by one-time migration |

Keep the state directory when updating or uninstalling. To recover text manually,
close the relevant editors before copying or editing note files. Herdr tab IDs
are local identities; copying a state directory to another installation does not
automatically match its files to that installation's tabs.

## Building from source

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

## Development

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
python tools/test_workspace_follow.py
```

The integration test uses an isolated Herdr session and temporary config/state
roots. It checks independent tab notes and widths, stable pane rectangles when
switching tabs, shortcut isolation, migration, and server restart recovery.
Despite its historical filename, it tests the current per-tab behavior. Installer
tests are `powershell -File tools/test_install.ps1` on Windows and
`python3 tools/test_install_unix.py` on Linux/macOS; they check platform selection
and checksum failure handling without real downloads.

## Publishing binaries

The `Release binaries` workflow builds and tests all five targets on a `v*` tag,
checks that the tag matches both manifests, and publishes binaries with SHA-256
checksum files only after every build succeeds. Set the same version in
`Cargo.toml`, `Cargo.lock`, and `herdr-plugin.toml` before tagging. A manual workflow
run builds artifacts without publishing a release. Release assets must remain
available for users installing that version.

## License

[MIT](LICENSE). Original editor by [alexarthurs](https://github.com/alexarthurs/herdr-notes);
this fork adds the per-tab behavior, settings, and binary distribution described above.
