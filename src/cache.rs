//! Local cache for downloaded skills.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Metadata stored alongside cached skill files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMetadata {
    /// Skill name.
    pub name: String,
    /// Cached version.
    pub version: String,
    /// Source (e.g. "s3://bucket/path" or "github").
    pub source: String,
    /// ISO 8601 timestamp of when the cache entry was created.
    pub cached_at: String,
}

/// Local skill cache manager.
pub struct SkillCache {
    cache_dir: PathBuf,
}

impl SkillCache {
    /// Create a cache using the platform-appropriate cache directory.
    pub fn new() -> Result<Self> {
        let base = dirs::cache_dir().context("Could not determine cache directory")?;
        let cache_dir = base.join("skill-builder").join("skills");
        Ok(Self { cache_dir })
    }

    /// Create a cache at a specific directory (for testing).
    pub fn with_dir<P: AsRef<Path>>(path: P) -> Self {
        Self {
            cache_dir: path.as_ref().to_path_buf(),
        }
    }

    /// Get the path to the cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if a skill version is cached and return its path.
    pub fn get(&self, name: &str, version: &str) -> Option<PathBuf> {
        let skill_file = self.skill_path(name, version);
        if skill_file.exists() {
            Some(skill_file)
        } else {
            None
        }
    }

    /// Store a skill file in the cache. Returns the cached file path.
    pub fn store(&self, name: &str, version: &str, data: &[u8], source: &str) -> Result<PathBuf> {
        let dir = self.version_dir(name, version);
        fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create cache directory: {}", dir.display()))?;

        let skill_file = dir.join(format!("{}.skill", name));
        fs::write(&skill_file, data)
            .with_context(|| format!("Failed to write cache file: {}", skill_file.display()))?;

        let metadata = CacheMetadata {
            name: name.to_string(),
            version: version.to_string(),
            source: source.to_string(),
            cached_at: chrono::Utc::now().to_rfc3339(),
        };
        let metadata_path = dir.join("metadata.json");
        let metadata_json = serde_json::to_string_pretty(&metadata)
            .context("Failed to serialize cache metadata")?;
        fs::write(&metadata_path, metadata_json)
            .with_context(|| format!("Failed to write metadata: {}", metadata_path.display()))?;

        Ok(skill_file)
    }

    /// Remove a specific version from the cache.
    pub fn remove(&self, name: &str, version: &str) -> Result<()> {
        let dir = self.version_dir(name, version);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .with_context(|| format!("Failed to remove cache: {}", dir.display()))?;
        }

        // Clean up empty skill directory
        let skill_dir = self.skill_dir(name);
        if skill_dir.exists() && fs::read_dir(&skill_dir)?.next().is_none() {
            fs::remove_dir(&skill_dir).ok();
        }

        Ok(())
    }

    /// Remove all cached versions of a skill.
    pub fn remove_all(&self, name: &str) -> Result<()> {
        let skill_dir = self.skill_dir(name);
        if skill_dir.exists() {
            fs::remove_dir_all(&skill_dir)
                .with_context(|| format!("Failed to remove cache: {}", skill_dir.display()))?;
        }
        Ok(())
    }

    /// List all cached skill name/version pairs.
    pub fn list_cached(&self) -> Result<Vec<(String, String)>> {
        let mut entries = Vec::new();

        if !self.cache_dir.exists() {
            return Ok(entries);
        }

        for skill_entry in fs::read_dir(&self.cache_dir)? {
            let skill_entry = skill_entry?;
            if !skill_entry.file_type()?.is_dir() {
                continue;
            }
            let skill_name = skill_entry.file_name().to_string_lossy().to_string();

            for version_entry in fs::read_dir(skill_entry.path())? {
                let version_entry = version_entry?;
                if !version_entry.file_type()?.is_dir() {
                    continue;
                }
                let version = version_entry.file_name().to_string_lossy().to_string();
                entries.push((skill_name.clone(), version));
            }
        }

        entries.sort();
        Ok(entries)
    }

    fn skill_dir(&self, name: &str) -> PathBuf {
        self.cache_dir.join(name)
    }

    fn version_dir(&self, name: &str, version: &str) -> PathBuf {
        self.cache_dir.join(name).join(version)
    }

    fn skill_path(&self, name: &str, version: &str) -> PathBuf {
        self.version_dir(name, version)
            .join(format!("{}.skill", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_cache() -> (SkillCache, TempDir) {
        let tmp = TempDir::new().unwrap();
        let cache = SkillCache::with_dir(tmp.path().join("cache"));
        (cache, tmp)
    }

    #[test]
    fn test_get_returns_none_when_empty() {
        let (cache, _tmp) = test_cache();
        assert!(cache.get("skill", "1.0.0").is_none());
    }

    #[test]
    fn test_store_and_get() {
        let (cache, _tmp) = test_cache();
        let path = cache
            .store("my-skill", "1.0.0", b"skill data", "test")
            .unwrap();

        assert!(path.exists());
        assert_eq!(fs::read(&path).unwrap(), b"skill data");
        assert_eq!(cache.get("my-skill", "1.0.0"), Some(path));
    }

    #[test]
    fn test_store_creates_metadata() {
        let (cache, _tmp) = test_cache();
        cache
            .store("my-skill", "1.0.0", b"data", "s3://bucket/path")
            .unwrap();

        let metadata_path = cache.version_dir("my-skill", "1.0.0").join("metadata.json");
        assert!(metadata_path.exists());

        let meta: CacheMetadata =
            serde_json::from_str(&fs::read_to_string(metadata_path).unwrap()).unwrap();
        assert_eq!(meta.name, "my-skill");
        assert_eq!(meta.version, "1.0.0");
        assert_eq!(meta.source, "s3://bucket/path");
    }

    #[test]
    fn test_remove_version() {
        let (cache, _tmp) = test_cache();
        cache.store("my-skill", "1.0.0", b"v1", "test").unwrap();
        cache.store("my-skill", "2.0.0", b"v2", "test").unwrap();

        cache.remove("my-skill", "1.0.0").unwrap();
        assert!(cache.get("my-skill", "1.0.0").is_none());
        assert!(cache.get("my-skill", "2.0.0").is_some());
    }

    #[test]
    fn test_remove_all() {
        let (cache, _tmp) = test_cache();
        cache.store("my-skill", "1.0.0", b"v1", "test").unwrap();
        cache.store("my-skill", "2.0.0", b"v2", "test").unwrap();

        cache.remove_all("my-skill").unwrap();
        assert!(cache.get("my-skill", "1.0.0").is_none());
        assert!(cache.get("my-skill", "2.0.0").is_none());
    }

    #[test]
    fn test_list_cached() {
        let (cache, _tmp) = test_cache();
        cache.store("alpha", "1.0.0", b"a1", "test").unwrap();
        cache.store("alpha", "2.0.0", b"a2", "test").unwrap();
        cache.store("beta", "1.0.0", b"b1", "test").unwrap();

        let entries = cache.list_cached().unwrap();
        assert_eq!(
            entries,
            vec![
                ("alpha".to_string(), "1.0.0".to_string()),
                ("alpha".to_string(), "2.0.0".to_string()),
                ("beta".to_string(), "1.0.0".to_string()),
            ]
        );
    }

    #[test]
    fn test_list_cached_empty() {
        let (cache, _tmp) = test_cache();
        let entries = cache.list_cached().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_is_ok() {
        let (cache, _tmp) = test_cache();
        cache.remove("nonexistent", "1.0.0").unwrap();
        cache.remove_all("nonexistent").unwrap();
    }
}
