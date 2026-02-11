//! Integration tests for package module.

mod common;

use skill_builder::package::{list_skill_contents, package_skill};
use std::fs;
use tempfile::TempDir;
use zip::ZipArchive;

#[test]
fn test_package_fixture_skill() {
    let skill_path = common::fixture_path("valid_skill");
    let temp = TempDir::new().unwrap();
    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_path, &output_dir).unwrap();

    assert!(result.output_path.exists());
    assert!(result.output_path.to_string_lossy().ends_with(".skill"));
    assert!(result.validation.valid);
}

#[test]
fn test_package_temp_skill() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("packaged-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    assert!(result.output_path.exists());
    assert_eq!(result.files_included, 2); // SKILL.md + references/example.md
}

#[test]
fn test_package_creates_valid_zip() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("zip-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    // Verify it's a valid zip file
    let file = fs::File::open(&result.output_path).unwrap();
    let archive = ZipArchive::new(file);
    assert!(archive.is_ok(), "Output should be a valid zip file");
}

#[test]
fn test_package_correct_structure() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("structure-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    let contents = list_skill_contents(&result.output_path).unwrap();

    // All entries should start with skill name
    for entry in &contents {
        assert!(
            entry.starts_with("structure-skill/"),
            "Entry '{}' should start with 'structure-skill/'",
            entry
        );
    }

    // Should contain SKILL.md
    assert!(contents.iter().any(|c| c.ends_with("SKILL.md")));

    // Should contain references
    assert!(contents.iter().any(|c| c.contains("references/")));
}

#[test]
fn test_package_skips_hidden_files() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("hidden-skill");
    common::create_valid_skill(&skill_dir);

    // Add hidden file
    fs::write(skill_dir.join(".hidden"), "hidden content").unwrap();

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    let contents = list_skill_contents(&result.output_path).unwrap();

    // Should not contain hidden file
    assert!(!contents.iter().any(|c| c.contains(".hidden")));
}

#[test]
fn test_package_skips_pyc_files() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("pyc-skill");
    common::create_valid_skill(&skill_dir);

    // Add .pyc file
    fs::write(skill_dir.join("cache.pyc"), "bytecode").unwrap();

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    let contents = list_skill_contents(&result.output_path).unwrap();

    // Should not contain .pyc file
    assert!(!contents.iter().any(|c| c.ends_with(".pyc")));
}

#[test]
fn test_package_roundtrip() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("roundtrip-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");
    let extract_dir = temp.path().join("extracted");

    // Package
    let result = package_skill(&skill_dir, &output_dir).unwrap();

    // Extract
    let file = fs::File::open(&result.output_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    archive.extract(&extract_dir).unwrap();

    // Verify extracted contents
    let extracted_skill = extract_dir.join("roundtrip-skill");
    assert!(extracted_skill.join("SKILL.md").exists());
    assert!(extracted_skill.join("references/example.md").exists());

    // Verify content matches
    let original = fs::read_to_string(skill_dir.join("SKILL.md")).unwrap();
    let extracted = fs::read_to_string(extracted_skill.join("SKILL.md")).unwrap();
    assert_eq!(original, extracted);
}

#[test]
fn test_package_invalid_skill_fails() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("invalid-skill");
    common::create_invalid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir);

    assert!(result.is_err());
}

#[test]
fn test_package_nonexistent_skill_fails() {
    let temp = TempDir::new().unwrap();
    let output_dir = temp.path().join("dist");

    let result = package_skill("/nonexistent/skill", &output_dir);

    assert!(result.is_err());
}

#[test]
fn test_list_skill_contents() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("list-skill");
    common::create_valid_skill(&skill_dir);

    let output_dir = temp.path().join("dist");

    let result = package_skill(&skill_dir, &output_dir).unwrap();

    let contents = list_skill_contents(&result.output_path).unwrap();

    assert!(!contents.is_empty());
    assert!(contents.iter().any(|c| c.contains("SKILL.md")));
}
