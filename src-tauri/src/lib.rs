mod aligner;
mod audio;
mod commands;
mod cover;
mod filesystem;
mod logger;
mod metadata;
mod models;
mod parser;
mod transaction;

use tauri::menu::{MenuBuilder, MenuItemBuilder, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logger::init_logger();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let check_updates = MenuItemBuilder::with_id("check_updates", "检查更新").build(app)?;
            let quit = PredefinedMenuItem::quit(app, Some("退出应用"))?;
            let menu = MenuBuilder::new(app)
                .items(&[&check_updates, &quit])
                .build()?;

            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("momotag")
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "check_updates" {
                        let _ = app.emit("tray://check-for-updates", ());
                    }
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }
            tray.build(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::align_tracks,
            commands::apply_album_metadata,
            commands::load_cover_preview,
            commands::parse_tracklist,
            commands::scan_album_folder,
            commands::suggest_album_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
