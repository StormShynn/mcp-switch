use crate::atomic::{atomic_write, read_file_optional};
use crate::paths;
use crate::types::{McpError, McpServerEntry, Store};

/// Load the store from disk, returning an empty store if not found.
pub fn load_store() -> Result<Store, McpError> {
    let path = paths::store_path();
    match read_file_optional(&path)? {
        Some(content) => {
            let store: Store = serde_json::from_str(&content)?;
            Ok(store)
        }
        None => Ok(Store::empty()),
    }
}

/// Persist the store to disk atomically.
pub fn save_store(store: &Store) -> Result<(), McpError> {
    let path = paths::store_path();
    let content = serde_json::to_string_pretty(store)?;
    atomic_write(&path, &content)
}

/// Toggle a server's enabled state for a specific app.
/// Returns the updated store.
pub fn toggle_server(server_name: &str, app_id: &str, enabled: bool) -> Result<Store, McpError> {
    let mut store = load_store()?;

    let server = store
        .find_server_mut(server_name)
        .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

    server.enabled.insert(app_id.to_string(), enabled);
    save_store(&store)?;
    Ok(store)
}

/// List all servers in the store.
pub fn list_servers() -> Result<Store, McpError> {
    load_store()
}

/// Import servers from an external source and merge into the store.
/// New names are added outright; names that already exist only get their
/// `sources` merged in (command/args/env/enabled are left as the user has
/// them, so a re-import never clobbers manual edits or toggle choices).
/// Returns the number of brand-new servers added.
pub fn import_servers(servers: Vec<McpServerEntry>) -> Result<usize, McpError> {
    let mut store = load_store()?;
    let mut new_count = 0;
    let mut changed = false;

    for server in servers {
        match store.find_server_mut(&server.name) {
            Some(existing) => {
                for src in &server.sources {
                    if !existing.sources.contains(src) {
                        existing.sources.push(src.clone());
                        changed = true;
                    }
                }
            }
            None => {
                store.upsert_server(server);
                new_count += 1;
                changed = true;
            }
        }
    }

    if changed {
        save_store(&store)?;
    }

    Ok(new_count)
}
