<div align="center">

# Herdr Notes

### The scratch note that lives beside your agents.

One markdown note per workspace in a [Herdr](https://github.com/herdrdev/herdr)
pane that follows you between tabs. The same editor stays running, preserving
your cursor, edit mode and unsaved text. Notes autosave and survive restarts.

This is a fork of [alexarthurs/herdr-notes](https://github.com/alexarthurs/herdr-notes)
with automatic workspace tab following. Requires Herdr 0.9.3 or newer.

<img alt="Rust" src="https://img.shields.io/badge/Rust-self--contained_crate-orange?logo=rust&logoColor=white">
<img alt="herdr" src="https://img.shields.io/badge/herdr-%E2%89%A5%200.9.3-5865a3">
<img alt="Platforms" src="https://img.shields.io/badge/Windows%20%C2%B7%20macOS%20%C2%B7%20Linux-supported-2ea44f">
<img alt="CI" src="https://github.com/kormic911/herdr-notes/actions/workflows/ci.yml/badge.svg">
<img alt="License" src="https://img.shields.io/badge/license-MIT-blue">

<br><br>

<img src="docs/media/hero.png" alt="The Notes pane docked beside a running test suite: rendered markdown with headings, checkboxes, code and quotes" width="920">

</div>

That's the note docked on the right, keeping its shape while the test suite runs
next door. Terminals are where the work happens, and the work generates thoughts —
half-finished todo lists, commands you keep retyping, things to ask about later.
This gives them a permanent home one keypress away: no editor window, no stray
`notes.txt`, no saving.

```
herdr plugin install kormic911/herdr-notes
```

---

## Why you'll keep it open

- **Rendered markdown** — headings, checkboxes, lists, quotes, code blocks
  and inline styles, drawn natively in the terminal with a scrollbar.
- **Zero-friction editing** — `e` to type, `Esc` to go back. That's it.
- **A note per workspace** — every herdr space keeps its own note, keyed to
  the workspace itself (renames don't lose it).
- **Actually persistent** — atomic autosaves to a per-workspace JSON file in
  herdr's config directory. Close the pane, kill the terminal, reboot: it
  comes back.
- **A polite pane** — one toggle action opens, focuses, or closes it; a
  heartbeat token means a dead pane — even one left behind by a herdr
  server restart — gets replaced on the next toggle, never duplicated.

## Install

From a checkout of this repo:

```
cargo build --release
herdr plugin link .
```

Or straight from GitHub with the command under the hero image.

When replacing the upstream plugin, quit its open Notes panes first (`Esc`,
then `q`). This fork keeps the same plugin ID and note format, so existing notes
are preserved. Do not run both versions against the same workspace note.

## Open

One toggle action opens the workspace's notes on the right, focuses the existing
editor if open, and closes it if already focused:

```
herdr plugin action invoke herdr-notes.open-notes-windows   # windows
herdr plugin action invoke herdr-notes.open-notes           # linux / macos
```

Once open, Notes follows tab changes within its workspace, normally within half
a second, without taking keyboard focus. Returning from another workspace brings
that workspace's editor to its selected tab. No second editor is created.

Herdr has no native workspace-dock API. This fork moves the existing terminal
pane between tab layouts; it is not a permanent UI sidebar. Zoomed layouts defer
the move until unzoomed. A deliberately standalone Notes tab is left in place
to avoid closing that tab. Closing the tab currently containing Notes closes the
editor too; use the shortcut to reopen the saved note. With multiple attached
clients, following uses Herdr's server-wide focus, not an individual client's view.

### Keyboard shortcut

Add this to your Herdr `config.toml` (Windows: `%APPDATA%\\herdr\\config.toml`):

```toml
[[keys.command]]
key = "ctrl+alt+n"
type = "plugin_action"
command = "herdr-notes.open-notes-windows"
description = "Workspace notes"
```

On Linux/macOS, use `herdr-notes.open-notes` as the command. Run
`herdr config check` and `herdr server reload-config` to apply it. If your
terminal or keyboard layout reserves `Ctrl+Alt+N`, use an available prefix
combination such as `prefix+n` instead.

First run greets you with the keymap:

<div align="center">
<img src="docs/media/welcome.png" alt="Empty note showing the built-in key reference" width="920">
</div>

## Keys

Preview (default):

| Key | Action |
| --- | --- |
| `e` / `Enter` | edit the note |
| `Up` `Down` `PgUp` `PgDn` | scroll |
| `g` / `G` | jump to top / bottom |
| `x` | clear the note (y/N confirm) |
| `q` | quit |

Edit:

| Key | Action |
| --- | --- |
| `Esc` | back to preview (saves) |
| `Ctrl+S` | save now (autosave runs anyway, ~2s after the last keystroke) |

`Esc` never exits the app.

<div align="center">
<img src="docs/media/edit.png" alt="Edit mode: the same note as plain markdown with a block cursor" width="920">
</div>

## Persistence

Each herdr workspace gets its own note, stored as
`<workspace-id>.json` in herdr's per-plugin state directory
(`HERDR_PLUGIN_STATE_DIR` — e.g. `%LOCALAPPDATA%\herdr\plugins\herdr-notes\`
on Windows), keyed by the stable `HERDR_WORKSPACE_ID` herdr injects into
every pane — ids survive workspace renames, so the note follows the
workspace, not its label. Closing a workspace just orphans its file; delete
`<workspace-id>.json` by hand if you want it gone. Run outside herdr, the
pane falls back to `herdr/notes/` under the platform config dir (single
shared `notes.json` when there's no workspace id), and any note found in
the fallback layout is moved into the state dir on first load — an
existing note is inherited, never lost.

The format is `{ "text": "...", "mode": "preview"|"edit" }`. Saves are
atomic (temp file + fsync + rename) and happen on leaving edit mode, clear,
quit, and debounced while typing. A missing or corrupt file falls back to
an empty note — it never wedges the pane.

## Hacking

`CLAUDE.md` has the build/dev workflow and the hard-won herdr/Windows
gotchas (pane spawning, heartbeats, PowerShell 5.1 quirks). The short
version: `cargo build --release`, `cargo test`,
`cargo clippy --all-targets -- -D warnings`, all green before shipping.
