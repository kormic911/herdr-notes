//! Remember user resizing without learning widths imposed by tab moves or zoom.
use serde_json::{Value, json};
use std::path::PathBuf;

fn path(workspace: &str) -> Option<PathBuf> {
    let key = crate::state::note_key(Some(workspace))?;
    let dir = std::env::var_os("HERDR_PLUGIN_STATE_DIR").filter(|p| !p.is_empty())?;
    Some(PathBuf::from(dir).join(format!("layout-{key}.json")))
}

pub fn preferred(workspace: &str, config: &crate::config::Config) -> f64 {
    let saved = config
        .remember_width
        .then(|| {
            let value: Value =
                serde_json::from_str(&std::fs::read_to_string(path(workspace)?).ok()?).ok()?;
            value["width_columns"]
                .as_u64()
                .filter(|w| *w > 0 && *w <= 65535)
        })
        .flatten();
    saved
        .map(|w| w as f64)
        .unwrap_or(config.width_columns.into())
}

#[derive(Debug, PartialEq)]
struct Sample {
    tab: String,
    area: Value,
    width: u64,
}

#[derive(Default)]
pub struct Observer {
    last: Option<Sample>,
}

impl Observer {
    fn observe(&mut self, response: &str, pane: &str) -> Option<u64> {
        let value: Value = serde_json::from_str(response).ok()?;
        let layout = &value["result"]["layout"];
        if layout["zoomed"] == true {
            self.last = None;
            return None;
        }
        let own = layout["panes"]
            .as_array()?
            .iter()
            .find(|p| p["pane_id"] == pane)?;
        let sample = Sample {
            tab: layout["tab_id"].as_str()?.into(),
            area: layout["area"].clone(),
            width: own["rect"]["width"].as_u64()?,
        };
        let changed = self.last.as_ref().is_some_and(|old| {
            old.tab == sample.tab && old.area == sample.area && old.width != sample.width
        });
        let width = sample.width;
        self.last = Some(sample);
        changed.then_some(width).filter(|w| *w > 0)
    }

    pub fn capture(&mut self, pane: &str, workspace: &str) -> std::io::Result<()> {
        let config = crate::config::load()?;
        if !config.remember_width {
            self.last = None;
            return Ok(());
        }
        let response = crate::ipc::call_text("pane.layout", json!({"pane_id":pane}))?;
        if let Some(width) = self.observe(&response, pane)
            && let Some(path) = path(workspace)
        {
            std::fs::create_dir_all(path.parent().unwrap())?;
            let temp = path.with_extension("json.tmp");
            std::fs::write(&temp, json!({"width_columns":width}).to_string())?;
            std::fs::rename(temp, path)?;
        }
        Ok(())
    }
}

/// Observe only. Never move panes or resize the user's terminals.
pub fn start() {
    if std::env::var("HERDR_ENV").as_deref() != Ok("1") { return; }
    let (Ok(pane), Some(key)) = (std::env::var("HERDR_PANE_ID"), crate::state::current_key()) else { return; };
    let _ = std::thread::Builder::new().name("notes-width".into()).spawn(move || {
        let mut observer = Observer::default();
        loop {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = observer.capture(&pane, &key);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(tab: &str, area: u64, width: u64, zoomed: bool) -> String {
        json!({"result":{"layout":{"tab_id":tab,"area":{"width":area},"zoomed":zoomed,"panes":[{"pane_id":"notes","rect":{"width":width}}]}}}).to_string()
    }
    #[test]
    fn remembers_resizes_but_not_moves_viewport_changes_or_zoom() {
        let mut observer = Observer::default();
        for (tab, area, width, zoom, expected) in [
            ("a", 200, 42, false, None),
            ("a", 200, 60, false, Some(60)),
            ("b", 80, 40, false, None),
            ("a", 200, 60, false, None),
            ("a", 100, 30, false, None),
            ("a", 100, 100, true, None),
            ("a", 100, 30, false, None),
            ("a", 100, 35, false, Some(35)),
        ] {
            assert_eq!(
                observer.observe(&sample(tab, area, width, zoom), "notes"),
                expected
            );
        }
    }
}
