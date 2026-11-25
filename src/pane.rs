// Pane launching and restoration follow the shell workflow in
// herdr-command-shelf (MIT, Alex Arthurs).

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions, TryLockError},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

const LABEL: &str = "Notes";

pub fn herdr_bin() -> OsString {
    env::var_os("HERDR_BIN_PATH").unwrap_or_else(|| "herdr".into())
}

fn call(args: &[&str]) -> Result<Value> {
    call_with(&herdr_bin(), args)
}

fn call_with(bin: &OsStr, args: &[&str]) -> Result<Value> {
    let output = Command::new(bin).args(args).output()?;
    if args.starts_with(&["pane", "run"]) && output.status.success() && output.stdout.is_empty() {
        return Ok(Value::Null);
    }
    let reply: Value = serde_json::from_slice(&output.stdout).unwrap_or(Value::Null);
    ensure!(
        output.status.success() && reply.get("error").is_none(),
        "herdr {}: {}",
        args.join(" "),
        reply
            .pointer("/error/message")
            .and_then(Value::as_str)
            .unwrap_or(String::from_utf8_lossy(&output.stderr).trim())
    );
    reply.get("result").cloned().context("Missing Herdr result")
}

#[derive(Deserialize)]
struct Pane {
    pane_id: String,
    terminal_id: String,
    workspace_id: String,
    tab_id: String,
    label: Option<String>,
    agent: Option<String>,
    display_agent: Option<String>,
    agent_session: Option<Value>,
    agent_status: String,
}

fn get(id: &str) -> Result<Pane> {
    get_with(&herdr_bin(), id)
}

fn get_with(bin: &OsStr, id: &str) -> Result<Pane> {
    Ok(serde_json::from_value(
        call_with(bin, &["pane", "get", id])?["pane"].take(),
    )?)
}

#[derive(Deserialize, Serialize)]
struct Placement {
    pane_id: String,
    original_label: Option<String>,
}

fn state_dir() -> Result<PathBuf> {
    env::var_os("HERDR_PLUGIN_STATE_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .context("Missing Herdr Notes state directory")
}

fn placement_path(root: &Path, tab: &str) -> Result<PathBuf> {
    let socket = env::var_os("HERDR_SOCKET_PATH").context("Missing Herdr session socket")?;
    let session = format!("{:x}", Sha256::digest(socket.as_encoded_bytes()));
    let tab = crate::state::tab_key(tab).context("Invalid Herdr tab ID")?;
    let directory = root.join("placements").join(session);
    fs::create_dir_all(&directory)?;
    Ok(directory.join(format!("{tab}.json")))
}

fn lock_file(path: &Path, extension: &str) -> Result<File> {
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_extension(extension))?)
}

