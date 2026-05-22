use std::collections::BTreeSet;

const MODIFIER_ORDER: [&str; 4] = ["ctrl", "alt", "shift", "super"];

pub fn normalize_key_name(name: &str) -> String {
    let key = name.trim().to_ascii_lowercase().replace(' ', "_");
    let key = key.strip_prefix("key.").unwrap_or(&key);
    match key {
        "control" | "control_l" | "control_r" | "ctrl_l" | "ctrl_r" => "ctrl".into(),
        "shift_l" | "shift_r" => "shift".into(),
        "alt_l" | "alt_r" | "option" => "alt".into(),
        "super_l" | "super_r" | "cmd" | "command" | "win" | "windows" => "super".into(),
        "esc" => "escape".into(),
        "return" | "kp_enter" => "enter".into(),
        _ => key.into(),
    }
}

pub fn normalize_hotkey(hotkey: &str) -> String {
    let parts: Vec<String> = hotkey
        .replace('-', "+")
        .split('+')
        .map(normalize_key_name)
        .filter(|p| !p.is_empty())
        .collect();
    let modifiers: Vec<&str> = MODIFIER_ORDER
        .iter()
        .copied()
        .filter(|m| parts.iter().any(|p| p == m))
        .collect();
    let key = parts
        .iter()
        .rev()
        .find(|p| !MODIFIER_ORDER.contains(&p.as_str()))
        .cloned()
        .unwrap_or_default();
    let mut out: Vec<String> = modifiers.into_iter().map(str::to_string).collect();
    if !key.is_empty() {
        out.push(key);
    }
    if out.is_empty() {
        "f8".into()
    } else {
        out.join("+")
    }
}

pub fn hotkey_parts(hotkey: &str) -> (BTreeSet<String>, String) {
    let normalized = normalize_hotkey(hotkey);
    let mut mods = BTreeSet::new();
    let mut key = String::new();
    for part in normalized.split('+') {
        if MODIFIER_ORDER.contains(&part) {
            mods.insert(part.to_string());
        } else {
            key = part.to_string();
        }
    }
    (mods, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_hotkeys_and_parts() {
        assert_eq!(normalize_key_name("Key.Control_L"), "ctrl");
        assert_eq!(normalize_hotkey("Shift-Ctrl-F8"), "ctrl+shift+f8");
        let (mods, key) = hotkey_parts("ctrl+shift+s");
        assert!(mods.contains("ctrl") && mods.contains("shift"));
        assert_eq!(key, "s");
    }
}
