#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AmbientAction {
    Append(String),
    Submit,
    Hide,
    Backspace,
    Edit(String),
    Dismissed,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct AmbientState {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub floating_visible: bool,
    pub entry_has_focus: bool,
    pub toast_visible: bool,
}

pub fn route_ambient_key(
    state: &mut AmbientState,
    name: &str,
    ch: Option<char>,
    pressed: bool,
    force: bool,
) -> AmbientAction {
    if !force && !pressed {
        return AmbientAction::None;
    }
    let key = name.to_ascii_lowercase();
    match key.as_str() {
        "shift" | "shift_l" | "shift_r" => {
            state.shift = pressed;
            return AmbientAction::None;
        }
        "ctrl" | "control" | "control_l" | "control_r" => {
            state.ctrl = pressed;
            return AmbientAction::None;
        }
        "alt" | "alt_l" | "alt_r" => {
            state.alt = pressed;
            return AmbientAction::None;
        }
        _ => {}
    }
    if !pressed {
        return AmbientAction::None;
    }
    if state.toast_visible && (key == "esc" || key == "escape") {
        return AmbientAction::Dismissed;
    }
    if !state.floating_visible || state.entry_has_focus {
        return AmbientAction::None;
    }
    match key.as_str() {
        "enter" | "return" => AmbientAction::Submit,
        "esc" | "escape" => AmbientAction::Hide,
        "backspace" => AmbientAction::Backspace,
        "left" | "right" | "home" | "end" | "delete" => AmbientAction::Edit(key),
        _ => ch
            .filter(|_| !state.ctrl && !state.alt)
            .map(|c| AmbientAction::Append(c.to_string()))
            .unwrap_or(AmbientAction::None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ambient_routing() {
        let mut s = AmbientState {
            floating_visible: true,
            entry_has_focus: true,
            ..Default::default()
        };
        assert_eq!(
            route_ambient_key(&mut s, "", Some('a'), true, true),
            AmbientAction::None
        );
        s.entry_has_focus = false;
        assert_eq!(
            route_ambient_key(&mut s, "", Some('a'), true, true),
            AmbientAction::Append("a".into())
        );
        assert_eq!(
            route_ambient_key(&mut s, "left", None, true, true),
            AmbientAction::Edit("left".into())
        );
        let mut t = AmbientState {
            toast_visible: true,
            ..Default::default()
        };
        assert_eq!(
            route_ambient_key(&mut t, "esc", None, true, true),
            AmbientAction::Dismissed
        );
    }
}
