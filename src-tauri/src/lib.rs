mod audio;
mod commands;
mod cover;
mod filesystem;
mod metadata;
mod models;
mod parser;
mod transaction;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::apply_album_metadata,
            commands::load_cover_preview,
            commands::parse_tracklist,
            commands::scan_album_folder,
            commands::suggest_album_metadata
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
