//! Package skills into distributable .skill files.

use anyhow::{Context, Result};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::output::Output;
use crate::validate::{validate_skill, ValidationResult};

/// Files and directories to skip when packaging.
const SKIP_EXTENSIONS: &[&str] = &["pyc", "pyo"];
const SKIP_FILES: &[&str] = &["__pycache__", ".DS_Store", "Thumbs.db"];

/// Check if a path should be skipped during packaging.
fn should_skip(path: &Path) -> bool {
    // Skip hidden files and directories
    if path
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return true;
    }

    // Skip by extension
    if let Some(ext) = path.extension() {
        if SKIP_EXTENSIONS.contains(&ext.to_string_lossy().as_ref()) {
            return true;
        }
    }

    // Skip by filename
    if let Some(name) = path.file_name() {
        if SKIP_FILES.contains(&name.to_string_lossy().as_ref()) {
            return true;
        }
    }

    false
}

/// Collect all files to include in the package.
fn collect_files(skill_path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    fn visit_dir(dir: &Path, base: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let relative = path.strip_prefix(base).unwrap_or(&path);

            if should_skip(relative) {
                continue;
            }

            if path.is_dir() {
                visit_dir(&path, base, files)?;
            } else if path.is_file() {
                files.push(path);
            }
        }
        Ok(())
    }

    visit_dir(skill_path, skill_path, &mut files)?;
    files.sort();

    Ok(files)
}

/// Result of packaging operation.
#[derive(Debug)]
pub struct PackageResult {
    pub output_path: PathBuf,
    pub files_included: usize,
    pub validation: ValidationResult,
}

/// Package a skill directory into a .skill file (silent output for internal use).
pub fn package_skill<P: AsRef<Path>, Q: AsRef<Path>>(
    skill_path: P,
    output_dir: Q,
) -> Result<PackageResult> {
    let silent = Output::new(true);
    package_skill_with_output(skill_path, output_dir, &silent)
}

/// Package a skill directory into a .skill file with output.
pub fn package_skill_with_output<P: AsRef<Path>, Q: AsRef<Path>>(
    skill_path: P,
    output_dir: Q,
    output: &Output,
) -> Result<PackageResult> {
    let skill_path = skill_path.as_ref();
    let output_dir = output_dir.as_ref();

    // Get skill name from directory
    let skill_name = skill_path
        .file_name()
        .context("Invalid skill path")?
        .to_string_lossy();

    output.header(&format!("Packaging skill: {}", skill_path.display()));
    output.step(&format!("Output directory: {}", output_dir.display()));
    output.newline();

    // Validate first
    let validation = validate_skill(skill_path);

    if !validation.valid {
        output.error("Validation failed:");
        for error in &validation.errors {
            output.step(&format!("- {}", error));
        }
        anyhow::bail!("Skill validation failed");
    }

    output.status("Valid", "Skill is valid!");
    output.newline();

    // Create output directory
    fs::create_dir_all(output_dir)?;

    // Collect files
    let files = collect_files(skill_path)?;

    // Create output file
    let output_path = output_dir.join(format!("{}.skill", skill_name));
    let file = File::create(&output_path)?;
    let mut zip = ZipWriter::new(file);

    let zip_options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // Add files to archive
    let progress = output.progress_bar(files.len() as u64, "Adding files");

    for file_path in &files {
        let relative_path = file_path.strip_prefix(skill_path)?;
        let archive_path = PathBuf::from(skill_name.as_ref()).join(relative_path);

        zip.start_file(archive_path.to_string_lossy(), zip_options)?;

        let mut f = File::open(file_path)?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        zip.write_all(&buffer)?;

        progress.inc(1);
    }

    progress.finish_and_clear();
    zip.finish()?;

    output.status("Packaged", &format!("{}", output_path.display()));

    Ok(PackageResult {
        output_path,
        files_included: files.len(),
        validation,
    })
}

