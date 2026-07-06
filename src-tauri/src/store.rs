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
/// Returns the number of new servers added.
pub fn import_servers(servers: Vec<McpServerEntry>) -> Result<usize, McpError> {
    let mut store = load_store()?;
    let mut count = 0;

    for server in servers {
        if store.find_server(&server.name).is_none() {
            store.upsert_server(server);
            count += 1;
        }
    }

    if count > 0 {
        save_store(&store)?;
    }

    Ok(count)
}
