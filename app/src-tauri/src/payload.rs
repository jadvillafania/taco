pub const DEFAULT_INSTRUCTION: &str = "Analyze this screenshot in the context of the current task.";

pub fn build_payload(message: Option<&str>, wsl_paths: &[String], default_instruction: &str) -> String {
    let fallback = if default_instruction.trim().is_empty() { DEFAULT_INSTRUCTION } else { default_instruction };
    let msg = message.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(fallback);
    format!("{}\n{}", msg, wsl_paths.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(paths: &[&str]) -> Vec<String> {
        paths.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn message_then_path() {
        assert_eq!(
            build_payload(Some("Why misaligned?"), &p(&["/mnt/c/x/cap.png"]), DEFAULT_INSTRUCTION),
            "Why misaligned?\n/mnt/c/x/cap.png"
        );
    }

    #[test]
    fn default_instruction_when_empty_or_none() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, &p(&["/mnt/c/x/cap.png"]), DEFAULT_INSTRUCTION), want);
        assert_eq!(build_payload(Some("   "), &p(&["/mnt/c/x/cap.png"]), DEFAULT_INSTRUCTION), want);
    }

    #[test]
    fn blank_custom_default_falls_back() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, &p(&["/mnt/c/x/cap.png"]), "  "), want);
    }

    #[test]
    fn multi_path_payload_lists_all_paths() {
        let paths = vec!["/mnt/c/x/a.png".to_string(), "/mnt/c/x/b.png".to_string()];
        assert_eq!(
            build_payload(Some("Compare these."), &paths, DEFAULT_INSTRUCTION),
            "Compare these.\n/mnt/c/x/a.png\n/mnt/c/x/b.png"
        );
    }
}
