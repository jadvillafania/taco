use tauri::{
    menu::{Menu, MenuItem, CheckMenuItem},
    tray::TrayIconBuilder,
    Manager, RunEvent,
};

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
            settings::set_settings
        ])
        .manage(capture::CaptureState(Default::default()))
        .manage(commands::ComposerGeom::default())
        .setup(|app| {
            let s = crate::settings::load(&crate::retention::data_dir(app.handle()));

            let retention_hours = s.retention_hours;
            let captures = crate::retention::data_dir(app.handle()).join("captures");
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
            let install_shim = MenuItem::with_id(app, "install_shim", "Install WSL Shim…", true, None::<&str>)?;
            let remove_shim = MenuItem::with_id(app, "remove_shim", "Remove WSL Shim", true, None::<&str>)?;
            use tauri_plugin_autostart::ManagerExt;
            let auto_on = app.autolaunch().is_enabled().unwrap_or(false);
            let autostart = CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, auto_on, None::<&str>)?;
            let autostart_handle = autostart.clone();
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&region, &screen, &window, &clip, &open_comp, &history, &settings, &install_shim, &remove_shim, &autostart, &quit])?;

            app.manage(crate::hotkeys::TrayLabels {
                region: region.clone(),
                window: window.clone(),
                clipboard: clip.clone(),
            });

            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Developer Visual Companion")
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
                                .title("Capture History")
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
                                .title("Settings")
                                .inner_size(420.0, 460.0)
                                .visible(false)
                                .build()
                                .ok();
                        }
                    }
                    "install_shim" => {
                        use tauri_plugin_dialog::{DialogExt, MessageDialogButtons};
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let ok = app.dialog().message(
                                "This copies dvc-shim into your WSL distribution (~/.local/share/dvc/) and adds a 'claude' alias to ~/.bashrc so sessions support instant delivery.\n\nBoth changes are reversible via 'Remove WSL Shim'.")
                                .title("Install WSL shim?")
                                .buttons(MessageDialogButtons::OkCancelCustom("Install".into(), "Cancel".into()))
                                .blocking_show();
                            if !ok { return; }
                            let result = crate::deployer::default_distro()
                                .and_then(|d| crate::deployer::install(&app, &d));
                            match result {
                                Ok(()) => crate::commands::notify(&app, "Shim installed", "Restart your terminal, then run 'claude' as usual."),
                                Err(e) => crate::commands::notify(&app, "Shim install failed", &e),
                            }
                        });
                    }
                    "remove_shim" => {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let result = crate::deployer::default_distro().and_then(|d| crate::deployer::remove(&d));
                            match result {
                                Ok(()) => crate::commands::notify(&app, "Shim removed", "Alias and binary deleted."),
                                Err(e) => crate::commands::notify(&app, "Shim removal failed", &e),
                            }
                        });
                    }
                    "autostart" => {
                        use tauri_plugin_autostart::ManagerExt;
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
