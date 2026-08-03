use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::types::McpError;

/// How long a `.bak.*` copy is kept before [`cleanup_old_backups`] deletes it.
const BACKUP_RETENTION_SECS: u64 = 7 * 24 * 60 * 60;

/// Copies `path`'s current on-disk content to a timestamped `<name>.bak.<unix
/// seconds>` next to it, then sweeps the directory for backups older than
/// [`BACKUP_RETENTION_SECS`] and deletes them. Meant to be called right
/// before overwriting any config file MCP Switch doesn't itself own the only
/// copy of — its own `store.json` (on every save, and whenever a load fails
/// to parse) as well as every adapter's write-back into a *tool's* live
/// config — so a bad write (bug, unexpected schema, disk hiccup) never loses
/// the last-known-good version. A no-op if `path` doesn't exist yet —
/// nothing to back up. Best-effort throughout: a failure to back up or clean
/// up is logged, never fatal, since refusing to save/load over a backup
/// hiccup would be worse than the data-loss risk this exists to prevent.
pub fn backup_file(path: &Path) {
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
        .unwrap_or_else(|| std::ffi::OsString::from("backup"));
    let mut backup_name = file_name;
    backup_name.push(format!(".bak.{now}"));
    let backup_path = path.with_file_name(backup_name);

    match std::fs::copy(path, &backup_path) {
        Ok(_) => {}
        Err(backup_err) => eprintln!(
            "Failed to back up {} to {}: {backup_err}",
            path.display(),
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

/// Write content to a file atomically by writing to a temp file first,
/// then renaming it over the target. This prevents partial writes.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), McpError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write to a temporary file next to the target. The temp name is
    // process-id + nanosecond so two concurrent writes (or a straggler
    // from a crashed prior write) can't collide on the same path and
    // either fail to rename or, worse, half-overwrite each other.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_file_name = format!(
        "{}.tmp.{}.{}",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp"),
        std::process::id(),
        nanos
    );
    let tmp_path = path.with_file_name(tmp_file_name);
    let mut tmp = std::fs::File::create(&tmp_path)?;
    tmp.write_all(content.as_bytes())?;
    tmp.sync_all()?;
    drop(tmp);

    // Atomically rename temp -> target. On Windows, rename doesn't
    // overwrite an existing file, so remove it first.
    let _ = std::fs::remove_file(path);
    if let Err(e) = std::fs::rename(&tmp_path, path) {
        // Don't leave a stranded temp file on failure.
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e.into());
    }

    Ok(())
}

/// Read a file, returning None if it doesn't exist.
pub fn read_file_optional(path: &Path) -> Result<Option<String>, McpError> {
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn atomic_write_uses_unique_temp_per_call() {
        // Two writes to the same target must not collide on a single
        // shared temp name; the second one would otherwise see the
        // first's temp file still in place and either rename the wrong
        // file or lose data.
        let path = std::env::temp_dir().join("mcp_switch_test_atomic_unique.json");
        let _ = std::fs::remove_file(&path);

        atomic_write(&path, "first").unwrap();
        // Sleep a hair so the second call's nanosecond timestamp is
        // distinct on platforms with low-resolution timers.
        std::thread::sleep(std::time::Duration::from_millis(2));
        atomic_write(&path, "second").unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");

        // No stray .tmp.* files left in the parent dir.
        let dir = path.parent().unwrap();
        let prefix = path.file_name().unwrap().to_str().unwrap().to_string();
        let stray: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(&format!("{prefix}.tmp.")))
            .collect();
        assert!(stray.is_empty(), "atomic_write left a stranded temp file");

        std::fs::remove_file(&path).unwrap();
    }
    #[test]
    fn backup_file_preserves_original_content() {
        // Uses the OS temp dir, never a real config path, so this test can
        // never touch (let alone lose) anything real.
        let path = std::env::temp_dir().join("mcp_switch_test_atomic_backup.json");
        std::fs::write(&path, "not valid json").unwrap();

        backup_file(&path);

        let backups: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("mcp_switch_test_atomic_backup.json.bak.")
            })
            .collect();
        assert_eq!(backups.len(), 1, "expected exactly one backup file");
        assert_eq!(
            std::fs::read_to_string(backups[0].path()).unwrap(),
            "not valid json"
        );

        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(backups[0].path()).unwrap();
    }

    #[test]
    fn backup_file_is_a_noop_when_nothing_exists_yet() {
        let path = std::env::temp_dir().join("mcp_switch_test_atomic_backup_missing.json");
        let _ = std::fs::remove_file(&path); // in case a previous run left one behind

        backup_file(&path); // must not panic or create anything

        let stray: Vec<_> = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("mcp_switch_test_atomic_backup_missing.json.bak.")
            })
            .collect();
        assert!(stray.is_empty(), "nothing to back up, so no backup should appear");
    }

    #[test]
    fn cleanup_old_backups_removes_only_entries_past_retention() {
        let path = std::env::temp_dir().join("mcp_switch_test_atomic_cleanup_target.json");
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
}
