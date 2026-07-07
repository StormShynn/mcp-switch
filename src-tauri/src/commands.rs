use crate::adapter::{adapter_for, all_adapters};
use crate::paths;
use crate::store;
use crate::types::{McpError, McpServerEntry, Store};

/// Returns the on-disk path of the SSOT store, for display in the UI.
#[tauri::command]
pub fn get_store_path() -> String {
    paths::store_path().display().to_string()
}

/// List all MCP servers from the store.
#[tauri::command]
pub fn list_servers() -> Result<Store, String> {
    store::list_servers().map_err(|e| e.to_string())
}

/// Toggle a server's enabled state for a specific app.
/// Then write the updated config for that app.
#[tauri::command]
pub fn toggle_server(server_name: String, app_id: String, enabled: bool) -> Result<Store, String> {
    // Validate app_id
    if !crate::types::APPS.contains(&app_id.as_str()) {
        return Err(McpError::UnknownApp(app_id).into());
    }

    // Update the store
    let store = store::toggle_server(&server_name, &app_id, enabled)?;

    // Write the enabled servers to the target app's config
    if let Some(adapter) = adapter_for(&app_id) {
        let enabled_servers: Vec<McpServerEntry> = store
            .servers
            .iter()
            .filter(|s| s.enabled.get(&app_id).copied().unwrap_or(false))
            .cloned()
            .collect();
        adapter.write_enabled(&enabled_servers)?;
    }

    Ok(store)
}

/// Import MCP servers from all installed coding tools into the store.
#[tauri::command]
pub fn import_servers() -> Result<usize, String> {
    // Merge by name across tools first: the same server can be defined in
    // multiple config files, and each one only knows about its own
    // `enabled` bit. Without this merge, reading a second tool's copy of an
    // already-seen name would be skipped entirely by `store::import_servers`,
    // silently dropping that tool's enabled flag.
    let mut merged: std::collections::HashMap<String, McpServerEntry> = std::collections::HashMap::new();

    for adapter in all_adapters() {
        match adapter.read_servers() {
            Ok(servers) => {
                let app_id = adapter.id();
                for mut server in servers {
                    // Mark enabled for the app it was imported from, disabled for others
                    for other in crate::types::APPS {
                        server.enabled.insert((*other).to_string(), *other == app_id);
                    }
                    server.sources = vec![app_id.to_string()];
                    merged
                        .entry(server.name.clone())
                        .and_modify(|existing| {
                            for (app, on) in &server.enabled {
                                if *on {
                                    existing.enabled.insert(app.clone(), true);
                                }
                            }
                            for src in &server.sources {
                                if !existing.sources.contains(src) {
                                    existing.sources.push(src.clone());
                                }
                            }
                        })
                        .or_insert(server);
                }
            }
            Err(e) => {
                eprintln!("Error reading {} config: {e}", adapter.id());
            }
        }
    }

    let count = store::import_servers(merged.into_values().collect()).map_err(|e| e.to_string())?;
    Ok(count)
}
