use std::collections::HashMap;

use crate::adapter::{adapter_for, all_adapters};
use crate::paths;
use crate::store;
use crate::store::SyncSummary;
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

    // Surgically upsert/remove just this one server in the target app's live
    // config. This reads the file fresh and touches only this entry, so a
    // server added or edited in that app outside MCP Switch since the last
    // import is never clobbered or deleted by an unrelated toggle.
    if let Some(adapter) = adapter_for(&app_id) {
        let entry = if enabled {
            store.servers.iter().find(|s| s.name == server_name)
        } else {
            None
        };
        adapter.write_server(&server_name, entry)?;
    }

    Ok(store)
}

/// Re-reads every installed tool's live config and reconciles it into the
/// store: new servers are added, servers still enabled somewhere get their
/// command/args/env refreshed from that app's current definition, and
/// servers that vanished from every app they used to be enabled in are
/// soft-deleted (moved to Trash) rather than dropped outright. Safe to call
/// automatically on every app launch — an app whose config fails to parse
/// is simply skipped for this pass rather than treated as "now has zero
/// servers", so a transient error can never wrongly trash its entries.
#[tauri::command]
pub fn import_servers() -> Result<SyncSummary, String> {
    let mut fresh: HashMap<String, HashMap<String, McpServerEntry>> = HashMap::new();

    for adapter in all_adapters() {
        match adapter.read_servers() {
            Ok(servers) => {
                let by_name = servers.into_iter().map(|s| (s.name.clone(), s)).collect();
                fresh.insert(adapter.id().to_string(), by_name);
            }
            Err(e) => {
                eprintln!("Error reading {} config, skipping this app for this sync: {e}", adapter.id());
            }
        }
    }

    store::sync_servers(fresh).map_err(|e| e.to_string())
}

/// Restores a trashed server (undoes a soft-delete). It stays disabled for
/// every app until the user re-enables it from the normal list.
#[tauri::command]
pub fn restore_server(server_name: String) -> Result<Store, String> {
    store::restore_server(&server_name).map_err(|e| e.to_string())
}

/// Permanently removes a server from the store. Only meant to be called on
/// an already-trashed entry, which by definition is disabled everywhere
/// already, so no live config needs touching.
#[tauri::command]
pub fn delete_server_forever(server_name: String) -> Result<Store, String> {
    store::delete_server_forever(&server_name).map_err(|e| e.to_string())
}

/// Payload for creating or editing a server from the UI's Add/Edit form.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub transport: String,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub args: Option<Vec<String>>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
    /// App ids this server should be enabled for once saved.
    pub enabled_apps: Vec<String>,
}

/// Creates a new server or overwrites an existing one's definition (matched
/// by name), then syncs every affected app's live config directly — apps
/// newly checked get the entry written, apps newly unchecked get it
/// removed, and apps that stay checked get the (possibly edited)
/// definition re-written. This is the single place a server's config needs
/// to be authored; MCP Switch fans it out to every app's own config file.
#[tauri::command]
pub fn save_server(input: ServerInput) -> Result<Store, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Server name cannot be empty".to_string());
    }
    for app in &input.enabled_apps {
        if !crate::types::APPS.contains(&app.as_str()) {
            return Err(McpError::UnknownApp(app.clone()).into());
        }
    }

    let previous = store::list_servers()
        .map_err(|e| e.to_string())?
        .servers
        .into_iter()
        .find(|s| s.name == name);
    let previous_enabled = previous.as_ref().map(|s| s.enabled.clone()).unwrap_or_default();
    let sources = previous.map(|s| s.sources).unwrap_or_default();

    let enabled_set: std::collections::HashSet<&str> =
        input.enabled_apps.iter().map(String::as_str).collect();
    let mut enabled = HashMap::new();
    for app in crate::types::APPS {
        enabled.insert(app.to_string(), enabled_set.contains(app));
    }

    let entry = McpServerEntry {
        name: name.clone(),
        transport: input.transport,
        command: input.command,
        args: input.args,
        env: input.env,
        url: input.url,
        headers: input.headers,
        enabled,
        sources,
        deleted: false,
    };

    let store = store::upsert_server(entry.clone()).map_err(|e| e.to_string())?;

    for app in crate::types::APPS {
        let now_on = entry.enabled.get(*app).copied().unwrap_or(false);
        let was_on = previous_enabled.get(*app).copied().unwrap_or(false);
        if !now_on && !was_on {
            continue;
        }
        if let Some(adapter) = adapter_for(app) {
            let write_entry = if now_on { Some(&entry) } else { None };
            adapter.write_server(&entry.name, write_entry)?;
        }
    }

    Ok(store)
}
