//! One-shot startup/workspace-created reconciliation. Always uses explicit IDs
//! and never changes the user's focus. Serialized across concurrent hooks.
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::io;
use std::process::Command;

fn cli(args: &[&str]) -> io::Result<Value> {
    let mut cmd =
        Command::new(std::env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into()));
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }
    let output = cmd.output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    if output.stdout.is_empty() {
        return Ok(Value::Null);
    }
    let value: Value = serde_json::from_slice(&output.stdout)?;
    if let Some(error) = value.get("error") {
        return Err(io::Error::other(error.to_string()));
    }
    Ok(value["result"].clone())
}

fn is_notes(pane: &Value) -> bool {
    pane["label"] == crate::state::PANE_LABEL
        || pane["tokens"].get(crate::state::METADATA_SOURCE).is_some()
}

fn live(pane: &Value, now: u64) -> bool {
    pane["tokens"][crate::state::METADATA_SOURCE]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .is_some_and(|stamp| now.saturating_sub(stamp) <= crate::launch::HEARTBEAT_STALE_SECS)
}

fn workspaces(panes: &[Value]) -> BTreeSet<String> {
    panes
        .iter()
        .filter_map(|p| p["workspace_id"].as_str())
        .filter(|id| crate::state::note_key(Some(id)).is_some())
        .map(str::to_owned)
        .collect()
}

fn shell_command(path: &str) -> String {
    #[cfg(windows)]
    {
        format!(
            "& '{}'; exit",
            path.strip_prefix(r"\\?\")
                .unwrap_or(path)
                .replace('\'', "''")
        )
    }
    #[cfg(not(windows))]
    {
        format!("exec '{}'", path.replace('\'', "'\"'\"'"))
    }
}

pub fn run() -> io::Result<()> {
    let config = crate::config::load()?;
    if !config.auto_open {
        return Ok(());
    }
    let state_dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR")
        .ok_or_else(|| io::Error::other("auto-open requires HERDR_PLUGIN_STATE_DIR"))?;
    let state_dir = std::path::PathBuf::from(state_dir);
    std::fs::create_dir_all(&state_dir)?;
    let lock = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(state_dir.join("auto-open.lock"))?;
    lock.lock()?;
    let snapshot = cli(&["pane", "list"])?;
    let panes = snapshot["panes"]
        .as_array()
        .ok_or_else(|| io::Error::other("Missing pane list"))?;
    let binary = std::env::current_exe()?;
    for workspace in workspaces(panes) {
        // Refresh while holding the lock: startup and creation hooks can race.
        let snapshot = cli(&["pane", "list"])?;
        let all = snapshot["panes"]
            .as_array()
            .ok_or_else(|| io::Error::other("Missing pane list"))?;
        let local: Vec<_> = all
            .iter()
            .filter(|p| p["workspace_id"] == workspace)
            .collect();
        let notes: Vec<_> = local.iter().copied().filter(|p| is_notes(p)).collect();
        if notes.len() > 1 || notes.iter().any(|p| live(p, crate::state::unix_now())) {
            continue;
        }
        let anchor = local
            .iter()
            .copied()
            .find(|p| !is_notes(p) && p["focused"] == true)
            .or_else(|| local.iter().copied().find(|p| !is_notes(p)));
        let Some(anchor) = anchor.and_then(|p| p["pane_id"].as_str()) else {
            continue;
        };
        // Restore the saved note when Herdr restores an empty Notes shell.
        // Match the launcher's graceful stale-pane recovery rather than writing
        // a command into an unknown foreground process.
        for stale in notes {
            if let Some(id) = stale["pane_id"].as_str() {
                let _ = cli(&["pane", "send-keys", id, "Escape", "q"]);
                std::thread::sleep(std::time::Duration::from_millis(400));
                let _ = cli(&["pane", "close", id]);
            }
        }
        let layout = cli(&["pane", "layout", "--pane", anchor])?;
        // Respect intentional zoom; the user can open Notes after unzooming.
        if layout["layout"]["zoomed"] == true {
            continue;
        }
        let plan = crate::launch::open_plan_with_width(
            &json!({"result":layout}).to_string(),
            config.width_columns.into(),
        );
        let Some((target, ratio)) = plan.split_once('\t') else {
            continue;
        };
        let state_env = format!("HERDR_PLUGIN_STATE_DIR={}", state_dir.display());
        let config_dir = crate::config::path()
            .and_then(|p| p.parent().map(std::path::Path::to_owned))
            .ok_or_else(|| io::Error::other("auto-open requires HERDR_PLUGIN_CONFIG_DIR"))?;
        let config_env = format!("HERDR_PLUGIN_CONFIG_DIR={}", config_dir.display());
        let split = cli(&[
            "pane",
            "split",
            target,
            "--direction",
            "right",
            "--ratio",
            ratio,
            "--no-focus",
            "--env",
            &state_env,
            "--env",
            &config_env,
        ])?;
        let id = split["pane"]["pane_id"]
            .as_str()
            .ok_or_else(|| io::Error::other("Missing new pane id"))?;
        let command = shell_command(&binary.to_string_lossy());
        crate::ipc::stamp_identity(id)?;
        if let Err(err) = cli(&["pane", "run", id, &command]) {
            let _ = cli(&["pane", "close", id]);
            return Err(err);
        }
        cli(&["pane", "rename", id, crate::state::PANE_LABEL])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detects_live_and_restored_notes_and_filters_unsafe_workspaces() {
        assert!(is_notes(&json!({"label":"Notes"})));
        assert!(!live(&json!({"label":"Notes"}), 100));
        assert!(live(&json!({"tokens":{"herdr-notes":"95"}}), 100));
        assert!(!live(&json!({"tokens":{"herdr-notes":"1"}}), 100));
        assert_eq!(
            workspaces(&[
                json!({"workspace_id":"w1"}),
                json!({"workspace_id":"w1"}),
                json!({"workspace_id":"../x"}),
                json!({})
            ]),
            BTreeSet::from(["w1".into()])
        );
    }
    #[test]
    fn quotes_shell_paths() {
        let cmd = shell_command("/a path/it's notes");
        #[cfg(windows)]
        assert_eq!(cmd, "& '/a path/it''s notes'; exit");
        #[cfg(not(windows))]
        assert_eq!(cmd, "exec '/a path/it'\"'\"'s notes'");
    }
}
