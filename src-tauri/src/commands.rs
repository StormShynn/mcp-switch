use crate::adapter::{adapter_for, all_adapters};
use crate::store;
use crate::types::{McpError, McpServerEntry, Store};

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
    let mut imported = Vec::new();

    for adapter in all_adapters() {
        match adapter.read_servers() {
            Ok(servers) => {
                for mut server in servers {
                    // Mark the server as enabled for this app
                    server
                        .enabled
                        .insert(adapter.id().to_string(), true);
                    imported.push(server);
                }
            }
            Err(e) => {
                eprintln!("Error reading {} config: {e}", adapter.id());
            }
        }
    }

    let count = store::import_servers(imported).map_err(|e| e.to_string())?;
    Ok(count)
}
