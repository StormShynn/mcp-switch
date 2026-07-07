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

/// Toggle a server's enabled state. `server_name`+`app_id` identify a single
/// entry, since every entry belongs to exactly one app.
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
            store
                .servers
                .iter()
                .find(|s| s.name == server_name && s.app == app_id)
        } else {
            None
        };
        adapter.write_server(&server_name, entry)?;
    }

    Ok(store)
}

/// Re-reads every installed tool's live config and reconciles it into the
/// store: new `(name, app)` entries are added, entries still enabled get
/// their command/args/env refreshed from that app's current definition, and
/// entries that vanished from their app's live config are soft-deleted
/// (moved to Trash) rather than dropped outright. Safe to call automatically
/// on every app launch — an app whose config fails to parse is simply
/// skipped for this pass rather than treated as "now has zero servers", so a
/// transient error can never wrongly trash its entries.
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

/// Moves a server from the main list straight to Trash: removes it from its
/// app's live config first if it was enabled (the same surgical single-entry
/// removal `toggle_server(..., false)` does), then flags it deleted in the
/// store. Reversible via `restore_server`; permanent removal still goes
/// through `delete_server_forever`, which the UI gates behind its own
/// confirmation dialog.
#[tauri::command]
pub fn trash_server(server_name: String, app_id: String) -> Result<Store, String> {
    if !crate::types::APPS.contains(&app_id.as_str()) {
        return Err(McpError::UnknownApp(app_id).into());
    }

    if let Some(adapter) = adapter_for(&app_id) {
        adapter.write_server(&server_name, None)?;
    }

    store::trash_server(&server_name, &app_id).map_err(|e| e.to_string())
}

/// Kills and relaunches `app_id`'s GUI process — currently only
/// "claude-desktop" and "antigravity", the two apps installed as a Windows
/// Store package with a discoverable launch id. See
/// `app_control::store_package_name` for why this is deliberately not
/// offered for CLI tools.
#[tauri::command]
pub fn restart_app(app_id: String) -> Result<(), String> {
    crate::app_control::restart_app(&app_id).map_err(|e| e.to_string())
}

/// Restores a trashed server (undoes a soft-delete). It stays disabled until
/// the user re-enables it from the normal list.
#[tauri::command]
pub fn restore_server(server_name: String, app_id: String) -> Result<Store, String> {
    store::restore_server(&server_name, &app_id).map_err(|e| e.to_string())
}

/// Permanently removes a server from the store. Only meant to be called on
/// an already-trashed entry, which by definition is disabled already, so no
/// live config needs touching.
#[tauri::command]
pub fn delete_server_forever(server_name: String, app_id: String) -> Result<Store, String> {
    store::delete_server_forever(&server_name, &app_id).map_err(|e| e.to_string())
}

/// Payload for creating or editing a server from the UI's Add/Edit form.
/// Every server belongs to exactly one app: real-world MCP server configs
/// often genuinely differ per tool (different cookie/token paths, different
/// transports), so a definition is never fanned out across several apps.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub app: String,
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
    #[serde(default)]
    pub enabled: bool,
}

/// Creates a new server or overwrites an existing one's definition (matched
/// by `(name, app)`), then writes it straight into that one app's live
/// config — surgically, so every other entry already there (including ones
/// MCP Switch doesn't track) is preserved untouched.
#[tauri::command]
pub fn save_server(input: ServerInput) -> Result<Store, String> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err("Server name cannot be empty".to_string());
    }
    if !crate::types::APPS.contains(&input.app.as_str()) {
        return Err(McpError::UnknownApp(input.app.clone()).into());
    }

    let entry = McpServerEntry {
        name: name.clone(),
        app: input.app.clone(),
        transport: input.transport,
        command: input.command,
        args: input.args,
        env: input.env,
        url: input.url,
        headers: input.headers,
        enabled: input.enabled,
        deleted: false,
    };

    let store = store::upsert_server(entry.clone()).map_err(|e| e.to_string())?;

    if let Some(adapter) = adapter_for(&input.app) {
        let write_entry = if entry.enabled { Some(&entry) } else { None };
        adapter.write_server(&entry.name, write_entry)?;
    }

    Ok(store)
}
