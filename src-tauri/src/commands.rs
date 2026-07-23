use std::collections::HashMap;

use crate::adapter::{adapter_for, all_adapters};
use crate::paths;
use crate::store;
use crate::store::SyncSummary;
use crate::types::{ConnectionTestResult, McpError, McpServerEntry, Store};

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

/// Tests whether an MCP server is reachable by performing the MCP initialize
/// handshake. For stdio servers it spawns the process and sends an initialize
/// request over stdin; for HTTP/SSE servers it makes network requests to the
/// configured URL. Returns a `ConnectionTestResult` with the server's
/// reported identity on success, or an error description on failure.
#[tauri::command]
pub fn test_server_connection(server_name: String, app_id: String) -> Result<ConnectionTestResult, String> {
    if !crate::types::APPS.contains(&app_id.as_str()) {
        return Err(McpError::UnknownApp(app_id).into());
    }

    let store = store::list_servers().map_err(|e| e.to_string())?;
    let entry = store
        .servers
        .iter()
        .find(|s| s.name == server_name && s.app == app_id)
        .ok_or_else(|| McpError::ServerNotFound(server_name).to_string())?;

    Ok(crate::mcp_test::test_connection(entry))
}

/// Export the entire server store to a JSON file at the given path.
/// Useful for backup or transferring config between machines.
#[tauri::command]
pub fn export_servers(path: String) -> Result<(), String> {
    let store = store::list_servers().map_err(|e| e.to_string())?;
    let content = serde_json::to_string_pretty(&store).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| format!("Failed to write export file: {e}"))?;
    Ok(())
}

/// Import servers from a JSON file and merge them into the store.
/// Returns the number of servers added.
#[tauri::command]
pub fn import_servers_from_file(path: String) -> Result<usize, String> {
    let content = std::fs::read_to_string(&path).map_err(|e| format!("Failed to read import file: {e}"))?;
    let imported: Store = serde_json::from_str(&content).map_err(|e| format!("Invalid server file: {e}"))?;

    let mut store = store::load_store().map_err(|e| e.to_string())?;
    let mut added = 0;
    for entry in imported.servers {
        if !crate::types::APPS.contains(&entry.app.as_str()) {
            eprintln!("Skipping entry '{}' — unknown app '{}'", entry.name, entry.app);
            continue;
        }
        if store.find_server_mut(&entry.name, &entry.app).is_some() {
            // Server already exists, skip to avoid overwriting
            continue;
        }
        store.servers.push(entry);
        added += 1;
    }
    store::save_store(&store).map_err(|e| e.to_string())?;
    Ok(added)
}

/// Return the live config file path for a given app (for the "Open config" feature).
#[tauri::command]
pub fn get_app_config_path(app_id: String) -> Result<String, String> {
    if !crate::types::APPS.contains(&app_id.as_str()) {
        return Err(McpError::UnknownApp(app_id).into());
    }
    let path = crate::paths::app_config_path(&app_id)
        .ok_or_else(|| format!("No config path known for app '{app_id}'"))?;
    Ok(path.display().to_string())
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

    // Carry over any previously-captured extra fields (Codex's `cwd`,
    // Gemini's `timeout`, ...) so editing a server through this form never
    // drops a live-config field the form itself doesn't expose.
    let extra = store::list_servers()
        .map_err(|e| e.to_string())?
        .servers
        .into_iter()
        .find(|s| s.name == name && s.app == input.app)
        .map(|s| s.extra)
        .unwrap_or_default();

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
        extra,
    };

    let store = store::upsert_server(entry.clone()).map_err(|e| e.to_string())?;

    if let Some(adapter) = adapter_for(&input.app) {
        let write_entry = if entry.enabled { Some(&entry) } else { None };
        adapter.write_server(&entry.name, write_entry)?;
    }

    Ok(store)
}

