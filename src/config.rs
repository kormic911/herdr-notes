//! User settings live outside the managed checkout and survive plugin updates.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub auto_open: bool,
    pub follow_tabs: bool,
    pub remember_width: bool,
    pub width_columns: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_open: false,
            follow_tabs: true,
            remember_width: true,
            width_columns: 42,
        }
    }
}

pub fn path() -> Option<PathBuf> {
    std::env::var_os("HERDR_PLUGIN_CONFIG_DIR")
        .filter(|p| !p.is_empty())
        .map(|p| PathBuf::from(p).join("config.json"))
}

fn parse(text: &str) -> std::io::Result<Config> {
    let config: Config = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    if !(20..=160).contains(&config.width_columns) {
        return Err(std::io::Error::other(
            "width_columns must be between 20 and 160",
        ));
    }
    Ok(config)
}

fn read(path: &Path) -> std::io::Result<Config> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
        Err(err) => Err(err),
    }
}

pub fn load() -> std::io::Result<Config> {
    path()
        .as_deref()
        .map(read)
        .unwrap_or_else(|| Ok(Config::default()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_and_partial_settings() {
        assert_eq!(parse("{}").unwrap(), Config::default());
        let config = parse("\u{feff}{\"auto_open\":true,\"width_columns\":60}").unwrap();
        assert!(config.auto_open && config.follow_tabs);
        assert_eq!(config.width_columns, 60);
    }
    #[test]
    fn invalid_settings_fail_instead_of_silently_opening_panes() {
        for text in [
            "{",
            "{\"autopen\":true}",
            "{\"auto_open\":\"yes\"}",
            "{\"width_columns\":0}",
            "{\"width_columns\":161}",
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
    }
}
