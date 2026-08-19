pub const DEFAULT_INSTRUCTION: &str = "Analyze this screenshot in the context of the current task.";

pub fn build_payload(message: Option<&str>, wsl_path: &str, default_instruction: &str) -> String {
    let default_instruction = default_instruction.trim();
    let default_instruction = if default_instruction.is_empty() { DEFAULT_INSTRUCTION } else { default_instruction };
    let msg = message.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(default_instruction);
    format!("{}\n{}", msg, wsl_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    const P: &str = "/mnt/c/x/cap.png";

    #[test]
    fn message_then_path() {
        assert_eq!(build_payload(Some("Why misaligned?"), P, DEFAULT_INSTRUCTION), "Why misaligned?\n/mnt/c/x/cap.png");
    }

    #[test]
    fn default_instruction_when_empty_or_none() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, P, DEFAULT_INSTRUCTION), want);
        assert_eq!(build_payload(Some("   "), P, DEFAULT_INSTRUCTION), want);
    }

    #[test]
    fn blank_custom_default_falls_back() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, P, "  "), want);
    }
}