/// Start a stdio MCP server as a child of MCP Switch, with no console window
/// on Windows. Its stdout/stderr are captured in an in-memory ring buffer.
#[tauri::command]
pub fn start_server(
    server_name: String,
    app_id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<crate::runner::RunningServer, String> {
    if !crate::types::APPS.contains(&app_id.as_str()) {
        return Err(McpError::UnknownApp(app_id).into());
    }

    let store = store::list_servers().map_err(|e| e.to_string())?;
    let entry = store
        .servers
        .iter()
        .find(|s| s.name == server_name && s.app == app_id)
        .ok_or_else(|| McpError::ServerNotFound(server_name.clone()).to_string())?;

    let policy = state.get_restart_policy(&server_name, &app_id);
    state.start_with_app(Some(&app), entry, Some(policy)).map_err(|e| e.to_string())
}

/// Stop a server previously started by MCP Switch. Returns false when no
/// live process was registered for that `(name, app)` pair.
#[tauri::command]
pub fn stop_server(
    server_name: String,
    app_id: String,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<bool, String> {
    state
        .stop(&server_name, &app_id)
        .map_err(|e| e.to_string())
}

/// Return live child processes owned by MCP Switch. Dead children are reaped
/// before the snapshot is produced.
#[tauri::command]
pub fn list_running(
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Vec<crate::runner::RunningServer> {
    state.list()
}

/// Read the most recent stdout/stderr lines captured for a running server.
#[tauri::command]
pub fn read_log(
    server_name: String,
    app_id: String,
    tail: Option<usize>,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Vec<String> {
    state.read_log(&server_name, &app_id, tail.unwrap_or(100).min(500))
}

/// Toggle whether `server_name` for `app_id` should auto-spawn when MCP
/// Switch launches.
#[tauri::command]
pub fn set_auto_run(
    server_name: String,
    app_id: String,
    enabled: bool,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<bool, String> {
    state
        .set_auto_run(&server_name, &app_id, enabled)
        .map_err(|e| e.to_string())
}

/// Return the persisted auto-run list, alphabetised by (app, name).
#[tauri::command]
pub fn get_auto_run(
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Vec<crate::runner::ProfileMemberDto> {
    state.get_auto_run()
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartPolicyInput {
    pub mode: String,
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub backoff_ms: Option<u64>,
}

fn parse_policy(input: RestartPolicyInput) -> crate::runner::RestartPolicy {
    use crate::runner::RestartPolicy;
    let max_retries = input.max_retries.unwrap_or(5);
    let backoff_ms = input.backoff_ms.unwrap_or(1000);
    match input.mode.as_str() {
        "onFailure" | "on_failure" => {
            RestartPolicy::OnFailure { max_retries, backoff_ms }
        }
        "always" | "Always" => RestartPolicy::Always { max_retries, backoff_ms },
        _ => RestartPolicy::Never,
    }
}

/// Read the persisted restart policy for a server.
#[tauri::command]
pub fn get_restart_policy(
    server_name: String,
    app_id: String,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> crate::runner::RestartPolicy {
    state.get_restart_policy(&server_name, &app_id)
}

/// Save the restart policy for a server.
#[tauri::command]
pub fn set_restart_policy(
    server_name: String,
    app_id: String,
    policy: RestartPolicyInput,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<(), String> {
    let parsed = parse_policy(policy);
    state
        .set_restart_policy(&server_name, &app_id, parsed)
        .map_err(|e| e.to_string())
}

/// List all saved profiles (Foreman-style procfile groups).
#[tauri::command]
pub fn list_profiles(
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Vec<crate::runner::ProfileDto> {
    state.list_profiles()
}

/// Save (insert or update) a profile by id.
#[tauri::command]
pub fn upsert_profile(
    profile: crate::runner::ProfileDto,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<(), String> {
    state.upsert_profile(profile).map_err(|e| e.to_string())
}

/// Delete a profile by id. Returns true if one was actually removed.
#[tauri::command]
pub fn delete_profile(
    id: String,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<bool, String> {
    state.delete_profile(&id).map_err(|e| e.to_string())
}

/// Start every member of a profile. Returns a list of error strings
/// (empty list = all good).
#[tauri::command]
pub fn start_profile(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Result<Vec<String>, String> {
    let store = store::list_servers().map_err(|e| e.to_string())?;
    Ok(state.start_profile(&app, &id, &store.servers))
}

/// Stop every member of a profile. Returns the list of (name, app, killed).
#[tauri::command]
pub fn stop_profile(
    id: String,
    state: tauri::State<'_, crate::runner::RunnerState>,
) -> Vec<(String, String, bool)> {
    state.stop_profile(&id)
}