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

/// Tray menu items whose accelerator labels must stay in sync with the hotkeys actually
/// registered by `apply`.
pub struct TrayLabels {
    pub region: tauri::menu::MenuItem<tauri::Wry>,
    pub window: tauri::menu::MenuItem<tauri::Wry>,
    pub clipboard: tauri::menu::MenuItem<tauri::Wry>,
}

/// (Re)register the three capture hotkeys from settings. A binding that fails to parse or
/// register falls back to that action's built-in default; failures surface via notify.
pub fn apply(app: &tauri::AppHandle, s: &crate::settings::Settings) {
    use tauri::Manager;
    let gs = app.global_shortcut();
    gs.unregister_all().ok();

    let defaults = crate::settings::Settings::default();
    let resolve = |wanted: &str, fallback: &str, label: &str| -> (Shortcut, String) {
        let (sc, wanted_valid, mut winning) = match parse_shortcut(wanted) {
            Ok(sc) => (sc, true, wanted.to_string()),
            Err(e) => {
                crate::commands::notify(app, "Hotkey invalid", &format!("{label}: {e} — using {fallback}"));
                (parse_shortcut(fallback).expect("default hotkey parses"), false, fallback.to_string())
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
                winning = fallback.to_string();
                return (fb, winning);
            }
            // wanted was invalid syntax (already notified above); sc IS the fallback and its
            // registration just failed too — don't retry the same registration a second time.
            // `winning` is already `fallback` from the parse-fail branch above.
            crate::commands::notify(
                app,
                "Hotkey unavailable",
                &format!("{label}: default '{fallback}' could not be registered"),
            );
            // sc failed to register, but the label still shows the default binding — the tray
            // click handler falls through to nothing for this action until settings are fixed.
            return (sc, winning);
        }
        (sc, winning)
    };

    let (region, region_label) = resolve(&s.hotkey_region, &defaults.hotkey_region, "Capture Region");
    let (window, window_label) = resolve(&s.hotkey_window, &defaults.hotkey_window, "Capture Active Window");
    let (clipboard, clipboard_label) = resolve(&s.hotkey_clipboard, &defaults.hotkey_clipboard, "Send Clipboard Image");

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

    if let Some(labels) = app.try_state::<TrayLabels>() {
        labels.region.set_text(format!("Capture Region\t{region_label}")).ok();
        labels.window.set_text(format!("Capture Active Window\t{window_label}")).ok();
        labels.clipboard.set_text(format!("Send Clipboard Image\t{clipboard_label}")).ok();
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
