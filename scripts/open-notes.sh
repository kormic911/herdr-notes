#!/usr/bin/env bash
# open-notes.sh — unix launcher for the herdr-notes pane.
#
# Toggle Notes in the focused tab: open if absent, save and close if live,
# recover if stale. Each tab has its own note file and width preference.
#
# Right dock: split the tab's RIGHTMOST pane to the right. `pane split --ratio`
# is the ORIGINAL pane's share, so ~0.7 leaves the new Notes pane ~0.3 on the
# right edge — no `pane swap` needed.
#
# All ids/ratios come from the binary's unit-tested stdin modes
# (--launch-decision / --focused-pane / --open-plan), never ad-hoc JSON
# parsing; the ids it emits are validated flag-safe before reaching an argv.
set -uo pipefail

herdr_bin="${HERDR_BIN_PATH:-herdr}"
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
bin="$script_dir/../target/release/herdr-notes"

# Without the binary there is no decision logic; fall back to herdr's
# declarative pane open (right split — degraded but functional).
if [ ! -x "$bin" ]; then
  exec "$herdr_bin" plugin pane open \
    --plugin herdr-notes \
    --entrypoint notes \
    --placement split \
    --direction right \
    --focus
fi

# Read the global list to find the currently focused tab. The Rust helper
# scopes the toggle by that tab ID rather than an inherited workspace ID.
panes="$("$herdr_bin" pane list 2>/dev/null || true)"

open_pane() {
  "$bin" --visibility open
  local fp fid fcwd plan target ratio out np
  fp="$(printf '%s' "$panes" | "$bin" --focused-pane 2>/dev/null || true)"
  fid="${fp%%	*}"
  fcwd="${fp#*	}"
  if [ -z "$fid" ]; then
    exec "$herdr_bin" plugin pane open --plugin herdr-notes \
      --entrypoint notes --placement split --direction right --focus
  fi

  target="$fid"
  ratio="0.70"
  plan="$("$herdr_bin" pane layout --pane "$fid" 2>/dev/null | "$bin" --open-plan 2>/dev/null || true)"
  if [ -n "$plan" ]; then
    target="${plan%%	*}"
    ratio="${plan#*	}"
  fi

  # Per herdr's plugin docs, durable state belongs in HERDR_PLUGIN_STATE_DIR;
  # actions receive it but split panes do not — pass it through to the TUI.
  out="$("$herdr_bin" pane split "$target" --direction right --ratio "$ratio" \
    ${fcwd:+--cwd "$fcwd"} \
    ${HERDR_PLUGIN_STATE_DIR:+--env "HERDR_PLUGIN_STATE_DIR=$HERDR_PLUGIN_STATE_DIR"} \
    ${HERDR_PLUGIN_CONFIG_DIR:+--env "HERDR_PLUGIN_CONFIG_DIR=$HERDR_PLUGIN_CONFIG_DIR"} \
    --no-focus 2>/dev/null || true)"
  np="$(printf '%s' "$out" | sed -n 's/.*"pane_id":"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$np" ] || exit 1

  # Stamp the identity token BEFORE the TUI spawns (best effort — the TUI
  # re-stamps at startup): --launch-decision treats label-without-token as
  # a restart corpse to REPLACE, so a fresh pane must never be observable
  # in that state, or a second toggle during startup would replace it.
  "$bin" --stamp "$np" >/dev/null 2>&1 || true

  # The split already put the new pane on the right edge — no swap needed.
  "$herdr_bin" pane run "$np" "exec \"$bin\""
  "$herdr_bin" pane rename "$np" "Notes" >/dev/null 2>&1 || true
  # herdr has no focus-by-id; a zoom on/off cycle focuses deterministically.
  "$herdr_bin" pane zoom "$np" --on >/dev/null 2>&1 || true
  exec "$herdr_bin" pane zoom "$np" --off
}

decision="OPEN"
if [ -n "$panes" ]; then
  decision="$(printf '%s' "$panes" | "$bin" --launch-decision 2>/dev/null || echo OPEN)"
fi

case "$decision" in
  "CLOSE "*)
    pid="${decision#CLOSE }"
    "$bin" --visibility closed
    # Graceful save+quit before the close: Esc leaves edit mode (which saves),
    # q quits from preview. `pane close` alone kills the TUI, losing any
    # keystrokes still inside the 2s autosave debounce window.
    "$herdr_bin" pane send-keys "$pid" Escape q >/dev/null 2>&1 || true
    sleep 0.4
    # `pane run` exec'd the TUI, so the pane normally closes itself when the
    # TUI quits; this close is cleanup for a wedged TUI and often finds the
    # pane already gone — that is success, not an error.
    "$herdr_bin" pane close "$pid" >/dev/null 2>&1 || true
    exit 0
    ;;
  "REPLACE "*)
    # Dead pane (stale heartbeat): close the corpse, then dock a fresh one.
    # The Esc+q is a best-effort save in case the pane is alive after all
    # (e.g. just woken from a suspend); harmless on a real corpse.
    pid="${decision#REPLACE }"
    "$herdr_bin" pane send-keys "$pid" Escape q >/dev/null 2>&1 || true
    sleep 0.4
    "$herdr_bin" pane close "$pid" >/dev/null 2>&1 || true
    # Re-list panes AFTER the close: the corpse may have been the focused
    # pane, and open_pane derives its split target from this snapshot — a
    # stale one made `pane layout`/`pane split` fail with pane_not_found.
    panes="$("$herdr_bin" pane list 2>/dev/null || true)"
    open_pane
    ;;
  *)
    open_pane
    ;;
esac
