use regex::Regex;

pub fn parse_xdotool_mouselocation(output: &str) -> (Option<i32>, Option<i32>) {
    let (x, y, _) = parse_xdotool_mouselocation_full(output);
    (x, y)
}

pub fn parse_xdotool_mouselocation_full(output: &str) -> (Option<i32>, Option<i32>, String) {
    let re = Regex::new(r"x:(-?\d+)\s+y:(-?\d+)").unwrap();
    let Some(caps) = re.captures(output) else { return (None, None, String::new()); };
    let window = Regex::new(r"window:([^\s]+)").unwrap().captures(output).and_then(|c| c.get(1).map(|m| m.as_str().to_string())).unwrap_or_default();
    (caps[1].parse().ok(), caps[2].parse().ok(), window)
}

#[cfg(test)]
mod tests { use super::*; #[test] fn parses_mouse_location() { assert_eq!(parse_xdotool_mouselocation("x:2657 y:50 screen:0 window:8388629"), (Some(2657), Some(50))); assert_eq!(parse_xdotool_mouselocation("bad"), (None, None)); assert_eq!(parse_xdotool_mouselocation_full("x:2657 y:50 screen:0 window:8388629"), (Some(2657), Some(50), "8388629".into())); } }
