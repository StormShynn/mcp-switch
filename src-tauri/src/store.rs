use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::atomic::{atomic_write, read_file_optional};
use crate::paths;
use crate::types::{McpError, McpServerEntry, Store};

/// Load the store from disk, returning an empty store if not found — or if
/// found but no longer parseable under the current schema (e.g. an on-disk
/// file written by an older MCP Switch version). The store is only ever a
/// synced cache of every tool's live config, never the source of truth, so
/// starting fresh and letting the next sync repopulate it is safe, and far
/// better than every command hard-failing after a schema change.
///
/// Before doing that, the unparseable file is backed up (see
/// [`backup_store_file`]) — a schema change alone can't destroy anything
/// then, and `save_store` backs up on every write anyway so the very next
/// save wouldn't have clobbered it silently even without this.
pub fn load_store() -> Result<Store, McpError> {
    let path = paths::store_path();
    match read_file_optional(&path)? {
        Some(content) => match serde_json::from_str(&content) {
            Ok(store) => Ok(store),
            Err(e) => {
                backup_store_file(&path);
                eprintln!("Store file didn't match the current schema, starting fresh: {e}");
                Ok(Store::empty())
            }
        },
        None => Ok(Store::empty()),
    }
}

/// How long a `.bak.*` copy of the store is kept before [`cleanup_old_backups`]
/// deletes it.
const BACKUP_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

/// Copies `path`'s current on-disk content to a timestamped `<name>.bak.<unix
/// seconds>` next to it, then sweeps the directory for backups older than
/// [`BACKUP_RETENTION_SECS`] and deletes them. Called on every `save_store`
/// (so every version of the store that ever existed has a same-day-or-newer
/// recovery point for a full week) and whenever a load fails to parse (so a
/// breaking schema change is backed up the moment it's detected, not just
/// whenever the next save happens to occur). A no-op if `path` doesn't exist
/// yet — nothing to back up. Best-effort throughout: a failure to back up or
/// clean up is logged, never fatal, since refusing to save/load over a
/// backup hiccup would be worse than the data-loss risk this exists to
/// prevent.
fn backup_store_file(path: &Path) {
    if !path.exists() {
        return;
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let file_name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_else(|| std::ffi::OsString::from("store.json"));
    let mut backup_name = file_name;
    backup_name.push(format!(".bak.{now}"));
    let backup_path = path.with_file_name(backup_name);

    match std::fs::copy(path, &backup_path) {
        Ok(_) => {}
        Err(backup_err) => eprintln!(
            "Failed to back up store file to {}: {backup_err}",
            backup_path.display()
        ),
    }

    cleanup_old_backups(path, now);
}

/// Deletes sibling `<name>.bak.<unix-seconds>` files whose *own encoded
/// timestamp* (not filesystem mtime, which a copy/sync could reset) is more
/// than [`BACKUP_RETENTION_SECS`] behind `now`. Only ever touches files
/// matching that exact naming pattern next to `path`, so it can't reach any
/// other file in the directory.
fn cleanup_old_backups(path: &Path, now: u64) {
    let Some(dir) = path.parent() else { return };
    let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else { return };
    let prefix = format!("{file_name}.bak.");

    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(timestamp) = name.strip_prefix(&prefix).and_then(|s| s.parse::<u64>().ok()) else {
            continue;
        };
        if now.saturating_sub(timestamp) > BACKUP_RETENTION_SECS {
            if let Err(e) = std::fs::remove_file(entry.path()) {
                eprintln!("Failed to remove expired backup {}: {e}", entry.path().display());
            }
        }
    }
}

/// Persist the store to disk atomically. Backs up whatever was on disk
/// beforehand (see [`backup_store_file`]) so every save leaves a same-day
/// recovery point behind, not just the schema-mismatch case.
pub fn save_store(store: &Store) -> Result<(), McpError> {
    let path = paths::store_path();
    backup_store_file(&path);
    let content = serde_json::to_string_pretty(store)?;
    atomic_write(&path, &content)
}

