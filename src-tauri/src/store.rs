use std::collections::{HashMap, HashSet};

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

/// Result of a [`sync_servers`] pass, for the UI's post-import summary toast.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSummary {
    pub added: usize,
    pub flagged_deleted: usize,
}

/// Copies the live-config-derived fields from `fresh` onto `existing`,
/// leaving `enabled`/`sources`/`deleted` (store-owned bookkeeping) untouched.
fn apply_fresh_fields(existing: &mut McpServerEntry, fresh: &McpServerEntry) {
    existing.transport = fresh.transport.clone();
    existing.command = fresh.command.clone();
    existing.args = fresh.args.clone();
    existing.env = fresh.env.clone();
    existing.url = fresh.url.clone();
    existing.headers = fresh.headers.clone();
}

/// Reconciles the store against a fresh read of every app's live config.
///
/// `fresh` maps app_id -> (server name -> freshly-read entry), and only
/// contains an app_id if that app's config was read successfully this pass —
/// an app that failed to parse is simply absent, so a transient read error
/// can never be mistaken for "this app now has zero servers" and wrongly
/// flag everything enabled there as deleted.
///
/// For each app a server is already enabled for:
/// - still present live -> refresh command/args/env/url/headers from it
///   (that app is authoritative for its own current definition);
/// - no longer present live -> drift: turn off `enabled` for that app and
///   drop it from `sources`, but never touch other apps' state.
/// For each app a server is NOT currently enabled for but IS found live
/// (e.g. added directly to that app's config outside MCP Switch) -> turn it
/// on and adopt the fresh definition.
/// A server that had at least one app enabled before this pass and ends up
/// with none is soft-deleted (`deleted = true`) rather than removed, so its
/// last-known command/args/env is never lost. A server that regains an
/// enabled app is un-deleted automatically.
/// Names present live that don't exist in the store at all are added fresh.
pub fn sync_servers(
    fresh: HashMap<String, HashMap<String, McpServerEntry>>,
) -> Result<SyncSummary, McpError> {
    let mut store = load_store()?;
    let summary = reconcile(&mut store, &fresh);
    save_store(&store)?;
    Ok(summary)
}

/// Pure reconciliation core (no file I/O), split out from [`sync_servers`]
/// so it can be unit-tested against an in-memory [`Store`] without ever
/// touching the user's real `store.json`.
fn reconcile(
    store: &mut Store,
    fresh: &HashMap<String, HashMap<String, McpServerEntry>>,
) -> SyncSummary {
    let mut flagged_deleted = 0;

    for existing in store.servers.iter_mut() {
        let had_any_enabled_before = existing.enabled.values().any(|v| *v);

        for (app_id, live_by_name) in fresh {
            let was_enabled = existing.enabled.get(app_id).copied().unwrap_or(false);
            let live_entry = live_by_name.get(&existing.name);

            match (was_enabled, live_entry) {
                (true, Some(fresh_entry)) => {
                    apply_fresh_fields(existing, fresh_entry);
                    if !existing.sources.contains(app_id) {
                        existing.sources.push(app_id.clone());
                    }
                }
                (true, None) => {
                    existing.enabled.insert(app_id.clone(), false);
                    existing.sources.retain(|s| s != app_id);
                }
                (false, Some(fresh_entry)) => {
                    apply_fresh_fields(existing, fresh_entry);
                    existing.enabled.insert(app_id.clone(), true);
                    if !existing.sources.contains(app_id) {
                        existing.sources.push(app_id.clone());
                    }
                }
                (false, None) => {}
            }
        }

        let has_any_enabled_after = existing.enabled.values().any(|v| *v);
        if has_any_enabled_after {
            existing.deleted = false;
        } else if had_any_enabled_before && !existing.deleted {
            existing.deleted = true;
            flagged_deleted += 1;
        }
    }

    let existing_names: HashSet<String> =
        store.servers.iter().map(|s| s.name.clone()).collect();
    let mut new_entries: HashMap<String, McpServerEntry> = HashMap::new();

    for app_id in crate::types::APPS {
        let Some(live_by_name) = fresh.get(*app_id) else {
            continue;
        };
        for (name, fresh_entry) in live_by_name {
            if existing_names.contains(name) {
                continue;
            }
            new_entries
                .entry(name.clone())
                .and_modify(|acc| {
                    acc.enabled.insert(app_id.to_string(), true);
                    if !acc.sources.contains(&app_id.to_string()) {
                        acc.sources.push(app_id.to_string());
                    }
                })
                .or_insert_with(|| {
                    let mut entry = fresh_entry.clone();
                    for other in crate::types::APPS {
                        entry.enabled.insert(other.to_string(), *other == *app_id);
                    }
                    entry.sources = vec![app_id.to_string()];
                    entry.deleted = false;
                    entry
                });
        }
    }

    let added = new_entries.len();
    store.servers.extend(new_entries.into_values());

    SyncSummary {
        added,
        flagged_deleted,
    }
}

/// Creates or fully replaces a server's definition (used by the manual
/// add/edit form) and returns the updated store.
pub fn upsert_server(entry: McpServerEntry) -> Result<Store, McpError> {
    let mut store = load_store()?;
    store.upsert_server(entry);
    save_store(&store)?;
    Ok(store)
}

/// Un-trashes a server, leaving its `enabled` map (all-false) as-is — the
/// user re-enables it per app from the normal list afterwards.
pub fn restore_server(name: &str) -> Result<Store, McpError> {
    let mut store = load_store()?;
    let server = store
        .find_server_mut(name)
        .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;
    server.deleted = false;
    save_store(&store)?;
    Ok(store)
}

