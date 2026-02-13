//! Integration tests for the repository module.

mod common;

use skill_builder::local_storage::LocalStorageClient;
use skill_builder::repository::Repository;
use skill_builder::s3::mock::MockS3Client;
use std::fs;
use tempfile::TempDir;

fn setup() -> (Repository<MockS3Client>, TempDir) {
    let tmp = TempDir::new().unwrap();
    let client = MockS3Client::new();
    let repo = Repository::new(client);
    (repo, tmp)
}

fn setup_with_cache() -> (Repository<MockS3Client>, TempDir) {
    let tmp = TempDir::new().unwrap();
    let cache = LocalStorageClient::new(tmp.path().join("cache").as_path()).unwrap();
    let client = MockS3Client::new();
    let repo = Repository::new_with_cache(client, cache);
    (repo, tmp)
}

fn create_test_skill_file(dir: &std::path::Path) -> std::path::PathBuf {
    let skill_dir = dir.join("repo-test-skill");
    common::create_valid_skill(&skill_dir);

    let dist = dir.join("dist");
    skill_builder::package::package_skill(&skill_dir, &dist).unwrap();
    dist.join("repo-test-skill.skill")
}

#[test]
fn test_upload_and_list() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload(
        "test-skill",
        "1.0.0",
        "A test skill",
        "https://example.com/llms.txt",
        &skill_file,
        None,
        None,
    )
    .unwrap();

    let index = repo.list(None).unwrap();
    assert_eq!(index.skills.len(), 1);
    assert_eq!(index.skills[0].name, "test-skill");
}

#[test]
fn test_upload_download_roundtrip() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());
    let original_data = fs::read(&skill_file).unwrap();

    repo.upload(
        "test-skill",
        "1.0.0",
        "desc",
        "url",
        &skill_file,
        None,
        None,
    )
    .unwrap();

    let output_dir = tmp.path().join("output");
    let downloaded = repo
        .download("test-skill", Some("1.0.0"), Some(&output_dir))
        .unwrap();

    assert!(downloaded.exists());
    assert_eq!(fs::read(&downloaded).unwrap(), original_data);
}

#[test]
fn test_upload_multiple_versions() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload("s", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();
    repo.upload("s", "2.0.0", "d", "u", &skill_file, None, None)
        .unwrap();

    let index = repo.list(None).unwrap();
    let entry = index.find_skill("s").unwrap();
    assert_eq!(entry.versions.len(), 2);
}

#[test]
fn test_delete_specific_version() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload("s", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();
    repo.upload("s", "2.0.0", "d", "u", &skill_file, None, None)
        .unwrap();

    repo.delete("s", Some("1.0.0")).unwrap();

    let index = repo.list(None).unwrap();
    let entry = index.find_skill("s").unwrap();
    assert_eq!(entry.versions.len(), 1);
    assert!(entry.versions.contains_key("2.0.0"));
}

#[test]
fn test_delete_all_versions() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload("s", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();

    repo.delete("s", None).unwrap();

    let index = repo.list(None).unwrap();
    assert!(index.skills.is_empty());
}

#[test]
fn test_list_with_filter() {
    let (repo, tmp) = setup();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload("a", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();
    repo.upload("b", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();

    let filtered = repo.list(Some("a")).unwrap();
    assert_eq!(filtered.skills.len(), 1);
    assert_eq!(filtered.skills[0].name, "a");
}

#[test]
fn test_download_caches_result() {
    let (repo, tmp) = setup_with_cache();
    let skill_file = create_test_skill_file(tmp.path());

    repo.upload("s", "1.0.0", "d", "u", &skill_file, None, None)
        .unwrap();

    let path1 = repo.download("s", Some("1.0.0"), None).unwrap();
    let path2 = repo.download("s", Some("1.0.0"), None).unwrap();
    assert!(path1.exists());
    assert!(path2.exists());
}

#[test]
fn test_download_nonexistent_skill_fails() {
    let (repo, _tmp) = setup();
    let result = repo.download("nonexistent", Some("1.0.0"), None);
    assert!(result.is_err());
}
