// Adapted from herdr-command-shelf (MIT, Alex Arthurs).

use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;
use toml::Value;

const KEY: &str = "prefix+f4";
const ACTION: &str = "herdr-notes.open-notes";

fn same_chord(a: &str, b: &str) -> bool {
    fn parts(key: &str) -> Vec<String> {
        let mut parts: Vec<_> = key
            .split('+')
            .map(|s| s.trim().to_ascii_lowercase())
            .collect();
        parts.sort();
        parts
    }
    parts(a) == parts(b)
}
fn additions(original: &str) -> Result<String> {
    let doc = original
        .trim_start_matches('\u{feff}')
        .parse::<toml::Table>()?;
    let keys = doc.get("keys").and_then(Value::as_table);
    let commands = keys
        .and_then(|k| k.get("command"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default();
    if let Some(binding) = commands.iter().find(|entry| {
        entry.get("type").and_then(Value::as_str) == Some("plugin_action")
            && matches!(
                entry.get("command").and_then(Value::as_str),
                Some(ACTION | "herdr-notes.open-notes-windows")
            )
            && entry
                .get("key")
                .and_then(Value::as_str)
                .is_some_and(|key| !same_chord(key, "ctrl+alt+n"))
    }) {
        println!("Notes already uses {}", binding["key"]);
        return Ok(String::new());
    }
    let occupied = commands
        .iter()
        .filter_map(|entry| entry.get("key").and_then(Value::as_str))
        .chain(keys.into_iter().flat_map(|k| k.values()).flat_map(|v| {
            v.as_str()
                .into_iter()
                .chain(v.as_array().into_iter().flatten().filter_map(Value::as_str))
        }))
        .any(|key| same_chord(key, KEY));
    if occupied {
        println!("Skipped Notes: {KEY} is already assigned.");
        return Ok(String::new());
    }
    let mut text = format!(
        "\n[[keys.command]]\nkey = \"{KEY}\"\ntype = \"plugin_action\"\ncommand = \"{ACTION}\"\ndescription = \"Open Notes here\"\n"
    );
    if original.contains("\r\n") {
        text = text.replace('\n', "\r\n");
    }
    Ok(text)
}
fn checked(cmd: &mut Command) -> Result<String> {
    let output = cmd.output().context("Starting Herdr")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    ensure!(
        output.status.success(),
        "Herdr returned {}: {} {}",
        output.status,
        stdout.trim(),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(stdout.into_owned())
}

fn read(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(s) => Ok(Some(s)),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn config_path() -> Result<PathBuf> {
    let bin = crate::pane::herdr_bin();
    let help = checked(Command::new(&bin).arg("--help"))?;
    let reported = help
        .lines()
        .find_map(|s| s.strip_prefix("Config: "))
        .context("Herdr did not report its config path")?;
    Ok(std::path::absolute(PathBuf::from(reported))?)
}
pub fn install() -> Result<()> {
    let bin = crate::pane::herdr_bin();
    let path = config_path()?;
    let path = if path.exists() {
        path.canonicalize()?
    } else {
        path
    };
    let parent = path.parent().context("Missing config directory")?;
    fs::create_dir_all(parent)?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path.with_added_extension("herdr-notes.lock"))?;
    lock.lock()?;
    let before = read(&path)?;
    let original = before.as_deref().unwrap_or_default();
    let extra = additions(original)?;
    if !extra.is_empty() {
        let mut candidate = NamedTempFile::new_in(parent)?;
        candidate.write_all(original.as_bytes())?;
        candidate.write_all(extra.as_bytes())?;
        candidate.flush()?;
        checked(
            Command::new(&bin)
                .args(["config", "check"])
                .env("HERDR_CONFIG_PATH", candidate.path()),
        )?;
        if let Ok(meta) = fs::metadata(&path) {
            candidate.as_file().set_permissions(meta.permissions())?;
        }
        ensure!(
            read(&path)? == before,
            "Herdr config changed during installation. Run the action again."
        );
        candidate.persist(&path)?;
        println!("Added missing shortcuts to {}", path.display());
    } else {
        checked(Command::new(&bin).args(["config", "check"]))?;
    }
    checked(Command::new(&bin).args(["server", "reload-config"]))
        .context("Reloading Herdr config")?;
    println!("Reloaded Herdr config.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_preserves_existing_assignments() -> Result<()> {
        let binding = additions("")?;
        assert!(binding.contains("prefix+f4"));
        assert!(additions(&binding)?.is_empty());
        assert!(additions(&binding.replace(KEY, "prefix+f9"))?.is_empty());
        assert!(additions("[keys]\nsettings = ['F4+PREFIX']\n")?.is_empty());

        let old = binding
            .replace(KEY, "ctrl+alt+n")
            .replace(ACTION, "herdr-notes.open-notes-windows")
            .replace('\n', "\r\n");
        assert_eq!(additions(&old)?, binding.replace('\n', "\r\n"));
        assert!(additions("[keys").is_err());
        Ok(())
    }
}
