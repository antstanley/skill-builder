//! Integration tests for config module.

mod common;

use skill_builder::config::Config;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_load_config_from_file() {
    let config_path = common::testdata_dir().join("skills.json");
    let config = Config::load(&config_path).unwrap();

    assert!(!config.skills.is_empty());

    let test_skill = config.find_skill("test-skill");
    assert!(test_skill.is_some());
    assert_eq!(
        test_skill.unwrap().llms_txt_url,
        "https://example.com/llms.txt"
    );
}

#[test]
fn test_config_file_not_found() {
    let result = Config::load("/nonexistent/path/skills.json");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Failed to read config file"));
}

#[test]
fn test_config_with_multiple_skills() {
    let config_path = common::testdata_dir().join("skills.json");
    let config = Config::load(&config_path).unwrap();

    assert_eq!(config.skills.len(), 2);
    assert!(config.find_skill("test-skill").is_some());
    assert!(config.find_skill("another-skill").is_some());
}

#[test]
fn test_config_with_all_fields() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("skills.json");

    fs::write(
        &config_path,
        r#"{
        "skills": [
            {
                "name": "full-config-skill",
                "description": "A skill with all fields specified",
                "llms_txt_url": "https://example.com/llms.txt",
                "base_url": "https://docs.example.com",
                "path_prefix": "/api/docs"
            }
        ]
    }"#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    let skill = config.find_skill("full-config-skill").unwrap();

    assert_eq!(skill.description, "A skill with all fields specified");
    assert_eq!(skill.base_url.as_deref(), Some("https://docs.example.com"));
    assert_eq!(skill.path_prefix.as_deref(), Some("/api/docs"));
    assert_eq!(skill.get_base_url().unwrap(), "https://docs.example.com");
}

#[test]
fn test_config_base_url_derivation() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("skills.json");

    fs::write(
        &config_path,
        r#"{
        "skills": [
            {
                "name": "derive-url-skill",
                "llms_txt_url": "https://www.example.com/docs/llms.txt"
            }
        ]
    }"#,
    )
    .unwrap();

    let config = Config::load(&config_path).unwrap();
    let skill = config.find_skill("derive-url-skill").unwrap();

    // base_url should be derived from llms_txt_url
    assert!(skill.base_url.is_none());
    assert_eq!(skill.get_base_url().unwrap(), "https://www.example.com");
}

#[test]
fn test_skill_names() {
    let config_path = common::testdata_dir().join("skills.json");
    let config = Config::load(&config_path).unwrap();

    let names = config.skill_names();
    assert!(names.contains(&"test-skill"));
    assert!(names.contains(&"another-skill"));
}

#[test]
fn test_config_without_repository() {
    let config_path = common::testdata_dir().join("skills.json");
    let config = Config::load(&config_path).unwrap();

    assert!(config.repository.is_none());
}

#[test]
fn test_config_with_repository() {
    let config_path = common::testdata_dir().join("skills_with_repo.json");
    let config = Config::load(&config_path).unwrap();

    let repo = config.repository.unwrap();
    assert_eq!(repo.name.as_deref(), Some("test-repo"));
    assert_eq!(repo.bucket_name.as_deref(), Some("test-skills-bucket"));
    assert_eq!(repo.region, "us-west-2");
    assert_eq!(repo.endpoint.as_deref(), Some("https://s3.example.com"));
}

#[test]
fn test_config_backward_compatibility() {
    // Existing skills.json without repository should still parse
    let config_path = common::testdata_dir().join("skills.json");
    let config = Config::load(&config_path).unwrap();

    assert!(!config.skills.is_empty());
    assert!(config.repository.is_none());
}
