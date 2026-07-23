mod adapter;
mod app_control;
mod atomic;
mod commands;
mod mcp_json;
mod mcp_test;
mod paths;
mod runner;
mod store;
mod tray;
mod types;
mod winshim;

use tauri::Manager;

use commands::{
    delete_server_forever, export_servers, get_app_config_path, get_store_path,
    import_servers, import_servers_from_file, list_running, list_servers, read_log,
    get_auto_run, restart_app, restore_server, save_server, set_auto_run, start_server, stop_server,
    test_server_connection, toggle_server, trash_server,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = runner::RunnerState::default();
            app.manage(state);
            let handle = app.handle().clone();
            tray::build(&handle)?;
            tray::hook_close_to_tray(&handle);
            // Spawn everything marked auto-run as soon as the store is readable.
            let h2 = handle.clone();
            std::thread::spawn(move || {
                match store::list_servers() {
                    Ok(s) => {
                        if let Some(state) = h2.try_state::<runner::RunnerState>() {
                            state.start_auto_run(&h2, &s.servers);
                        }
                    }
                    Err(e) => eprintln!("auto-run: could not load store: {e}"),
                }
            });
            Ok(())
        })
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
            export_servers,
            import_servers_from_file,
            get_app_config_path,
            start_server,
            stop_server,
            list_running,
            read_log,
            set_auto_run,
            get_auto_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
