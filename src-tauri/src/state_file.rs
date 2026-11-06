use serde::Serialize;
use std::{io::Write, path::Path};

/// Commit recovery data before changing hardware or an external application.
pub fn write_atomic(path: &Path, value: &impl Serialize) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(path.parent().ok_or("Missing app data directory")?)?;
    let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut file = std::fs::File::create(&temp)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(temp, path)?;
    Ok(())
}
