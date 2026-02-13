//! Local filesystem implementation of StorageOperations.

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::storage::StorageOperations;

/// Filesystem-backed storage client implementing StorageOperations.
///
/// Maps S3-like keys to filesystem paths relative to a root directory.
/// For example, key `"skills/foo/1.0.0/foo.skill"` becomes `{root}/skills/foo/1.0.0/foo.skill`.
pub struct LocalStorageClient {
    root: PathBuf,
}

impl LocalStorageClient {
    /// Create a new client, creating the root directory if it doesn't exist.
    pub fn new(root: &Path) -> Result<Self> {
        fs::create_dir_all(root).with_context(|| {
            format!(
                "Failed to create local storage directory: {}",
                root.display()
            )
        })?;
        Ok(Self {
            root: root.to_path_buf(),
        })
    }

    /// Create a client without creating the directory (for testing).
    pub fn with_dir(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// Get the root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve a key to a filesystem path.
    fn key_to_path(&self, key: &str) -> PathBuf {
        self.root.join(key)
    }
}

impl StorageOperations for LocalStorageClient {
    fn put_object(&self, key: &str, data: &[u8]) -> Result<()> {
        let path = self.key_to_path(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
        }
        fs::write(&path, data).with_context(|| format!("Failed to write: {}", path.display()))?;
        Ok(())
    }

    fn get_object(&self, key: &str) -> Result<Vec<u8>> {
        let path = self.key_to_path(key);
        fs::read(&path).with_context(|| format!("Object not found: {}", key))
    }

    fn delete_object(&self, key: &str) -> Result<()> {
        let path = self.key_to_path(key);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to delete: {}", path.display()))?;

            // Clean up empty parent directories up to root
            let mut dir = path.parent();
            while let Some(parent) = dir {
                if parent == self.root {
                    break;
                }
                if parent.exists()
                    && fs::read_dir(parent)
                        .map(|mut d| d.next().is_none())
                        .unwrap_or(false)
                {
                    fs::remove_dir(parent).ok();
                } else {
                    break;
                }
                dir = parent.parent();
            }
        }
        Ok(())
    }

    fn list_objects(&self, prefix: &str) -> Result<Vec<String>> {
        let base = self.key_to_path(prefix);
        let mut keys = Vec::new();

        // If the prefix path doesn't exist, return empty
        if !base.exists() {
            // Check if prefix matches as a file prefix in parent dir
            if let Some(parent) = base.parent() {
                if parent.exists() {
                    let prefix_name = base.file_name().map(|n| n.to_string_lossy().to_string());
                    if let Some(prefix_name) = prefix_name {
                        for entry in fs::read_dir(parent)? {
                            let entry = entry?;
                            let name = entry.file_name().to_string_lossy().to_string();
                            if name.starts_with(&prefix_name) {
                                let rel = entry
                                    .path()
                                    .strip_prefix(&self.root)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_default();
                                if entry.path().is_file() {
                                    keys.push(rel);
                                } else {
                                    collect_files_recursive(&entry.path(), &self.root, &mut keys)?;
                                }
                            }
                        }
                    }
                }
            }
            return Ok(keys);
        }

        if base.is_file() {
            let rel = base
                .strip_prefix(&self.root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            keys.push(rel);
        } else {
            collect_files_recursive(&base, &self.root, &mut keys)?;
        }

        Ok(keys)
    }

    fn object_exists(&self, key: &str) -> Result<bool> {
        Ok(self.key_to_path(key).is_file())
    }
}

fn collect_files_recursive(dir: &Path, root: &Path, keys: &mut Vec<String>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, root, keys)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            keys.push(rel);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_put_and_get() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        client
            .put_object("skills/foo/1.0.0/foo.skill", b"skill data")
            .unwrap();
        let data = client.get_object("skills/foo/1.0.0/foo.skill").unwrap();
        assert_eq!(data, b"skill data");
    }

    #[test]
    fn test_get_not_found() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        let result = client.get_object("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_and_cleanup() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        client
            .put_object("skills/foo/1.0.0/foo.skill", b"data")
            .unwrap();
        client.delete_object("skills/foo/1.0.0/foo.skill").unwrap();

        assert!(!client.object_exists("skills/foo/1.0.0/foo.skill").unwrap());
        // Parent directories should be cleaned up
        assert!(!tmp.path().join("store/skills/foo/1.0.0").exists());
    }

    #[test]
    fn test_list_objects() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        client.put_object("skills/a/1.0/a.skill", b"a").unwrap();
        client.put_object("skills/a/2.0/a.skill", b"a2").unwrap();
        client.put_object("skills/b/1.0/b.skill", b"b").unwrap();

        let mut keys = client.list_objects("skills/a/").unwrap();
        keys.sort();
        assert_eq!(keys, vec!["skills/a/1.0/a.skill", "skills/a/2.0/a.skill"]);
    }

    #[test]
    fn test_object_exists() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        assert!(!client.object_exists("key").unwrap());
        client.put_object("key", b"data").unwrap();
        assert!(client.object_exists("key").unwrap());
    }

    #[test]
    fn test_list_empty_prefix() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        let keys = client.list_objects("nonexistent/").unwrap();
        assert!(keys.is_empty());
    }

    #[test]
    fn test_delete_nonexistent_is_ok() {
        let tmp = TempDir::new().unwrap();
        let client = LocalStorageClient::new(tmp.path().join("store").as_path()).unwrap();

        client.delete_object("nonexistent").unwrap();
    }
}
