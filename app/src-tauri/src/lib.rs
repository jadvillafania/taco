use tauri::{
    menu::{Menu, MenuItem, CheckMenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};
use tauri_plugin_autostart::ManagerExt;

mod wslpath;
mod payload;
mod retention;
mod capture;
mod commands;
mod deployer;
mod history;
mod hotkeys;
mod sessions;
mod settings;
mod tier1;
mod winpos;

/// Opened on first run and from the tray — the app has no persistent window, so
/// without this an install ends with nothing on screen but a tray icon.
fn open_welcome(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("welcome") {
        w.show().ok();
        w.set_focus().ok();
    } else {
        tauri::WebviewWindowBuilder::new(app, "welcome", tauri::WebviewUrl::App("index.html?window=welcome".into()))
            .title("Welcome to Taco")
            .inner_size(500.0, 620.0)
            .min_inner_size(420.0, 420.0)
            .visible(false)
            .build()
            .ok();
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        use tauri::Manager;
                        let Some(hk) = app.try_state::<crate::hotkeys::Hotkeys>() else { return };
                        if shortcut == &*hk.window.lock().unwrap() {
                            crate::commands::start_window_capture(app);
                        } else if shortcut == &*hk.clipboard.lock().unwrap() {
                            crate::commands::start_clipboard_capture(app);
                        } else if shortcut == &*hk.region.lock().unwrap() {
                            crate::commands::start_region_capture(app);
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            commands::get_frame,
            commands::region_selected,
            commands::cancel_capture,
            commands::cancel_overlay,
            commands::trigger_capture,
            commands::get_captures,
            commands::remove_capture,
            commands::import_clipboard,
            commands::import_file,
            commands::send_capture,
            commands::save_annotated,
            commands::get_capture_data_url,
            sessions::list_sessions_cmd,
            history::list_captures,
            history::resend_capture,
            history::delete_capture,
            history::clear_captures,
            settings::get_settings,
            settings::set_settings,
            settings::get_default_settings,
            hotkeys::probe_hotkey,
            deployer::install_wsl_shim,
            deployer::remove_wsl_shim,
            deployer::install_native_shim,
            deployer::remove_native_shim,
            deployer::shim_status
        ])
        .manage(capture::CaptureState(Default::default()))
        .manage(commands::ComposerGeom::default())
        .setup(|app| {
            let dir = crate::retention::data_dir(app.handle());
            // No settings file yet == first launch after install. Writing the defaults now
            // is what marks the install as onboarded (a failed write just re-shows it).
            let first_run = !dir.join("settings.json").exists();
            let s = crate::settings::load(&dir);
            if first_run {
                let _ = crate::settings::save(&dir, &s);
            }

            let retention_hours = s.retention_hours;
            let captures = dir.join("captures");
            std::thread::spawn(move || {
                let _ = crate::retention::sweep(
                    &captures,
                    std::time::Duration::from_secs(retention_hours * 3600),
                    std::time::SystemTime::now(),
                );
            });

            let region = MenuItem::with_id(
                app, "capture_region", "Capture Region\tCtrl+Shift+Space", true, None::<&str>,
            )?;
            let screen = MenuItem::with_id(app, "capture_screen", "Capture Screen", true, None::<&str>)?;
            let window = MenuItem::with_id(app, "capture_window", "Capture Active Window\tCtrl+Alt+Space", true, None::<&str>)?;
            let clip = MenuItem::with_id(app, "capture_clipboard", "Send Clipboard Image\tCtrl+Alt+V", true, None::<&str>)?;
            let open_comp = MenuItem::with_id(app, "open_composer", "Open Composer…", true, None::<&str>)?;
            let history = MenuItem::with_id(app, "history", "Capture History…", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let auto_on = app.autolaunch().is_enabled().unwrap_or(false);
            let autostart = CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, auto_on, None::<&str>)?;
            let autostart_handle = autostart.clone();
            let welcome = MenuItem::with_id(app, "welcome", "Getting Started…", true, None::<&str>)?;
            let about = MenuItem::with_id(app, "about", "About Taco", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[
                &open_comp, &region, &screen, &window, &clip,
                &PredefinedMenuItem::separator(app)?,
                &history, &settings,
                &PredefinedMenuItem::separator(app)?,
                &autostart,
                &PredefinedMenuItem::separator(app)?,
                &welcome, &about, &quit,
            ])?;

            app.manage(crate::hotkeys::TrayLabels {
                region: region.clone(),
                window: window.clone(),
                clipboard: clip.clone(),
            });

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Taco")
                .menu(&menu)
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "capture_region" => crate::commands::start_region_capture(app),
                    "capture_screen" => crate::commands::start_screen_capture(app),
                    "capture_window" => crate::commands::start_window_capture(app),
                    "capture_clipboard" => crate::commands::start_clipboard_capture(app),
                    "open_composer" => {
                        let focus = crate::sessions::foreground_title();
                        app.state::<crate::capture::CaptureState>().0.lock().unwrap().focus_title = focus;
                        crate::commands::ensure_composer(app);
                    }
                    "history" => {
                        if let Some(w) = app.get_webview_window("history") {
                            w.show().ok();
                            w.set_focus().ok();
                        } else {
                            tauri::WebviewWindowBuilder::new(app, "history", tauri::WebviewUrl::App("index.html?window=history".into()))
                                .title("Taco: Capture History")
                                .inner_size(560.0, 480.0)
                                .visible(false)
                                .build()
                                .ok();
                        }
                    }
                    "settings" => {
                        if let Some(w) = app.get_webview_window("settings") {
                            w.show().ok();
                            w.set_focus().ok();
                        } else {
                            tauri::WebviewWindowBuilder::new(app, "settings", tauri::WebviewUrl::App("index.html?window=settings".into()))
                                .title("Taco: Settings")
                                .inner_size(420.0, 620.0)
                                .visible(false)
                                .build()
                                .ok();
                        }
                    }
                    "welcome" => open_welcome(app),
                    "about" => {
                        if let Some(w) = app.get_webview_window("about") {
                            w.show().ok();
                            w.set_focus().ok();
                        } else {
                            tauri::WebviewWindowBuilder::new(app, "about", tauri::WebviewUrl::App("index.html?window=about".into()))
                                .title("About Taco")
                                .inner_size(440.0, 240.0)
                                .resizable(false)
                                .maximizable(false)
                                .minimizable(false)
                                .visible(false)
                                .build()
                                .ok();
                        }
                    }
                    "autostart" => {
                        let al = app.autolaunch();
                        let was_enabled = al.is_enabled().unwrap_or(false);
                        let res = if was_enabled { al.disable() } else { al.enable() };
                        match res {
                            Ok(()) => {
                                let new_state = !was_enabled;
                                let _ = autostart_handle.set_checked(new_state);
                            }
                            Err(e) => {
                                crate::commands::notify(app, "Autostart change failed", &e.to_string());
                            }
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            crate::hotkeys::apply(app.handle(), &s);

            if first_run {
                open_welcome(app.handle());
            }

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error building tauri app")
        .run(|_app, event| {
            // no persistent windows: keep the process alive when overlay/composer close,
            // but let app.exit(0) (code is Some) actually quit
            if let RunEvent::ExitRequested { api, code, .. } = event {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
        });
}
