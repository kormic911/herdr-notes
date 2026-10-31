//! Keep one running editor visible across its workspace's tabs. Herdr has no
//! workspace-dock API: moving the existing PTY preserves the in-memory note,
//! cursor and undo state without introducing a second writer.

use serde::Deserialize;
use serde_json::{Value, json};
use std::time::Duration;

#[derive(Deserialize)]
struct Pane {
    pane_id: String,
    workspace_id: Option<String>,
    tab_id: Option<String>,
    #[serde(default)]
    focused: bool,
    #[serde(default)]
    tokens: serde_json::Map<String, Value>,
}

#[derive(Debug, PartialEq)]
struct Destination {
    tab: String,
    focused: String,
    source_tab: String,
}

fn destination(snapshot: &str, pane_id: &str, workspace: &str) -> Option<Destination> {
    let value: Value = serde_json::from_str(snapshot.trim_start_matches('\u{feff}')).ok()?;
    let panes: Vec<Pane> =
        serde_json::from_value(value.get("result")?.get("panes")?.clone()).ok()?;
    let own = panes.iter().find(|p| p.pane_id == pane_id)?;
    let focused = panes.iter().find(|p| p.focused)?;
    // Never drag a workspace's note into a different workspace, even if the
    // caller's inherited environment is stale after a manual move.
    if workspace.is_empty()
        || own.workspace_id.as_deref() != Some(workspace)
        || focused.workspace_id.as_deref() != Some(workspace)
    {
        return None;
    }
    let source_tab = own.tab_id.as_ref()?;
    let tab = focused.tab_id.as_ref()?;
    if tab == source_tab || tab.is_empty() || source_tab.is_empty() {
        return None;
    }
    // Moving the only pane would close its tab. Leave deliberately standalone
    // notes tabs alone; following is for the docked editor.
    if panes
        .iter()
        .filter(|p| p.tab_id.as_ref() == Some(source_tab))
        .count()
        < 2
    {
        return None;
    }
    // Do not chase another Notes instance in an already-corrupt/duplicated
    // layout. The launcher still owns duplicate prevention and stale recovery.
    if panes.iter().any(|p| {
        p.pane_id != pane_id
            && p.workspace_id.as_deref() == Some(workspace)
            && p.tokens.contains_key(crate::state::METADATA_SOURCE)
    }) {
        return None;
    }
    Some(Destination {
        tab: tab.clone(),
        focused: focused.pane_id.clone(),
        source_tab: source_tab.clone(),
    })
}

/// One reconciliation. Re-read focus after planning so delayed responses don't
/// move the note into a tab the user has already left. `focus: false` preserves
/// the terminal they are typing in. Zoomed tabs are refused by Herdr itself.
pub fn reconcile(pane_id: &str, workspace: &str) -> std::io::Result<()> {
    let config = crate::config::load()?;
    if !config.follow_tabs {
        return Ok(());
    }
    reconcile_with_width(
        pane_id,
        workspace,
        crate::width::preferred(workspace, &config),
        crate::ipc::call_text,
    )
}

#[cfg(test)]
fn reconcile_with(
    pane_id: &str,
    workspace: &str,
    call: impl FnMut(&str, Value) -> std::io::Result<String>,
) -> std::io::Result<()> {
    reconcile_with_width(pane_id, workspace, 42.0, call)
}

fn reconcile_with_width(
    pane_id: &str,
    workspace: &str,
    width_columns: f64,
    mut call: impl FnMut(&str, Value) -> std::io::Result<String>,
) -> std::io::Result<()> {
    let snapshot = call("pane.list", json!({}))?;
    let Some(plan) = destination(&snapshot, pane_id, workspace) else {
        return Ok(());
    };
    let layout = call("pane.layout", json!({"pane_id": plan.focused}))?;
    let placement = crate::launch::open_plan_with_width(&layout, width_columns);
    let Some((target, ratio)) = placement.split_once('\t') else {
        return Ok(());
    };
    let Ok(ratio) = ratio.parse::<f64>() else {
        return Ok(());
    };
    let current = call("pane.list", json!({}))?;
    if destination(&current, pane_id, workspace).as_ref() != Some(&plan) {
        return Ok(());
    }
    let response = call(
        "pane.move",
        json!({
            "pane_id": pane_id,
            "destination": {
                "type": "tab", "tab_id": plan.tab,
                "target_pane_id": target, "split": "right", "ratio": ratio
            },
            "focus": false
        }),
    )?;
    let value: Value = serde_json::from_str(&response)?;
    if let Some(error) = value.get("error") {
        return Err(std::io::Error::other(error.to_string()));
    }
    Ok(())
}

