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

/// Well-known Windows / ubiquitous app shortcuts. No OS API exposes these —
/// this is a curated table (same approach PowerToys takes). (accelerator, label, blocked)
const KNOWN_CHORDS: &[(&str, &str, bool)] = &[
    // blocked: breaks the OS or cannot function as a global hotkey
    ("Alt+F4", "Windows — close window", true),
    ("Alt+Tab", "Windows — switch windows", true),
    ("Alt+Space", "Windows — window menu", true),
    ("Ctrl+Esc", "Windows — Start menu", true),
    ("Ctrl+Shift+Esc", "Windows — Task Manager", true),
    // warn: famous multi-modifier app shortcuts
    ("Ctrl+Shift+T", "browsers — reopen closed tab", false),
    ("Ctrl+Shift+N", "browsers — incognito window", false),
    ("Ctrl+Shift+P", "browsers/editors — private window / command palette", false),
    ("Ctrl+Shift+I", "browsers — developer tools", false),
    ("Ctrl+Shift+C", "terminals/devtools — copy / inspect", false),
    ("Ctrl+Shift+V", "editors/terminals — paste without formatting", false),
    ("Ctrl+Shift+Z", "editors — redo", false),
    ("Ctrl+Shift+S", "editors — save as", false),
    ("Ctrl+Shift+F", "editors — find in files", false),
    ("Alt+Enter", "Windows — properties / fullscreen", false),
    ("Ctrl+Tab", "apps — next tab", false),
    ("F1", "apps — help", false), // unreachable via the recorder (requires a modifier); kept for hand-edited settings probes
];

pub fn classify_chord(sc: &Shortcut) -> Option<(String, bool)> {
    for (accel, label, blocked) in KNOWN_CHORDS {
        if let Ok(known) = parse_shortcut(accel) {
            if known == *sc {
                return Some((label.to_string(), *blocked));
            }
        }
    }
    None
}

/// One modifier + a non-function key is almost always some app's accelerator
/// (Ctrl+C = copy). Function keys are conventionally safe with a single modifier.
pub fn is_single_modifier_footgun(sc: &Shortcut) -> bool {
    use tauri_plugin_global_shortcut::Modifiers;
    let count = [Modifiers::CONTROL, Modifiers::ALT, Modifiers::SHIFT, Modifiers::SUPER]
        .iter()
        .filter(|m| sc.mods.contains(**m))
        .count();
    let is_fn_key = format!("{:?}", sc.key).starts_with('F')
        && format!("{:?}", sc.key).len() <= 3
        && format!("{:?}", sc.key)[1..].chars().all(|c| c.is_ascii_digit());
    count <= 1 && !is_fn_key
}

#[derive(serde::Serialize)]
pub struct ProbeVerdict {
    pub level: String,   // "ok" | "warn" | "block"
    pub message: String, // empty when ok
}

fn verdict(level: &str, message: impl Into<String>) -> Result<ProbeVerdict, String> {
    Ok(ProbeVerdict { level: level.into(), message: message.into() })
}

/// Probe a chord for collisions and known-shortcut conflicts, returning a tiered verdict.
/// Windows cannot enumerate hotkey owners, so external collisions are detected by trial
/// registration (register, then immediately unregister). `exclude` names the slot being
/// recorded ("region"|"window"|"clipboard") so re-recording an action's own current chord
/// doesn't count as a collision.
#[tauri::command]
pub fn probe_hotkey(app: tauri::AppHandle, binding: String, exclude: String) -> Result<ProbeVerdict, String> {
    use tauri::Manager;
    let sc = parse_shortcut(&binding)?;

    if let Some(hk) = app.try_state::<Hotkeys>() {
        let ours: [(&std::sync::Mutex<Shortcut>, &str, &str); 3] = [
            (&hk.region, "region", "Capture Region"),
            (&hk.window, "window", "Capture Active Window"),
            (&hk.clipboard, "clipboard", "Send Clipboard Image"),
        ];
        for (slot, key, label) in ours {
            if key != exclude && *slot.lock().unwrap() == sc {
                return verdict("warn", format!("already used by {label}"));
            }
            if key == exclude && *slot.lock().unwrap() == sc {
                return verdict("ok", ""); // unchanged binding for this slot
            }
        }
    }

    if let Some((label, blocked)) = classify_chord(&sc) {
        if blocked {
            return verdict("block", format!("reserved by Windows — {label}"));
        }
        return verdict("warn", format!("commonly used: {label}"));
    }

    if is_single_modifier_footgun(&sc) {
        return verdict(
            "warn",
            "single-modifier shortcuts usually collide with app shortcuts (e.g. Ctrl+C = copy) — consider adding a second modifier",
        );
    }

    let gs = app.global_shortcut();
    if gs.register(sc).is_ok() {
        gs.unregister(sc).ok();
        verdict("ok", "")
    } else {
        verdict("warn", "taken by another app")
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

    #[test]
    fn known_table_fully_parses() {
        for (accel, _, _) in KNOWN_CHORDS {
            assert!(parse_shortcut(accel).is_ok(), "table entry must parse: {accel}");
        }
    }

    #[test]
    fn classifies_reserved_and_common() {
        let esc = parse_shortcut("Ctrl+Shift+Esc").unwrap();
        let (label, blocked) = classify_chord(&esc).unwrap();
        assert!(blocked);
        assert!(label.contains("Task Manager"));

        let t = parse_shortcut("Ctrl+Shift+T").unwrap();
        let (label, blocked) = classify_chord(&t).unwrap();
        assert!(!blocked);
        assert!(label.to_lowercase().contains("tab"));

        assert!(classify_chord(&parse_shortcut("Ctrl+Alt+V").unwrap()).is_none());
    }

    #[test]
    fn detects_single_modifier_footguns() {
        assert!(is_single_modifier_footgun(&parse_shortcut("Ctrl+C").unwrap()));
        assert!(is_single_modifier_footgun(&parse_shortcut("Alt+Space").unwrap())); // table-blocked anyway, but the heuristic alone would also flag it
        assert!(!is_single_modifier_footgun(&parse_shortcut("Ctrl+Shift+Space").unwrap()));
        assert!(!is_single_modifier_footgun(&parse_shortcut("Ctrl+F9").unwrap())); // fn keys are safe with one modifier
    }
}