/// Permanently removes a server from the store. Safe to call on a trashed
/// entry without touching any live config: a trashed server is by
/// definition disabled everywhere already, so MCP Switch never wrote it
/// into any app's config in the first place.
pub fn delete_server_forever(name: &str) -> Result<Store, McpError> {
    let mut store = load_store()?;
    store.servers.retain(|s| s.name != name);
    save_store(&store)?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stdio_entry(name: &str, command: &str) -> McpServerEntry {
        McpServerEntry {
            name: name.to_string(),
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args: None,
            env: None,
            url: None,
            headers: None,
            enabled: HashMap::new(),
            sources: Vec::new(),
            deleted: false,
        }
    }

    fn stored(name: &str, command: &str, enabled: &[(&str, bool)], sources: &[&str]) -> McpServerEntry {
        let mut e = stdio_entry(name, command);
        e.enabled = enabled.iter().map(|(a, v)| (a.to_string(), *v)).collect();
        e.sources = sources.iter().map(|s| s.to_string()).collect();
        e
    }

    fn fresh_map(
        entries: &[(&str, &[(&str, &str)])],
    ) -> HashMap<String, HashMap<String, McpServerEntry>> {
        entries
            .iter()
            .map(|(app, servers)| {
                let by_name = servers
                    .iter()
                    .map(|(name, cmd)| (name.to_string(), stdio_entry(name, cmd)))
                    .collect();
                (app.to_string(), by_name)
            })
            .collect()
    }

    #[test]
    fn brand_new_live_server_is_added() {
        let mut store = Store::empty();
        let fresh = fresh_map(&[("claude", &[("fs", "npx")])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.added, 1);
        assert_eq!(summary.flagged_deleted, 0);
        let fs = store.servers.iter().find(|s| s.name == "fs").unwrap();
        assert_eq!(fs.enabled.get("claude"), Some(&true));
        assert_eq!(fs.sources, vec!["claude".to_string()]);
        assert!(!fs.deleted);
    }

    #[test]
    fn enabled_server_still_present_is_refreshed_not_flagged() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "old-cmd", &[("claude", true)], &["claude"]));
        let fresh = fresh_map(&[("claude", &[("fs", "new-cmd")])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0);
        let fs = &store.servers[0];
        assert_eq!(fs.command, Some("new-cmd".to_string()));
        assert!(!fs.deleted);
    }

    #[test]
    fn enabled_server_missing_live_is_flagged_deleted_but_kept() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "npx", &[("claude", true)], &["claude"]));
        // "claude" was scanned successfully this pass but no longer has "fs".
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 1);
        assert_eq!(store.servers.len(), 1, "entry must be kept, not removed");
        let fs = &store.servers[0];
        assert!(fs.deleted);
        assert_eq!(fs.command, Some("npx".to_string()), "last-known command preserved");
        assert_eq!(fs.enabled.get("claude"), Some(&false));
        assert!(!fs.sources.contains(&"claude".to_string()));
    }

    #[test]
    fn already_disabled_server_missing_live_is_left_alone() {
        // This is the case the user specifically asked about: a server the
        // user turned off themselves must never be touched just because the
        // app it used to live in doesn't have it (MCP Switch removed it
        // there itself when it was toggled off).
        let mut store = Store::empty();
        store.servers.push(stored("fs", "npx", &[("claude", false)], &[]));
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0);
        assert!(!store.servers[0].deleted);
    }

    #[test]
    fn server_enabled_in_one_app_is_not_flagged_when_another_app_drifts() {
        let mut store = Store::empty();
        store.servers.push(stored(
            "fs",
            "npx",
            &[("claude", true), ("codex", true)],
            &["claude", "codex"],
        ));
        // Only "claude" drops it; "codex" isn't scanned this pass (e.g. a
        // transient read error), so it must be left exactly as-is.
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0, "still enabled via codex");
        let fs = &store.servers[0];
        assert!(!fs.deleted);
        assert_eq!(fs.enabled.get("claude"), Some(&false));
        assert_eq!(fs.enabled.get("codex"), Some(&true), "untouched: codex wasn't scanned");
    }

    #[test]
    fn disabled_server_reappearing_live_is_turned_back_on() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "old-cmd", &[("claude", false)], &[]));
        let fresh = fresh_map(&[("claude", &[("fs", "new-cmd")])]);

        reconcile(&mut store, &fresh);

        let fs = &store.servers[0];
        assert_eq!(fs.enabled.get("claude"), Some(&true));
        assert_eq!(fs.command, Some("new-cmd".to_string()));
        assert!(fs.sources.contains(&"claude".to_string()));
    }

    #[test]
    fn trashed_server_regaining_a_live_app_is_restored_automatically() {
        let mut store = Store::empty();
        let mut fs = stored("fs", "npx", &[("claude", false)], &[]);
        fs.deleted = true;
        store.servers.push(fs);
        let fresh = fresh_map(&[("claude", &[("fs", "npx")])]);

        reconcile(&mut store, &fresh);

        assert!(!store.servers[0].deleted);
    }

    #[test]
    fn app_absent_from_fresh_map_is_never_checked() {
        // Simulates a transient parse error on "codex": it's entirely absent
        // from `fresh`, so nothing enabled there should be touched at all.
        let mut store = Store::empty();
        store.servers.push(stored("fs", "npx", &[("codex", true)], &["codex"]));
        let fresh: HashMap<String, HashMap<String, McpServerEntry>> = HashMap::new();

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0);
        assert_eq!(store.servers[0].enabled.get("codex"), Some(&true));
    }
}
