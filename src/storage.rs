use anyhow::Result;
use std::path::PathBuf;

/// Threshold in bytes: payloads larger than this go to disk.
pub const PAYLOAD_THRESHOLD: usize = 4096;

/// Get the payloads directory root.
pub fn payloads_dir() -> PathBuf {
    crate::db::data_dir().join("payloads")
}

/// Get the payload directory for a specific session.
pub fn session_payload_dir(session_id: &str) -> PathBuf {
    payloads_dir().join(session_id)
}

/// Write a payload to disk. Returns the file path (relative to data dir).
pub fn write_payload(session_id: &str, event_uuid: &str, payload: &str) -> Result<String> {
    let dir = session_payload_dir(session_id);
    std::fs::create_dir_all(&dir)?;
    let filename = format!("{}.json", event_uuid);
    let path = dir.join(&filename);
    std::fs::write(&path, payload)?;
    // Return the absolute path for storage in DB
    Ok(path.to_string_lossy().to_string())
}

/// Read a payload from disk.
pub fn read_payload(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)?;
    Ok(content)
}

/// Delete a payload file.
pub fn delete_payload(path: &str) -> Result<()> {
    if std::path::Path::new(path).exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Clean up empty session payload directories.
pub fn cleanup_empty_dirs() -> Result<()> {
    let root = payloads_dir();
    if !root.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            // Remove if empty
            if std::fs::read_dir(entry.path())?.next().is_none() {
                std::fs::remove_dir(entry.path())?;
            }
        }
    }
    Ok(())
}
