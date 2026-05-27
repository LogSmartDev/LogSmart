use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::fs;

const EXPORT_RETENTION_DAYS: i64 = 7;

pub struct ExportRecord {
    pub company_id: String,
    pub file_path: PathBuf,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub fn exports_dir() -> PathBuf {
    std::env::var("EXPORT_DIR").map_or_else(|_| PathBuf::from("exports"), PathBuf::from)
}

fn safe_path(dir: &Path, filename: &str) -> Result<PathBuf> {
    if filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
    {
        anyhow::bail!("Invalid filename");
    }

    let safe_name = Path::new(filename)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

    let canonical_dir = dir
        .canonicalize()
        .with_context(|| format!("Exports directory does not exist: {}", dir.display()))?;
    let path = canonical_dir.join(safe_name);

    Ok(path)
}

pub async fn init_exports_dir() -> Result<()> {
    let dir = exports_dir();
    fs::create_dir_all(&dir)
        .await
        .with_context(|| format!("Failed to create exports directory: {}", dir.display()))?;
    Ok(())
}

pub async fn save_export(company_id: &str, data: &[u8]) -> Result<String> {
    let dir = exports_dir();
    let timestamp = chrono::Utc::now().timestamp_millis();
    let filename = format!("{company_id}_{timestamp}.zip");
    let file_path = safe_path(&dir, &filename)?;

    fs::write(&file_path, data)
        .await
        .with_context(|| format!("Failed to write export file: {}", file_path.display()))?;

    tracing::info!("Export saved: {} ({} bytes)", filename, data.len());
    Ok(filename)
}

pub async fn get_export(_company_id: &str, filename: &str) -> Result<Option<Vec<u8>>> {
    let dir = exports_dir();
    let file_path = safe_path(&dir, filename)?;

    if !file_path.exists() {
        return Ok(None);
    }

    let content = fs::read(&file_path)
        .await
        .with_context(|| format!("Failed to read export file: {}", file_path.display()))?;

    Ok(Some(content))
}

pub async fn delete_export(filename: &str) -> Result<()> {
    let dir = exports_dir();
    let file_path = safe_path(&dir, filename)?;

    if file_path.exists() {
        fs::remove_file(&file_path)
            .await
            .with_context(|| format!("Failed to delete export file: {}", file_path.display()))?;
    }

    Ok(())
}

pub async fn cleanup_old_exports() -> Result<u32> {
    let dir = exports_dir();
    let cutoff = chrono::Utc::now() - chrono::Duration::days(EXPORT_RETENTION_DAYS);
    let mut deleted = 0u32;

    let mut entries = fs::read_dir(&dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("zip") {
            continue;
        }

        let metadata = fs::metadata(&path).await?;
        let modified = metadata.modified().ok().and_then(|t| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(
                t.duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs() as i64),
                0,
            )
        });

        if let Some(modified) = modified
            && modified < cutoff
        {
            if let Err(e) = fs::remove_file(&path).await {
                tracing::warn!("Failed to delete old export {}: {e}", path.display());
            } else {
                deleted += 1;
            }
        }
    }

    if deleted > 0 {
        tracing::info!("Cleaned up {deleted} old export files");
    }

    Ok(deleted)
}

pub fn spawn_cleanup_task() {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3600));
        loop {
            interval.tick().await;
            if let Err(e) = cleanup_old_exports().await {
                tracing::error!("Export cleanup failed: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    #[serial]
    fn test_safe_path_accepts_valid_filename() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        let result = safe_path(&dir, "abc_123.zip");
        if result.is_err() {
            println!("Error: {result:?}");
        }
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            PathBuf::from("/tmp/test_exports/abc_123.zip")
        );
    }

    #[test]
    #[serial]
    fn test_safe_path_rejects_double_dot() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(safe_path(&dir, "../etc/passwd").is_err());
        assert!(safe_path(&dir, "foo/../bar.zip").is_err());
        assert!(safe_path(&dir, "..").is_err());
    }

    #[test]
    #[serial]
    fn test_safe_path_rejects_forward_slash() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(safe_path(&dir, "foo/bar.zip").is_err());
        assert!(safe_path(&dir, "/etc/passwd").is_err());
    }

    #[test]
    #[serial]
    fn test_safe_path_rejects_backslash() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(safe_path(&dir, "foo\\bar.zip").is_err());
    }

    #[test]
    #[serial]
    fn test_safe_path_rejects_null_byte() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(safe_path(&dir, "foo\0bar.zip").is_err());
    }

    #[test]
    #[serial]
    fn test_safe_path_rejects_empty_filename() {
        let dir = PathBuf::from("/tmp/test_exports");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(safe_path(&dir, "").is_err());
    }

    #[test]
    #[serial]
    fn test_exports_dir_uses_env_override() {
        unsafe { std::env::set_var("EXPORT_DIR", "/custom/exports") };
        assert_eq!(exports_dir(), PathBuf::from("/custom/exports"));
        unsafe { std::env::remove_var("EXPORT_DIR") };
    }

    #[test]
    #[serial]
    fn test_exports_dir_defaults_to_exports() {
        unsafe { std::env::remove_var("EXPORT_DIR") };
        assert_eq!(exports_dir(), PathBuf::from("exports"));
    }
}
