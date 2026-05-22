pub fn render_command(template: &str, command: &str) -> Vec<String> {
    shell_words::split(
        &template
            .replace("{cmd}", command)
            .replace("{quoted_cmd}", &shell_quote(command)),
    )
    .unwrap_or_default()
}

pub fn shell_quote(command: &str) -> String {
    if command
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "@%_+=:,./-".contains(c))
    {
        command.into()
    } else {
        format!("'{}'", command.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terminal_template_rendering() {
        assert_eq!(
            render_command("xterm -e sh -lc {quoted_cmd}", "echo hi"),
            vec!["xterm", "-e", "sh", "-lc", "echo hi"]
        );
    }
}
