//! End-to-end CLI tests using assert_cmd.

mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

#[allow(deprecated)]
fn sb() -> Command {
    Command::cargo_bin("sb").unwrap()
}

#[test]
fn test_help() {
    sb().arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("builds Claude Code skills"));
}

#[test]
fn test_version() {
    sb().arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_download_help() {
    sb().args(["download", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Download documentation"));
}

#[test]
fn test_validate_help() {
    sb().args(["validate", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Validate"));
}

#[test]
fn test_package_help() {
    sb().args(["package", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Package"));
}

#[test]
fn test_install_help() {
    sb().args(["install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Install"));
}

#[test]
fn test_list_help() {
    sb().args(["list", "--help"]).assert().success();
}

#[test]
fn test_validate_valid_skill() {
    let skill_path = common::fixture_path("valid_skill");

    sb().args(["validate", &skill_path.to_string_lossy()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Skill is valid"));
}

#[test]
fn test_validate_invalid_skill_exits_nonzero() {
    let skill_path = common::fixture_path("invalid_skill");

    sb().args(["validate", &skill_path.to_string_lossy()])
        .assert()
        .failure()
        .stdout(predicate::str::contains("Validation failed"));
}

#[test]
fn test_validate_nonexistent_skill() {
    sb().args(["validate", "/nonexistent/skill"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_package_valid_skill() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("package-test-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    sb().args([
        "package",
        &skill_dir.to_string_lossy(),
        "--output",
        &output_dir.to_string_lossy(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Successfully packaged"));

    // Verify output file exists
    assert!(output_dir.join("package-test-skill.skill").exists());
}

#[test]
fn test_package_invalid_skill_fails() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("bad-skill");
    common::create_invalid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    sb().args([
        "package",
        &skill_dir.to_string_lossy(),
        "--output",
        &output_dir.to_string_lossy(),
    ])
    .assert()
    .failure();
}

#[test]
fn test_list_with_config() {
    let config_path = common::testdata_dir().join("skills.json");

    sb().args(["--config", &config_path.to_string_lossy(), "list"])
        .assert()
        .success()
        .stdout(predicate::str::contains("test-skill"))
        .stdout(predicate::str::contains("another-skill"));
}

#[test]
fn test_list_missing_config() {
    sb().args(["--config", "/nonexistent/skills.json", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config file"));
}

#[test]
fn test_download_requires_skill_or_flags() {
    let config_path = common::testdata_dir().join("skills.json");

    sb().args(["--config", &config_path.to_string_lossy(), "download"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("specify a skill name"));
}

#[test]
fn test_download_url_requires_name() {
    sb().args(["download", "--url", "https://example.com/llms.txt"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--name is required"));
}

#[test]
fn test_validate_with_skills_dir() {
    let temp = TempDir::new().unwrap();

    // Create skills directory structure
    let skills_dir = temp.path().join("skills");
    let skill_dir = skills_dir.join("my-skill");
    common::create_valid_skill(&skill_dir);

    sb().args([
        "validate",
        "my-skill",
        "--skills-dir",
        &skills_dir.to_string_lossy(),
    ])
    .assert()
    .success();
}

#[test]
fn test_package_with_skills_dir() {
    let temp = TempDir::new().unwrap();

    // Create skills directory structure
    let skills_dir = temp.path().join("skills");
    let skill_dir = skills_dir.join("my-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    sb().args([
        "package",
        "my-skill",
        "--skills-dir",
        &skills_dir.to_string_lossy(),
        "--output",
        &output_dir.to_string_lossy(),
    ])
    .assert()
    .success();

    assert!(output_dir.join("my-skill.skill").exists());
}

#[test]
fn test_install_from_file() {
    let temp = TempDir::new().unwrap();

    // Create and package a skill
    let skill_dir = temp.path().join("install-test-skill");
    common::create_valid_skill(&skill_dir);

    let package_dir = temp.path().join("packages");
    fs::create_dir_all(&package_dir).unwrap();

    sb().args([
        "package",
        &skill_dir.to_string_lossy(),
        "--output",
        &package_dir.to_string_lossy(),
    ])
    .assert()
    .success();

    // Install from the packaged file
    let install_dir = temp.path().join(".claude/skills");
    let skill_file = package_dir.join("install-test-skill.skill");

    sb().args([
        "install",
        "install-test-skill",
        "--file",
        &skill_file.to_string_lossy(),
        "--install-dir",
        &install_dir.to_string_lossy(),
    ])
    .assert()
    .success()
    .stdout(predicate::str::contains("Successfully installed"));

    // Verify installation
    assert!(install_dir.join("install-test-skill/SKILL.md").exists());
}

#[test]
fn test_repo_help() {
    sb().args(["repo", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("S3-compatible hosted repository"));
}

#[test]
fn test_repo_upload_help() {
    sb().args(["repo", "upload", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Upload"));
}

#[test]
fn test_repo_download_help() {
    sb().args(["repo", "download", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Download"));
}

#[test]
fn test_repo_install_help() {
    sb().args(["repo", "install", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Install"));
}

#[test]
fn test_repo_delete_help() {
    sb().args(["repo", "delete", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Delete"));
}

#[test]
fn test_repo_list_help() {
    sb().args(["repo", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List"));
}

#[test]
fn test_init_help() {
    sb().args(["init", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialize"));
}

#[test]
fn test_local_help() {
    sb().args(["local", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("local skill repository"));
}

#[test]
fn test_local_list_help() {
    sb().args(["local", "list", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("List"));
}

#[test]
fn test_local_clear_help() {
    sb().args(["local", "clear", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Clear"));
}

#[test]
fn test_local_list_runs() {
    sb().args(["local", "list"]).assert().success();
}

#[test]
fn test_error_messages_are_user_friendly() {
    // Missing config file should give clear error
    sb().args(["--config", "nonexistent.json", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Failed to read config file"));

    // Invalid skill should give clear error
    sb().args(["validate", "/does/not/exist"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
