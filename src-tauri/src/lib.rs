mod adapter;
mod atomic;
mod commands;
mod mcp_json;
mod paths;
mod store;
mod types;
mod winshim;

use commands::{get_store_path, import_servers, list_servers, toggle_server};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            list_servers,
            toggle_server,
            import_servers,
            get_store_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