/// The follower belongs to the TUI process, not a detached daemon. It exits with
/// the editor. A separate thread keeps a slow/disconnected server from blocking
/// keystrokes, autosaves or the heartbeat. No plugin events spawn extra editors.
pub fn start() {
    if std::env::var("HERDR_ENV").as_deref() != Ok("1") {
        return;
    }
    let (Ok(pane), Ok(workspace)) = (
        std::env::var("HERDR_PANE_ID"),
        std::env::var("HERDR_WORKSPACE_ID"),
    ) else {
        return;
    };
    let _ = std::thread::Builder::new()
        .name("workspace-notes".into())
        .spawn(move || {
            let mut width = crate::width::Observer::default();
            loop {
                std::thread::sleep(Duration::from_millis(500));
                let _ = width.capture(&pane, &workspace);
                let _ = reconcile(&pane, &workspace);
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot() -> Value {
        json!({"result":{"panes":[
            {"pane_id":"w1:notes","workspace_id":"w1","tab_id":"w1:t1",
             "tokens":{"herdr-notes":"100"}},
            {"pane_id":"w1:old","workspace_id":"w1","tab_id":"w1:t1"},
            {"pane_id":"w1:work","workspace_id":"w1","tab_id":"w1:t2","focused":true}
        ]}})
    }

    #[test]
    fn follows_another_tab_of_its_own_workspace() {
        let plan = destination(&snapshot().to_string(), "w1:notes", "w1").unwrap();
        assert_eq!(plan.tab, "w1:t2");
        assert_eq!(plan.focused, "w1:work");
    }

    #[test]
    fn ignores_same_tab_other_workspaces_missing_focus_and_duplicate_editors() {
        for mutation in 0..6 {
            let mut value = snapshot();
            let panes = value["result"]["panes"].as_array_mut().unwrap();
            match mutation {
                0 => panes[2]["tab_id"] = json!("w1:t1"),
                1 => panes[2]["workspace_id"] = json!("w2"),
                2 => panes[2]["focused"] = json!(false),
                3 => panes[2]["tokens"] = json!({"herdr-notes":"100"}),
                4 => panes[0]["workspace_id"] = json!("w2"),
                _ => panes[2]["tab_id"] = Value::Null,
            }
            assert!(destination(&value.to_string(), "w1:notes", "w1").is_none());
        }
    }

    #[test]
    fn preserves_standalone_tabs_and_rejects_invalid_snapshots() {
        let mut value = snapshot();
        value["result"]["panes"].as_array_mut().unwrap().remove(1);
        assert!(destination(&value.to_string(), "w1:notes", "w1").is_none());
        for input in ["", "{}", "{\"error\":{}}"] {
            assert!(destination(input, "w1:notes", "w1").is_none());
        }
        assert!(destination(&snapshot().to_string(), "missing", "w1").is_none());
    }

    #[test]
    fn move_preserves_focus_and_aborts_if_user_switches_during_planning() {
        for changed in [false, true] {
            let mut calls = 0;
            reconcile_with("w1:notes", "w1", |method, params| {
                calls += 1;
                match calls {
                    1 => {
                        assert_eq!(method, "pane.list");
                        Ok(snapshot().to_string())
                    }
                    2 => {
                        assert_eq!(method, "pane.layout");
                        Ok(json!({"result":{"layout":{"panes":[
                            {"pane_id":"w1:work","rect":{"x":0,"y":0,"width":120}}
                        ]}}})
                        .to_string())
                    }
                    3 => {
                        let mut value = snapshot();
                        if changed {
                            value["result"]["panes"][2]["workspace_id"] = json!("w2");
                        }
                        Ok(value.to_string())
                    }
                    4 => {
                        assert!(!changed);
                        assert_eq!(method, "pane.move");
                        assert_eq!(params["focus"], false);
                        assert_eq!(params["destination"]["tab_id"], "w1:t2");
                        assert_eq!(params["destination"]["target_pane_id"], "w1:work");
                        Ok(json!({"result":{"changed":true}}).to_string())
                    }
                    _ => panic!("unexpected request"),
                }
            })
            .unwrap();
            assert_eq!(calls, if changed { 3 } else { 4 });
        }
    }
}
