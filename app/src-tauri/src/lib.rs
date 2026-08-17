use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    RunEvent,
};

mod wslpath;
mod payload;
mod retention;
mod capture;
mod commands;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|_app, _args, _cwd| {}))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        crate::commands::start_region_capture(app);
                    }
                })
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::get_frame,
            commands::region_selected,
            commands::cancel_capture
        ])
        .manage(capture::CaptureState(Default::default()))
        .setup(|app| {
            use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
            app.global_shortcut()
                .register(Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space))?;

            let captures = crate::retention::data_dir(app.handle()).join("captures");
            std::thread::spawn(move || {
                let _ = crate::retention::sweep(
                    &captures,
                    std::time::Duration::from_secs(24 * 3600),
                    std::time::SystemTime::now(),
                ); // ponytail: fixed 24h retention; settings UI when someone asks
            });

            let region = MenuItem::with_id(
                app, "capture_region", "Capture Region\tCtrl+Shift+Space", true, None::<&str>,
            )?;
            let screen = MenuItem::with_id(app, "capture_screen", "Capture Screen", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Exit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&region, &screen, &quit])?;
            TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Developer Visual Companion")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "capture_region" => crate::commands::start_region_capture(app),
                    "capture_screen" => { /* Task 9 */ }
                    _ => {}
                })
                .build(app)?;
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
