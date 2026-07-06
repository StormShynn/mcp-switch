mod adapter;
mod atomic;
mod commands;
mod paths;
mod store;
mod types;

use commands::{import_servers, list_servers, toggle_server};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            list_servers,
            toggle_server,
            import_servers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