/// List contents of a .skill file.
pub fn list_skill_contents<P: AsRef<Path>>(skill_file: P) -> Result<Vec<String>> {
    let skill_file = skill_file.as_ref();
    let file = File::open(skill_file)?;
    let mut archive = zip::ZipArchive::new(file)?;

    let mut contents = Vec::new();
    for i in 0..archive.len() {
        let file = archive.by_index(i)?;
        contents.push(file.name().to_string());
    }

    contents.sort();
    Ok(contents)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path) {
        fs::create_dir_all(dir).unwrap();

        // Create SKILL.md with valid frontmatter
        fs::write(
            dir.join("SKILL.md"),
            r#"---
name: test-skill
description: A test skill for unit tests that validates the packaging functionality works correctly
---

# Test Skill

This is a test skill.
"#,
        )
        .unwrap();

        // Create references directory with a file
        fs::create_dir_all(dir.join("references")).unwrap();
        fs::write(dir.join("references/example.md"), "# Example\n\nContent.").unwrap();
    }

    #[test]
    fn test_should_skip_hidden() {
        assert!(should_skip(Path::new(".git/config")));
        assert!(should_skip(Path::new(".DS_Store")));
        assert!(should_skip(Path::new("dir/.hidden")));
    }

    #[test]
    fn test_should_skip_extensions() {
        assert!(should_skip(Path::new("file.pyc")));
        assert!(should_skip(Path::new("file.pyo")));
    }

    #[test]
    fn test_should_skip_files() {
        assert!(should_skip(Path::new("__pycache__")));
        assert!(should_skip(Path::new("Thumbs.db")));
    }

    #[test]
    fn test_should_not_skip_normal() {
        assert!(!should_skip(Path::new("SKILL.md")));
        assert!(!should_skip(Path::new("references/example.md")));
        assert!(!should_skip(Path::new("file.txt")));
    }

    #[test]
    fn test_collect_files() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");
        create_test_skill(&skill_dir);

        // Add some files to skip
        fs::write(skill_dir.join(".hidden"), "hidden").unwrap();
        fs::write(skill_dir.join("test.pyc"), "compiled").unwrap();

        let files = collect_files(&skill_dir).unwrap();

        // Should include SKILL.md and references/example.md
        assert_eq!(files.len(), 2);

        let file_names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(file_names.contains(&"SKILL.md".to_string()));
        assert!(file_names.contains(&"example.md".to_string()));
    }

    #[test]
    fn test_package_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");
        create_test_skill(&skill_dir);

        let output_dir = temp.path().join("dist");

        let result = package_skill(&skill_dir, &output_dir).unwrap();

        assert!(result.output_path.exists());
        assert_eq!(result.files_included, 2);
        assert!(result.validation.valid);

        // Verify it's a valid zip
        let contents = list_skill_contents(&result.output_path).unwrap();
        assert!(contents.iter().any(|c| c.ends_with("SKILL.md")));
        assert!(contents.iter().any(|c| c.contains("references")));
    }

    #[test]
    fn test_package_skill_correct_structure() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("my-skill");

        // Create skill with the name "my-skill"
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: my-skill
description: A test skill for testing archive structure with proper path prefixes
---

# My Skill
"#,
        )
        .unwrap();

        let output_dir = temp.path().join("dist");
        let result = package_skill(&skill_dir, &output_dir).unwrap();

        let contents = list_skill_contents(&result.output_path).unwrap();

        // All paths should start with skill name
        for entry in &contents {
            assert!(
                entry.starts_with("my-skill/"),
                "Entry {} should start with 'my-skill/'",
                entry
            );
        }
    }

    #[test]
    fn test_package_invalid_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("invalid-skill");

        // Create skill without proper frontmatter
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# No frontmatter").unwrap();

        let output_dir = temp.path().join("dist");
        let result = package_skill(&skill_dir, &output_dir);

        assert!(result.is_err());
    }

    #[test]
    fn test_list_skill_contents() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("list-test");
        create_test_skill(&skill_dir);

        let output_dir = temp.path().join("dist");
        let result = package_skill(&skill_dir, &output_dir).unwrap();

        let contents = list_skill_contents(&result.output_path).unwrap();

        assert!(!contents.is_empty());
        assert!(contents.iter().any(|c| c.contains("SKILL.md")));
    }
}
