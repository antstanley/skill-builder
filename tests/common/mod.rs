//! Shared test utilities and fixtures.

use std::fs;
use std::path::{Path, PathBuf};

/// Get the path to the testdata directory.
pub fn testdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

/// Get the path to a fixture.
pub fn fixture_path(name: &str) -> PathBuf {
    testdata_dir().join("fixtures").join(name)
}

/// Create a temporary skill directory with valid content.
pub fn create_valid_skill(dir: &Path) {
    fs::create_dir_all(dir).unwrap();

    fs::write(
        dir.join("SKILL.md"),
        r#"---
name: test-skill
description: A test skill for integration testing that has enough characters to pass the validation check
---

# Test Skill

This is a test skill created for integration testing.

## Usage

Use this skill for testing the skill-builder CLI.
"#,
    )
    .unwrap();

    fs::create_dir_all(dir.join("references")).unwrap();
    fs::write(
        dir.join("references/example.md"),
        "# Example\n\nExample reference content.",
    )
    .unwrap();
}

/// Create a temporary skill directory with invalid content.
pub fn create_invalid_skill(dir: &Path) {
    fs::create_dir_all(dir).unwrap();

    fs::write(
        dir.join("SKILL.md"),
        r#"# Missing Frontmatter

This skill has no YAML frontmatter.
"#,
    )
    .unwrap();
}
