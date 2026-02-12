//! Integration tests for the cache module.

use skill_builder::cache::SkillCache;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_cache_store_and_retrieve() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    let path = cache
        .store("my-skill", "1.0.0", b"skill content here", "test-source")
        .unwrap();

    assert!(path.exists());
    assert_eq!(fs::read(&path).unwrap(), b"skill content here");

    let cached = cache.get("my-skill", "1.0.0");
    assert_eq!(cached, Some(path));
}

#[test]
fn test_cache_miss() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    assert!(cache.get("nonexistent", "1.0.0").is_none());
}

#[test]
fn test_cache_multiple_versions() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    cache.store("skill", "1.0.0", b"v1", "src").unwrap();
    cache.store("skill", "2.0.0", b"v2", "src").unwrap();

    let v1 = cache.get("skill", "1.0.0").unwrap();
    let v2 = cache.get("skill", "2.0.0").unwrap();

    assert_eq!(fs::read(&v1).unwrap(), b"v1");
    assert_eq!(fs::read(&v2).unwrap(), b"v2");
}

#[test]
fn test_cache_list() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    cache.store("alpha", "1.0.0", b"a", "src").unwrap();
    cache.store("beta", "2.0.0", b"b", "src").unwrap();

    let entries = cache.list_cached().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.contains(&("alpha".to_string(), "1.0.0".to_string())));
    assert!(entries.contains(&("beta".to_string(), "2.0.0".to_string())));
}

#[test]
fn test_cache_remove_version() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    cache.store("skill", "1.0.0", b"v1", "src").unwrap();
    cache.store("skill", "2.0.0", b"v2", "src").unwrap();

    cache.remove("skill", "1.0.0").unwrap();

    assert!(cache.get("skill", "1.0.0").is_none());
    assert!(cache.get("skill", "2.0.0").is_some());
}

#[test]
fn test_cache_remove_all() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    cache.store("skill", "1.0.0", b"v1", "src").unwrap();
    cache.store("skill", "2.0.0", b"v2", "src").unwrap();

    cache.remove_all("skill").unwrap();

    assert!(cache.get("skill", "1.0.0").is_none());
    assert!(cache.get("skill", "2.0.0").is_none());
    assert!(cache.list_cached().unwrap().is_empty());
}

#[test]
fn test_cache_metadata_written() {
    let tmp = TempDir::new().unwrap();
    let cache = SkillCache::with_dir(tmp.path().join("cache"));

    cache
        .store("my-skill", "1.0.0", b"data", "s3://bucket/path")
        .unwrap();

    let metadata_path = tmp.path().join("cache/my-skill/1.0.0/metadata.json");
    assert!(metadata_path.exists());

    let meta_json = fs::read_to_string(metadata_path).unwrap();
    assert!(meta_json.contains("\"name\": \"my-skill\""));
    assert!(meta_json.contains("\"version\": \"1.0.0\""));
    assert!(meta_json.contains("s3://bucket/path"));
}
