use std::io::Write;
use std::path::Path;
use crate::types::McpError;

/// Write content to a file atomically by writing to a temp file first,
/// then renaming it over the target. This prevents partial writes.
pub fn atomic_write(path: &Path, content: &str) -> Result<(), McpError> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Write to a temporary file next to the target
    let tmp_path = path.with_extension("tmp");
    let mut tmp = std::fs::File::create(&tmp_path)?;
    tmp.write_all(content.as_bytes())?;
    tmp.sync_all()?;
    drop(tmp);

    // Atomically rename temp -> target
    std::fs::rename(&tmp_path, path)?;

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