fn is_running(lease: &File) -> Result<bool> {
    match lease.try_lock() {
        Ok(()) => {
            lease.unlock()?;
            Ok(false)
        }
        Err(TryLockError::WouldBlock) => Ok(true),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn placement(path: &Path) -> Result<Option<Placement>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

pub fn open() -> Result<()> {
    let context: Value = serde_json::from_str(
        &env::var("HERDR_PLUGIN_CONTEXT_JSON").unwrap_or_else(|_| "{}".into()),
    )?;
    let id = context["focused_pane_id"]
        .as_str()
        .map(str::to_owned)
        .or_else(|| env::var("HERDR_PANE_ID").ok())
        .context("Invoke Open Notes from a shell pane")?;
    launch(&state_dir()?, &get(&id)?, false)
}

pub fn restore() -> Result<()> {
    let root = state_dir()?;
    let panes: Vec<Pane> = serde_json::from_value(call(&["pane", "list"])?["panes"].take())?;
    let mut failures = Vec::new();
    for pane in panes
        .iter()
        .filter(|pane| pane.label.as_deref() == Some(LABEL))
    {
        if let Err(error) = launch(&root, pane, true) {
            failures.push(format!("{}: {error}", pane.pane_id));
        }
    }
    ensure!(failures.is_empty(), "{}", failures.join(", "));
    Ok(())
}

fn launch(root: &Path, target: &Pane, restoring: bool) -> Result<()> {
    let path = placement_path(root, &target.tab_id)?;
    let launch_lock = lock_file(&path, "launch.lock")?;
    launch_lock.lock()?;
    let lease = lock_file(&path, "editor.lock")?;
    if is_running(&lease)? {
        if restoring || placement(&path)?.is_some_and(|saved| saved.pane_id == target.pane_id) {
            return Ok(());
        }
        bail!("Notes is already open in this tab");
    }
    let current = get(&target.pane_id)?;
    ensure!(
        current.terminal_id == target.terminal_id && current.tab_id == target.tab_id,
        "The target shell changed"
    );
    if restoring
        && (current.label.as_deref() != Some(LABEL)
            || !placement(&path)?.is_some_and(|saved| saved.pane_id == current.pane_id))
    {
        return Ok(());
    }
    let info = call(&["pane", "process-info", "--pane", &current.pane_id])?;
    if current.agent.is_some()
        || current.display_agent.is_some()
        || current.agent_session.is_some()
        || current.agent_status != "unknown"
        || !foreground_shell(&info["process_info"])
    {
        if restoring {
            return Ok(());
        }
        bail!("Focus a shell prompt before opening Notes");
    }

    let exe = env::current_exe()?;
    let bin = herdr_bin();
    let exe = exe.to_str().context("Invalid Notes executable path")?;
    let bin = bin.to_str().context("Invalid Herdr executable path")?;
    #[cfg(windows)]
    let exe = windows_shell_path(exe);
    #[cfg(windows)]
    let bin = windows_shell_path(bin);
    #[cfg(windows)]
    let (exe, bin) = (exe.as_ref(), bin.as_ref());
    let command = launch_command(&[
        exe,
        "--edit",
        root.to_str().context("Invalid Notes state path")?,
        &current.pane_id,
        &current.terminal_id,
        bin,
    ])?;
    call(&["pane", "run", &current.pane_id, &command])?;

    let start = Instant::now();
    while !is_running(&lease)? {
        ensure!(
            start.elapsed() < Duration::from_secs(8),
            "Notes did not start in {}",
            current.pane_id
        );
        thread::sleep(Duration::from_millis(50));
    }
    Ok(())
}

pub fn edit(root: &str, id: &str, terminal: &str, bin: &str) -> Result<()> {
    ensure!(
        env::var("HERDR_PANE_ID").as_deref() == Ok(id),
        "Notes pane identity changed"
    );
    let bin = OsStr::new(bin);
    let pane = get_with(bin, id)?;
    ensure!(
        pane.terminal_id == terminal,
        "Notes terminal identity changed"
    );
    let path = placement_path(Path::new(root), &pane.tab_id)?;
    let lease = lock_file(&path, "editor.lock")?;
    lease
        .try_lock()
        .context("Notes is already open in this tab")?;

    let original_label = match placement(&path)? {
        Some(saved) if saved.pane_id == id && pane.label.as_deref() == Some(LABEL) => {
            saved.original_label
        }
        _ => pane.label,
    };
    let saved = Placement {
        pane_id: id.to_owned(),
        original_label,
    };
    let mut candidate = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
    serde_json::to_writer(&mut candidate, &saved)?;
    candidate.flush()?;
    candidate.persist(&path)?;
    call_with(bin, &["pane", "rename", id, LABEL])?;

    let (note, save_path) =
        crate::state::tab_note(Path::new(root), &pane.workspace_id, &pane.tab_id)
            .context("Invalid Notes tab ID")?;
    crate::edit_note(crate::app::App::with_note(note, Some(save_path)))?;

    // An explicit editor quit returns the pane to its shell. If the terminal
    // disappears during shutdown, the saved placement is used on next startup.
    if get_with(bin, id)
        .is_ok_and(|live| live.terminal_id == terminal && live.label.as_deref() == Some(LABEL))
    {
        match saved.original_label.as_deref() {
            Some(label) => call_with(bin, &["pane", "rename", id, label])?,
            None => call_with(bin, &["pane", "rename", id, "--clear"])?,
        };
    }
    fs::remove_file(path)?;
    Ok(())
}

fn foreground_shell(info: &Value) -> bool {
    let Some(pid) = info["shell_pid"].as_u64() else {
        return false;
    };
    info["foreground_process_group_id"].as_u64() == Some(pid)
        && info["foreground_processes"]
            .as_array()
            .is_some_and(|processes| {
                processes.len() == 1
                    && processes[0]["pid"].as_u64() == Some(pid)
                    && processes[0]["name"].as_str().is_some_and(|name| {
                        let lower = name
                            .rsplit(['/', '\\'])
                            .next()
                            .unwrap_or(name)
                            .trim_start_matches('-')
                            .to_ascii_lowercase();
                        matches!(
                            lower.trim_end_matches(".exe"),
                            "sh" | "bash"
                                | "dash"
                                | "zsh"
                                | "fish"
                                | "ksh"
                                | "mksh"
                                | "cmd"
                                | "pwsh"
                                | "powershell"
                        )
                    })
            })
}

fn launch_command(args: &[&str]) -> Result<String> {
    if args.iter().any(|s| s.chars().any(char::is_control)) {
        bail!("Invalid control character in launch argument");
    }
    #[cfg(windows)]
    {
        use base64::Engine;
        let script = format!(
            "& {}",
            args.iter()
                .map(|s| quote_word(s, true))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        Ok(format!(
            "powershell.exe -NoLogo -NoProfile -EncodedCommand {}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    }
    #[cfg(not(windows))]
    {
        Ok(args
            .iter()
            .map(|s| quote_word(s, false))
            .collect::<Vec<_>>()
            .join(" "))
    }
}

#[cfg(windows)]
fn windows_shell_path(path: &str) -> std::borrow::Cow<'_, str> {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}").into();
    }
    if let Some(drive) = path.strip_prefix(r"\\?\")
        && drive.as_bytes().get(1..3) == Some(b":\\")
    {
        return drive.into();
    }
    path.into()
}

fn quote_word(word: &str, powershell: bool) -> String {
    let mut quoted = String::from("'");
    for c in word.chars() {
        match c {
            '\'' | '‘'..='‛' if powershell => {
                quoted.push(c);
                quoted.push(c);
            }
            '\'' if !powershell => quoted.push_str("'\\''"),
            // Escape outside single quotes so both POSIX shells and fish preserve it.
            '\\' if !powershell => quoted.push_str("'\\\\'"),
            _ => quoted.push(c),
        }
    }
    quoted.push('\'');
    quoted
}
