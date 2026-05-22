use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub installed: bool,
    pub message: String,
}

fn source_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../jcode-panel/integrations/obsidian_plugin")
}

fn detect_vault() -> Option<PathBuf> {
    let config = dirs::config_dir()?.join("obsidian/obsidian.json");
    let value: Value = serde_json::from_str(&fs::read_to_string(config).ok()?).ok()?;
    let vaults = value.get("vaults")?.as_object()?;
    vaults
        .values()
        .filter_map(|item| {
            let path = PathBuf::from(item.get("path")?.as_str()?);
            let open = item.get("open").and_then(Value::as_bool).unwrap_or(false);
            let ts = item.get("ts").and_then(Value::as_i64).unwrap_or(0);
            path.exists().then_some((open, ts, path))
        })
        .max_by_key(|(open, ts, _)| (*open, *ts))
        .map(|(_, _, path)| path)
}

fn target_dir() -> Option<PathBuf> {
    Some(detect_vault()?.join(".obsidian/plugins/jcode-panel"))
}

pub fn status() -> IntegrationStatus {
    let installed = target_dir().is_some_and(|dir| dir.join("manifest.json").exists());
    IntegrationStatus {
        installed,
        message: if installed {
            "Obsidian context plugin installed"
        } else {
            "Obsidian vault/plugin not detected"
        }
        .into(),
    }
}

pub fn install() -> IntegrationStatus {
    let Some(dst) = target_dir() else {
        return status();
    };
    let src = source_dir();
    if dst.exists() {
        let _ = fs::remove_dir_all(&dst);
    }
    let _ = fs::create_dir_all(&dst);
    for file in ["manifest.json", "main.js", "styles.css"] {
        let _ = fs::copy(src.join(file), dst.join(file));
    }
    if let Some(obsidian_dir) = dst.parent().and_then(|p| p.parent()) {
        let plugins_path = obsidian_dir.join("community-plugins.json");
        let mut plugins: Vec<String> = fs::read_to_string(&plugins_path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if !plugins.iter().any(|id| id == "jcode-panel") {
            plugins.push("jcode-panel".into());
            let _ = fs::write(
                plugins_path,
                serde_json::to_string_pretty(&plugins).unwrap_or_default(),
            );
        }
    }
    status()
}
