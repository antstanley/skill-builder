//! Integration tests for validate module.

mod common;

use skill_builder::validate::{validate_skill, ValidationError};
use tempfile::TempDir;

#[test]
fn test_validate_fixture_valid_skill() {
    let skill_path = common::fixture_path("valid_skill");
    let result = validate_skill(&skill_path);

    assert!(result.valid, "Expected valid skill, got errors: {:?}", result.errors);
    assert!(result.errors.is_empty());
}

#[test]
fn test_validate_fixture_invalid_skill() {
    let skill_path = common::fixture_path("invalid_skill");
    let result = validate_skill(&skill_path);

    assert!(!result.valid);
    assert!(result.errors.contains(&ValidationError::MissingFrontmatter));
}

#[test]
fn test_validate_temp_valid_skill() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("my-skill");
    common::create_valid_skill(&skill_dir);

    let result = validate_skill(&skill_dir);

    assert!(result.valid, "Expected valid skill, got errors: {:?}", result.errors);
}

#[test]
fn test_validate_temp_invalid_skill() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("bad-skill");
    common::create_invalid_skill(&skill_dir);

    let result = validate_skill(&skill_dir);

    assert!(!result.valid);
}

#[test]
fn test_validate_nonexistent_skill() {
    let result = validate_skill("/nonexistent/path/skill");

    assert!(!result.valid);
    assert!(result.errors.contains(&ValidationError::SkillMdNotFound));
}

#[test]
fn test_validate_multiple_errors() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("multi-error-skill");

    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name:
description: short
---

# Skill

[TODO: incomplete]
"#,
    )
    .unwrap();

    let result = validate_skill(&skill_dir);

    assert!(!result.valid);
    assert!(result.errors.len() >= 2, "Expected multiple errors");
}

#[test]
fn test_validate_warnings() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("warning-skill");

    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: warning-skill
description: A skill without references directory that still passes validation checks
---

# Warning Skill
"#,
    )
    .unwrap();

    // No references directory created

    let result = validate_skill(&skill_dir);

    assert!(result.valid); // Still valid
    assert!(!result.warnings.is_empty()); // But has warnings
}

#[test]
fn test_validate_description_length() {
    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("desc-skill");

    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        r#"---
name: test
description: Exactly fifty characters long description!
---

# Test
"#,
    )
    .unwrap();

    let result = validate_skill(&skill_dir);

    // "Exactly fifty characters long description!" is 43 chars, should fail
    assert!(!result.valid);
}
