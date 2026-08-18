const DEFAULT_INSTRUCTION: &str = "Analyze this screenshot in the context of the current task.";

pub fn build_payload(message: Option<&str>, wsl_path: &str) -> String {
    let msg = message.map(str::trim).filter(|m| !m.is_empty()).unwrap_or(DEFAULT_INSTRUCTION);
    format!("{}\n{}", msg, wsl_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    const P: &str = "/mnt/c/x/cap.png";

    #[test]
    fn message_then_path() {
        assert_eq!(build_payload(Some("Why misaligned?"), P), "Why misaligned?\n/mnt/c/x/cap.png");
    }

    #[test]
    fn default_instruction_when_empty_or_none() {
        let want = "Analyze this screenshot in the context of the current task.\n/mnt/c/x/cap.png";
        assert_eq!(build_payload(None, P), want);
        assert_eq!(build_payload(Some("   "), P), want);
    }
}
