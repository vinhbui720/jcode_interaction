use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub installed: bool,
    pub message: String,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn xpi_path() -> PathBuf {
    home().join(".jcode/browser/browser-agent-bridge.xpi")
}

fn native_host_source() -> PathBuf {
    home().join(".mozilla/native-messaging-hosts/firefox_agent_bridge.json")
}

fn native_host_targets() -> Vec<PathBuf> {
    vec![
        home().join(".mozilla/native-messaging-hosts/firefox_agent_bridge.json"),
        home()
            .join("snap/firefox/common/.mozilla/native-messaging-hosts/firefox_agent_bridge.json"),
    ]
}

fn profiles_ini_candidates() -> Vec<PathBuf> {
    vec![
        home().join(".mozilla/firefox/profiles.ini"),
        home().join("snap/firefox/common/.mozilla/firefox/profiles.ini"),
    ]
}

fn profile_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for ini in profiles_ini_candidates() {
        let Some(base) = ini.parent() else { continue };
        let Ok(text) = fs::read_to_string(&ini) else {
            continue;
        };
        dirs.extend(
            parse_profile_dirs(base, &text)
                .into_iter()
                .filter(|dir| dir.exists()),
        );
    }
    dirs.sort();
    dirs.dedup();
    dirs
}

fn parse_profile_dirs(base: &std::path::Path, profiles_ini: &str) -> Vec<PathBuf> {
    profiles_ini
        .lines()
        .filter_map(|line| line.strip_prefix("Path=").map(str::trim))
        .map(|path| {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                path
            } else {
                base.join(path)
            }
        })
        .collect()
}

pub fn status() -> IntegrationStatus {
    let profiles = profile_dirs();
    let xpi_installed = profiles.iter().any(|profile| {
        profile
            .join("extensions/browser-agent-bridge@1jehuang.github.io.xpi")
            .exists()
    });
    let native_installed = native_host_targets().iter().any(|p| p.exists());
    IntegrationStatus {
        installed: xpi_installed && native_installed,
        message: if xpi_installed && native_installed {
            format!("Firefox bridge installed in {} profile(s)", profiles.len())
        } else if profiles.is_empty() {
            "Firefox profile not found".into()
        } else if !xpi_installed {
            "Firefox bridge XPI not installed in profiles".into()
        } else {
            "Firefox native host not installed".into()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_relative_and_absolute_firefox_profiles() {
        let base = PathBuf::from("/home/me/snap/firefox/common/.mozilla/firefox");
        let dirs = parse_profile_dirs(
            &base,
            "[Profile0]\nPath=abc.default\n[Profile1]\nPath=/tmp/abs.profile\n",
        );
        assert_eq!(dirs[0], base.join("abc.default"));
        assert_eq!(dirs[1], PathBuf::from("/tmp/abs.profile"));
    }
}

pub fn install() -> IntegrationStatus {
    let xpi = xpi_path();
    let native_src = native_host_source();
    for target in native_host_targets() {
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if native_src.exists() {
            let _ = fs::copy(&native_src, &target);
        }
    }
    if xpi.exists() {
        for profile in profile_dirs() {
            let ext_dir = profile.join("extensions");
            let _ = fs::create_dir_all(&ext_dir);
            let _ = fs::copy(
                &xpi,
                ext_dir.join("browser-agent-bridge@1jehuang.github.io.xpi"),
            );
        }
    }
    status()
}
