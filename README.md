# herdr-notes

Persistent Markdown notes for [Herdr](https://herdr.dev), with a separate note
and Notes pane in each tab. Fork of [alexarthurs/herdr-notes](https://github.com/alexarthurs/herdr-notes).

Split a pane wherever you want Notes, then open it in that pane. Notes comes
back in the same pane after a Herdr server restart. Each tab has its own note
with Markdown preview, plain-text editing, and automatic saving.

## Install

```sh
herdr plugin install kormic911/herdr-notes --yes
```

Requires Herdr 0.9.5 or newer and Git. **Rust is not required for installation.**
The installer downloads the binary matching the plugin version from this fork's
GitHub Releases and verifies its SHA-256 checksum before installing it.

Supported binaries: Windows x64 (also usable on Windows ARM64 via x64 emulation),
Linux x64/ARM64 (static musl builds), and macOS Intel/Apple Silicon. Windows uses
PowerShell 5.1 or later. Linux/macOS use Bash, curl, and sha256sum or shasum.
Downloads require access to GitHub Release assets. A missing release, unsupported
platform, or checksum failure stops installation; it never silently compiles.

### Updating or switching from upstream

Save and close running Notes panes before replacing the plugin, then rerun the
install command above. This fork uses the same `herdr-notes` plugin ID as upstream.
Saved tab notes are reused, and [workspace note migration](#storage-and-migration)
still applies. Version 1.0.0 replaces right-docked opening with opening in the
selected shell. Herdr controls pane size through its normal layout controls.
The old auto-open and dock-width settings are retired.

## First run

1. Install the Notes key binding.

   ```sh
   herdr plugin action invoke install-keybindings --plugin herdr-notes
   ```

2. Split a pane and focus the new shell prompt.
3. Press the Herdr prefix (**Ctrl+B** by default), release it, then press **F4**.
4. Press **q** from Notes preview to save and return to the shell.

Open Notes once in the pane you want to reuse. Herdr then restores Notes there
when its server restarts. Quitting with **q** clears that pane’s saved placement.

The keybinding installer preserves existing assignments, validates the config
and reloads Herdr. It reports a conflict in the plugin log if **prefix then F4**
is already assigned. You can change the shortcut in Herdr’s `config.toml`. Run
the installer after upgrading to update older Windows bindings while keeping
their assigned shortcuts.

You can also run **Install Notes key bindings** or **Open Notes in this shell**
from Herdr’s plugin actions.

From a built checkout, run `./scripts/install-keybindings.sh` on Linux or macOS,
or `.\scripts\install-keybindings.ps1` on Windows, to see the installer output
directly.

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
press Escape first to return to preview.

## Storage and migration

Notes uses Herdr's plugin state directory (`%LOCALAPPDATA%/herdr/plugins/herdr-notes`
on Windows). Files use stable tab IDs encoded as hexadecimal, so tab renames
keep the same note. Note files survive plugin updates.
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
| `placements/<session-hash>/tab<hex-id>.json` | Saved pane for restart and its original label |
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
python tools/test_tab_notes.py
```

The integration test uses an isolated Herdr session and temporary config and
state directories. It checks binding installation, opening in the chosen pane,
reusing saved tab notes, workspace note migration, and server restart recovery.
Installer
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
The pane workflow and keybinding installer are adapted from [Command Shelf](https://github.com/kormic911/herdr-command-shelf).