/// Toggle a server's enabled state. `server_name`+`app_id` must identify an
/// existing entry (added manually or discovered via import).
pub fn toggle_server(server_name: &str, app_id: &str, enabled: bool) -> Result<Store, McpError> {
    let mut store = load_store()?;

    let server = store
        .find_server_mut(server_name, app_id)
        .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

    server.enabled = enabled;
    save_store(&store)?;
    Ok(store)
}

/// Moves an entry straight to Trash from the main list — a manual
/// counterpart to the automatic soft-delete `reconcile` applies when a
/// server vanishes from live config. Disables it (the caller is
/// responsible for removing it from the live config first, the same way
/// `toggle_server(..., false)`'s caller does) and flags it deleted in the
/// same save, so it can never end up enabled-but-hidden in Trash.
pub fn trash_server(server_name: &str, app_id: &str) -> Result<Store, McpError> {
    let mut store = load_store()?;

    let server = store
        .find_server_mut(server_name, app_id)
        .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

    server.enabled = false;
    server.deleted = true;
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
/// leaving `enabled`/`deleted` (store-owned bookkeeping) and `name`/`app`
/// (identity) untouched.
fn apply_fresh_fields(existing: &mut McpServerEntry, fresh: &McpServerEntry) {
    existing.transport = fresh.transport.clone();
    existing.command = fresh.command.clone();
    existing.args = fresh.args.clone();
    existing.env = fresh.env.clone();
    existing.url = fresh.url.clone();
    existing.headers = fresh.headers.clone();
    existing.extra = fresh.extra.clone();
}

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
///
/// `fresh` maps app_id -> (server name -> freshly-read entry), and only
/// contains an app_id if that app's config was read successfully this pass —
/// an app that failed to parse is simply absent, so a transient read error
/// can never be mistaken for "this app now has zero servers" and wrongly
/// flag its entries as deleted.
///
/// Every entry is scoped to exactly one `(name, app)` pair, so reconciling
/// one entry can never affect another that merely happens to share a name in
/// a different app's config — two apps genuinely needing different
/// command/args/env for a same-named server can never clobber each other.
///
/// For each existing entry, keyed by its own `(name, app)`:
/// - still enabled and present live -> refresh command/args/env/url/headers
///   from it (that app is authoritative for its own current definition);
/// - enabled but no longer present live -> drift: turn off and soft-delete
///   (`deleted = true`), keeping the last-known config instead of losing it;
/// - disabled but now present live (e.g. added/re-enabled directly in that
///   app's config outside MCP Switch) -> turn on, adopt the fresh
///   definition, and un-delete;
/// - disabled and still absent live -> expected (MCP Switch itself removed
///   it when it was toggled off), left alone entirely.
/// An app absent from `fresh` this pass is never checked at all.
/// `(name, app)` pairs present live that don't exist in the store yet are
/// added fresh, enabled.
fn reconcile(
    store: &mut Store,
    fresh: &HashMap<String, HashMap<String, McpServerEntry>>,
) -> SyncSummary {
    let mut flagged_deleted = 0;

    for existing in store.servers.iter_mut() {
        let Some(live_by_name) = fresh.get(&existing.app) else {
            continue; // this app wasn't scanned this pass (e.g. transient read error)
        };
        let live_entry = live_by_name.get(&existing.name);

        match (existing.enabled, live_entry) {
            (true, Some(fresh_entry)) => {
                apply_fresh_fields(existing, fresh_entry);
                existing.deleted = false;
            }
            (true, None) => {
                existing.enabled = false;
                if !existing.deleted {
                    existing.deleted = true;
                    flagged_deleted += 1;
                }
            }
            (false, Some(fresh_entry)) => {
                apply_fresh_fields(existing, fresh_entry);
                existing.enabled = true;
                existing.deleted = false;
            }
            (false, None) => {}
        }
    }

    let existing_keys: HashSet<(String, String)> = store
        .servers
        .iter()
        .map(|s| (s.name.clone(), s.app.clone()))
        .collect();

    let mut added = 0;
    for (app_id, live_by_name) in fresh {
        for (name, fresh_entry) in live_by_name {
            if existing_keys.contains(&(name.clone(), app_id.clone())) {
                continue;
            }
            let mut entry = fresh_entry.clone();
            entry.name = name.clone();
            entry.app = app_id.clone();
            entry.enabled = true;
            entry.deleted = false;
            store.servers.push(entry);
            added += 1;
        }
    }

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

/// Un-trashes a server, leaving `enabled = false` as-is — the user
/// re-enables it from the normal list afterwards.
pub fn restore_server(name: &str, app: &str) -> Result<Store, McpError> {
    let mut store = load_store()?;
    let server = store
        .find_server_mut(name, app)
        .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;
    server.deleted = false;
    save_store(&store)?;
    Ok(store)
}

/// Permanently removes a server from the store. Safe to call on a trashed
/// entry without touching any live config: a trashed server is by
/// definition disabled already, so MCP Switch never wrote it into its app's
/// config in the first place.
pub fn delete_server_forever(name: &str, app: &str) -> Result<Store, McpError> {
    let mut store = load_store()?;
    store.servers.retain(|s| !(s.name == name && s.app == app));
    save_store(&store)?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_store_file_preserves_original_content() {
        // Uses the OS temp dir, never the real store path, so this test can
        // never touch (let alone lose) the user's actual store.json.
        let path = std::env::temp_dir().join("mcp_switch_test_store_backup.json");
        std::fs::write(&path, "not valid store json").unwrap();

        backup_store_file(&path);

        let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("mcp_switch_test_store_backup.json.bak.")
            })
            .collect();
        assert_eq!(backups.len(), 1, "expected exactly one backup file");
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).unwrap(),
            "not valid store json"
        );

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(backups[0].path()).unwrap();
    }

    #[test]
    fn backup_store_file_is_a_noop_when_nothing_exists_yet() {
        let path = std::env::temp_dir().join("mcp_switch_test_store_backup_missing.json");
        let _ = std::fs::remove_file(&path); // in case a previous run left one behind

        backup_store_file(&path); // must not panic or create anything

        let stray: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("mcp_switch_test_store_backup_missing.json.bak.")
            })
            .collect();
        assert!(stray.is_empty(), "nothing to back up, so no backup should appear");
    }

    #[test]
    fn cleanup_old_backups_removes_only_entries_past_retention() {
        let path = std::env::temp_dir().join("mcp_switch_test_cleanup_target.json");
        std::fs::write(&path, "content").unwrap();
        let dir = path.parent().unwrap();
        let file_name = path.file_name().unwrap().to_str().unwrap();

        // Fixed reference instant so this test never depends on wall-clock time.
        let now: u64 = 1_700_000_000;
        let old_backup = dir.join(format!("{file_name}.bak.{}", now - BACKUP_RETENTION_SECS - 1));
        let boundary_backup = dir.join(format!("{file_name}.bak.{}", now - BACKUP_RETENTION_SECS));
        let recent_backup = dir.join(format!("{file_name}.bak.{}", now - 100));
        std::fs::write(&old_backup, "old").unwrap();
        std::fs::write(&boundary_backup, "boundary").unwrap();
        std::fs::write(&recent_backup, "recent").unwrap();

        cleanup_old_backups(&path, now);

        assert!(!old_backup.exists(), "backup past retention should be removed");
        assert!(boundary_backup.exists(), "backup exactly at retention should be kept");
        assert!(recent_backup.exists(), "backup within retention should be kept");

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(&boundary_backup).unwrap();
        std::fs::remove_file(&recent_backup).unwrap();
    }

    fn stdio_entry(name: &str, app: &str, command: &str) -> McpServerEntry {
        McpServerEntry {
            name: name.to_string(),
            app: app.to_string(),
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args: None,
            env: None,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
            extra: HashMap::new(),
        }
    }

    fn stored(name: &str, app: &str, command: &str, enabled: bool, deleted: bool) -> McpServerEntry {
        let mut e = stdio_entry(name, app, command);
        e.enabled = enabled;
        e.deleted = deleted;
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
                    .map(|(name, cmd)| (name.to_string(), stdio_entry(name, app, cmd)))
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
        assert!(fs.enabled);
        assert_eq!(fs.app, "claude");
        assert!(!fs.deleted);
    }

    #[test]
    fn same_name_different_app_is_added_as_a_separate_entry() {
        // The whole point of the per-(name, app) model: "fs" for claude and
        // "fs" for codex are unrelated entries, so they can carry entirely
        // different commands without any risk of one clobbering the other.
        let mut store = Store::empty();
        let fresh = fresh_map(&[
            ("claude", &[("fs", "npx-claude")]),
            ("codex", &[("fs", "npx-codex")]),
        ]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.added, 2);
        let claude_fs = store.servers.iter().find(|s| s.app == "claude").unwrap();
        let codex_fs = store.servers.iter().find(|s| s.app == "codex").unwrap();
        assert_eq!(claude_fs.command, Some("npx-claude".to_string()));
        assert_eq!(codex_fs.command, Some("npx-codex".to_string()));
    }

    #[test]
    fn enabled_server_still_present_is_refreshed_not_flagged() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "claude", "old-cmd", true, false));
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
        store.servers.push(stored("fs", "claude", "npx", true, false));
        // "claude" was scanned successfully this pass but no longer has "fs".
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 1);
        assert_eq!(store.servers.len(), 1, "entry must be kept, not removed");
        let fs = &store.servers[0];
        assert!(fs.deleted);
        assert!(!fs.enabled);
        assert_eq!(fs.command, Some("npx".to_string()), "last-known command preserved");
    }

    #[test]
    fn already_disabled_server_missing_live_is_left_alone() {
        // The case the user specifically asked about: a server the user
        // turned off themselves must never be touched just because the app
        // it used to live in doesn't have it (MCP Switch removed it there
        // itself when it was toggled off).
        let mut store = Store::empty();
        store.servers.push(stored("fs", "claude", "npx", false, false));
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0);
        assert!(!store.servers[0].deleted);
    }

    #[test]
    fn same_name_entry_in_a_different_app_is_never_affected_by_this_apps_drift() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "claude", "npx", true, false));
        store.servers.push(stored("fs", "codex", "npx", true, false));
        // Only "claude" drops it; "codex" isn't scanned this pass at all
        // (e.g. a transient read error), so it must be left exactly as-is.
        let fresh = fresh_map(&[("claude", &[])]);

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 1, "only the claude entry drifted");
        let claude_fs = store.servers.iter().find(|s| s.app == "claude").unwrap();
        let codex_fs = store.servers.iter().find(|s| s.app == "codex").unwrap();
        assert!(claude_fs.deleted);
        assert!(!codex_fs.deleted, "untouched: codex wasn't scanned this pass");
        assert!(codex_fs.enabled);
    }

    #[test]
    fn disabled_server_reappearing_live_is_turned_back_on() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "claude", "old-cmd", false, false));
        let fresh = fresh_map(&[("claude", &[("fs", "new-cmd")])]);

        reconcile(&mut store, &fresh);

        let fs = &store.servers[0];
        assert!(fs.enabled);
        assert_eq!(fs.command, Some("new-cmd".to_string()));
    }

    #[test]
    fn trashed_server_regaining_its_live_app_is_restored_automatically() {
        let mut store = Store::empty();
        store.servers.push(stored("fs", "claude", "npx", false, true));
        let fresh = fresh_map(&[("claude", &[("fs", "npx")])]);

        reconcile(&mut store, &fresh);

        assert!(!store.servers[0].deleted);
        assert!(store.servers[0].enabled);
    }

    #[test]
    fn app_absent_from_fresh_map_is_never_checked() {
        // Simulates a transient parse error on "codex": it's entirely absent
        // from `fresh`, so nothing enabled there should be touched at all.
        let mut store = Store::empty();
        store.servers.push(stored("fs", "codex", "npx", true, false));
        let fresh: HashMap<String, HashMap<String, McpServerEntry>> = HashMap::new();

        let summary = reconcile(&mut store, &fresh);

        assert_eq!(summary.flagged_deleted, 0);
        assert!(store.servers[0].enabled);
    }
}
