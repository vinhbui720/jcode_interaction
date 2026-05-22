use regex::Regex;
use std::sync::{Mutex, OnceLock};

static LAST_CURSOR: OnceLock<Mutex<Option<(i32, i32)>>> = OnceLock::new();

pub fn remember_cursor(pos: (i32, i32)) {
    if pos.0 <= 2 && pos.1 <= 2 {
        return;
    }
    let cache = LAST_CURSOR.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cache.lock() {
        *guard = Some(pos);
    }
}

pub fn last_cursor() -> Option<(i32, i32)> {
    LAST_CURSOR
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|guard| *guard)
}

pub fn parse_xdotool_mouselocation(output: &str) -> (Option<i32>, Option<i32>) {
    let (x, y, _) = parse_xdotool_mouselocation_full(output);
    (x, y)
}

pub fn parse_xdotool_mouselocation_full(output: &str) -> (Option<i32>, Option<i32>, String) {
    let xy = Regex::new(r"x:(-?\d+)\s+y:(-?\d+)")
        .unwrap()
        .captures(output)
        .and_then(|caps| Some((caps[1].parse().ok()?, caps[2].parse().ok()?)))
        .or_else(|| {
            Some((
                parse_shell_var(output, "X")? as i32,
                parse_shell_var(output, "Y")? as i32,
            ))
        });
    let window = Regex::new(r"window:([^\s]+)")
        .unwrap()
        .captures(output)
        .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        .or_else(|| parse_shell_var(output, "WINDOW").map(|v| v.to_string()))
        .unwrap_or_default();
    let (x, y) = xy.map(|(x, y)| (Some(x), Some(y))).unwrap_or((None, None));
    (x, y, window)
}

pub fn parse_shell_var(output: &str, key: &str) -> Option<i64> {
    output.lines().find_map(|line| {
        let (left, right) = line.split_once('=')?;
        (left.trim() == key)
            .then(|| right.trim().parse().ok())
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_mouse_location() {
        assert_eq!(
            parse_xdotool_mouselocation("x:2657 y:50 screen:0 window:8388629"),
            (Some(2657), Some(50))
        );
        assert_eq!(parse_xdotool_mouselocation("bad"), (None, None));
        assert_eq!(
            parse_xdotool_mouselocation_full("x:2657 y:50 screen:0 window:8388629"),
            (Some(2657), Some(50), "8388629".into())
        );
        assert_eq!(
            parse_xdotool_mouselocation("X=2657\nY=50\nSCREEN=0\nWINDOW=8388629"),
            (Some(2657), Some(50))
        );
    }
}
