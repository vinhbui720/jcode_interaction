use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub ok: bool,
    pub message: String,
    pub fix: String,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticsReport {
    pub checks: Vec<CheckResult>,
}
impl DiagnosticsReport {
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }
    pub fn as_text(&self) -> String {
        let mut lines = vec!["jcode-panel diagnostics".to_string()];
        for c in &self.checks {
            lines.push(format!(
                "[{}] {}: {}",
                if c.ok { "OK" } else { "FAIL" },
                c.name,
                c.message
            ));
            if !c.ok && !c.fix.is_empty() {
                lines.push(format!("      fix: {}", c.fix));
            }
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagnostics_report_text_and_status() {
        let report = DiagnosticsReport {
            checks: vec![
                CheckResult {
                    name: "a".into(),
                    ok: true,
                    message: "ok".into(),
                    fix: "".into(),
                },
                CheckResult {
                    name: "b".into(),
                    ok: false,
                    message: "bad".into(),
                    fix: "fix it".into(),
                },
            ],
        };
        assert!(!report.ok());
        let text = report.as_text();
        assert!(text.contains("[OK] a"));
        assert!(text.contains("fix: fix it"));
    }
}
