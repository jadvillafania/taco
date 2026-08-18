/// C:\foo\bar -> /mnt/c/foo/bar. None for UNC/relative paths.
pub fn to_wsl_path(win: &str) -> Option<String> {
    let bytes = win.as_bytes();
    if bytes.len() < 3 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || (bytes[2] != b'\\' && bytes[2] != b'/') {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_lowercase();
    let rest = win[3..].replace('\\', "/");
    Some(format!("/mnt/{}/{}", drive, rest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_drive_path() {
        assert_eq!(
            to_wsl_path(r"C:\Users\jv\AppData\Local\x\cap.png").as_deref(),
            Some("/mnt/c/Users/jv/AppData/Local/x/cap.png")
        );
    }

    #[test]
    fn lowercases_drive_letter() {
        assert_eq!(to_wsl_path(r"D:\a.png").as_deref(), Some("/mnt/d/a.png"));
    }

    #[test]
    fn rejects_unc_and_relative() {
        assert_eq!(to_wsl_path(r"\\wsl$\Ubuntu\home\x.png"), None);
        assert_eq!(to_wsl_path(r"captures\x.png"), None);
    }
}
