use std::path::Path;

pub fn load(dir: &Path) -> Option<(i32, i32)> {
    let txt = std::fs::read_to_string(dir.join("composer-pos.json")).ok()?;
    let v: serde_json::Value = serde_json::from_str(&txt).ok()?;
    Some((v["x"].as_i64()? as i32, v["y"].as_i64()? as i32))
}

pub fn save(dir: &Path, x: i32, y: i32) {
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(dir.join("composer-pos.json"), format!(r#"{{"x":{x},"y":{y}}}"#));
}

/// monitors: (x, y, width, height) per monitor, physical px
pub fn on_any_monitor(x: i32, y: i32, monitors: &[(i32, i32, u32, u32)]) -> bool {
    monitors.iter().any(|&(mx, my, mw, mh)| {
        x >= mx && y >= my && x < mx + mw as i32 && y < my + mh as i32
    })
}

/// True only if BOTH the top-left corner (x, y) and the bottom-right corner
/// (x + width - 1, y + height - 1) of a `width` x `height` window anchored at (x, y) land on
/// some monitor. Guards against restoring a remembered top-left that leaves most of the window
/// off-screen (e.g. x near a monitor's right edge).
pub fn rect_on_any_monitor(x: i32, y: i32, width: i32, height: i32, monitors: &[(i32, i32, u32, u32)]) -> bool {
    on_any_monitor(x, y, monitors) && on_any_monitor(x + width - 1, y + height - 1, monitors)
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: cargo runs tests in this file on separate threads within the same process, so a
    // tempdir keyed only by std::process::id() (as the brief's snippet suggests) collides across
    // tests. Each test appends its own name to keep directories disjoint while still rooting them
    // under the process id, per the brief's tempdir strategy.
    fn tempdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("dvc-pos-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempdir("roundtrip");
        save(&dir, 123, 456);
        assert_eq!(load(&dir), Some((123, 456)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_load_roundtrip_negative_coords() {
        let dir = tempdir("roundtrip-negative");
        save(&dir, -10, -20);
        assert_eq!(load(&dir), Some((-10, -20)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_is_none() {
        let dir = tempdir("missing");
        std::fs::remove_dir_all(&dir).ok(); // ensure no composer-pos.json under this dir
        assert_eq!(load(&dir), None);
    }

    #[test]
    fn load_corrupt_file_is_none() {
        let dir = tempdir("corrupt");
        std::fs::write(dir.join("composer-pos.json"), "not json").unwrap();
        assert_eq!(load(&dir), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn on_any_monitor_inside_one_of_two_is_true() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        assert!(on_any_monitor(100, 100, &monitors));
        assert!(on_any_monitor(2000, 100, &monitors));
    }

    #[test]
    fn on_any_monitor_outside_all_is_false() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 1920, 1080)];
        assert!(!on_any_monitor(-5, 100, &monitors));
        assert!(!on_any_monitor(5000, 100, &monitors));
    }

    #[test]
    fn on_any_monitor_at_right_bottom_edge_is_false() {
        let monitors = [(0, 0, 1920, 1080)];
        assert!(!on_any_monitor(1920, 100, &monitors));
        assert!(!on_any_monitor(100, 1080, &monitors));
    }

    #[test]
    fn rect_on_any_monitor_true_when_fully_inside() {
        let monitors = [(0, 0, 1920, 1080)];
        assert!(rect_on_any_monitor(100, 100, 420, 380, &monitors));
    }

    #[test]
    fn rect_on_any_monitor_false_when_bottom_right_off_screen() {
        let monitors = [(0, 0, 1920, 1080)];
        // top-left (1900, 100) is on the monitor, but the bottom-right corner at
        // x + width - 1 = 2319 is well past the 1920px-wide monitor.
        assert!(!rect_on_any_monitor(1900, 100, 420, 380, &monitors));
    }

    #[test]
    fn rect_on_any_monitor_false_when_top_left_off_screen() {
        let monitors = [(0, 0, 1920, 1080)];
        assert!(!rect_on_any_monitor(-50, 100, 420, 380, &monitors));
    }
}
