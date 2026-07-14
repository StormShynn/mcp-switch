mod adapter;
mod app_control;
mod atomic;
mod commands;
mod mcp_json;
mod mcp_test;
mod paths;
mod store;
mod types;
mod winshim;

use commands::{
    delete_server_forever, get_store_path, import_servers, list_servers, restart_app,
    restore_server, save_server, test_server_connection, toggle_server, trash_server,
};

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
            trash_server,
            restore_server,
            delete_server_forever,
            save_server,
            restart_app,
            test_server_connection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
