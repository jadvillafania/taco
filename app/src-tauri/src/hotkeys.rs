use std::str::FromStr;
use std::sync::Mutex;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

pub fn parse_shortcut(s: &str) -> Result<Shortcut, String> {
    Shortcut::from_str(s).map_err(|e| format!("invalid hotkey '{s}': {e}"))
}

pub struct Hotkeys {
    pub region: Mutex<Shortcut>,
    pub window: Mutex<Shortcut>,
    pub clipboard: Mutex<Shortcut>,
}

/// (Re)register the three capture hotkeys from settings. A binding that fails to parse or
/// register falls back to that action's built-in default; failures surface via notify.
// ponytail: tray menu accelerator labels ("...\tCtrl+Shift+Space") are built once and go
// stale after a remap; live tray-label updates when someone complains.
pub fn apply(app: &tauri::AppHandle, s: &crate::settings::Settings) {
    use tauri::Manager;
    let gs = app.global_shortcut();
    gs.unregister_all().ok();

    let defaults = crate::settings::Settings::default();
    let resolve = |wanted: &str, fallback: &str, label: &str| -> Shortcut {
        let (sc, wanted_valid) = match parse_shortcut(wanted) {
            Ok(sc) => (sc, true),
            Err(e) => {
                crate::commands::notify(app, "Hotkey invalid", &format!("{label}: {e} — using {fallback}"));
                (parse_shortcut(fallback).expect("default hotkey parses"), false)
            }
        };
        if gs.register(sc).is_err() {
            if wanted_valid {
                crate::commands::notify(
                    app,
                    "Hotkey unavailable",
                    &format!("{label}: '{wanted}' is taken by another app — using {fallback}"),
                );
                let fb = parse_shortcut(fallback).expect("default hotkey parses");
                gs.register(fb).ok(); // if even the default is taken, the tray menu still works
                return fb;
            }
            // wanted was invalid syntax (already notified above); sc IS the fallback and its
            // registration just failed too — don't retry the same registration a second time.
            crate::commands::notify(
                app,
                "Hotkey unavailable",
                &format!("{label}: default '{fallback}' could not be registered"),
            );
            return sc;
        }
        sc
    };

    let region = resolve(&s.hotkey_region, &defaults.hotkey_region, "Capture Region");
    let window = resolve(&s.hotkey_window, &defaults.hotkey_window, "Capture Active Window");
    let clipboard = resolve(&s.hotkey_clipboard, &defaults.hotkey_clipboard, "Send Clipboard Image");

    match app.try_state::<Hotkeys>() {
        Some(hk) => {
            *hk.region.lock().unwrap() = region;
            *hk.window.lock().unwrap() = window;
            *hk.clipboard.lock().unwrap() = clipboard;
        }
        None => {
            app.manage(Hotkeys {
                region: Mutex::new(region),
                window: Mutex::new(window),
                clipboard: Mutex::new(clipboard),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_defaults_and_rejects_junk() {
        for s in ["Ctrl+Shift+Space", "Ctrl+Alt+Space", "Ctrl+Alt+V"] {
            assert!(parse_shortcut(s).is_ok(), "should parse: {s}");
        }
        assert!(parse_shortcut("NotAKey+Q").is_err());
        assert!(parse_shortcut("").is_err());
    }
}
