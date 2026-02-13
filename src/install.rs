//! Install skills from GitHub releases.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use zip::ZipArchive;

/// Default repository for skill releases.
pub const DEFAULT_REPO: &str = "antstanley/skill-builder";

/// Default installation directory relative to current directory.
pub const DEFAULT_INSTALL_DIR: &str = ".claude/skills";

/// HTTP client with reasonable defaults.
fn create_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("sb/1.0")
        .build()
        .context("Failed to create HTTP client")
}

/// Get the GitHub release download URL for a skill.
pub fn get_release_url(skill_name: &str, version: Option<&str>, repo: Option<&str>) -> String {
    let repo = repo.unwrap_or(DEFAULT_REPO);

    match version {
        Some(v) => format!(
            "https://github.com/{}/releases/download/v{}/{}.skill",
            repo, v, skill_name
        ),
        None => format!(
            "https://github.com/{}/releases/latest/download/{}.skill",
            repo, skill_name
        ),
    }
}

/// Installation result.
#[derive(Debug)]
pub struct InstallResult {
    pub skill_name: String,
    pub install_path: PathBuf,
    pub files_extracted: usize,
}

/// Download and extract a skill from GitHub releases.
pub fn install_skill(
    skill_name: &str,
    version: Option<&str>,
    repo: Option<&str>,
    install_dir: Option<&Path>,
) -> Result<InstallResult> {
    let client = create_client()?;

    // Determine installation directory
    let install_dir = install_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_INSTALL_DIR));

    let url = get_release_url(skill_name, version, repo);

    println!("Installing {} skill...", skill_name);
    if let Some(v) = version {
        println!("Version: {}", v);
    } else {
        println!("Version: latest");
    }
    println!();

    // Download the skill file
    println!("Downloading from {}...", url);

    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to download {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} when downloading {}", response.status(), url);
    }

    let bytes = response.bytes().context("Failed to read response body")?;

    // Create install directory
    fs::create_dir_all(&install_dir)?;

    // Extract the skill
    println!("Extracting skill...");

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    let mut files_extracted = 0;
    let mut skill_path = install_dir.clone();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Determine output path
        let outpath = install_dir.join(&name);

        // Track the skill root directory
        if i == 0 {
            if let Some(first_component) = PathBuf::from(&name).components().next() {
                skill_path = install_dir.join(first_component.as_os_str());
            }
        }

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            // Create parent directories
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }

            // Write file
            let mut outfile = File::create(&outpath)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            outfile.write_all(&buffer)?;
            files_extracted += 1;

            println!("  Extracted: {}", name);
        }
    }

    println!();
    println!(
        "Successfully installed {} skill to {}",
        skill_name,
        skill_path.display()
    );
    println!();
    println!("The skill will be available in Claude Code for projects in this directory.");

    Ok(InstallResult {
        skill_name: skill_name.to_string(),
        install_path: skill_path,
        files_extracted,
    })
}

/// Install a skill from a local .skill file.
pub fn install_from_file<P: AsRef<Path>, Q: AsRef<Path>>(
    skill_file: P,
    install_dir: Q,
) -> Result<InstallResult> {
    let skill_file = skill_file.as_ref();
    let install_dir = install_dir.as_ref();

    println!("Installing skill from {}...", skill_file.display());

    // Read the skill file
    let file = File::open(skill_file)
        .with_context(|| format!("Failed to open {}", skill_file.display()))?;

    let mut archive = ZipArchive::new(file)?;

    // Create install directory
    fs::create_dir_all(install_dir)?;

    // Extract
    let mut files_extracted = 0;
    let mut skill_name = String::new();
    let mut skill_path = install_dir.to_path_buf();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Get skill name from first path component
        if i == 0 {
            if let Some(first) = PathBuf::from(&name).components().next() {
                skill_name = first.as_os_str().to_string_lossy().to_string();
                skill_path = install_dir.join(&skill_name);
            }
        }

        let outpath = install_dir.join(&name);

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut outfile = File::create(&outpath)?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            outfile.write_all(&buffer)?;
            files_extracted += 1;

            println!("  Extracted: {}", name);
        }
    }

    println!();
    println!(
        "Successfully installed {} skill to {}",
        skill_name,
        skill_path.display()
    );

    Ok(InstallResult {
        skill_name,
        install_path: skill_path,
        files_extracted,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::package_skill;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_get_release_url_latest() {
        let url = get_release_url("shadcn-svelte", None, None);
        assert_eq!(
            url,
            "https://github.com/antstanley/skill-builder/releases/latest/download/shadcn-svelte.skill"
        );
    }

    #[test]
    fn test_get_release_url_with_version() {
        let url = get_release_url("shadcn-svelte", Some("1.0.0"), None);
        assert_eq!(
            url,
            "https://github.com/antstanley/skill-builder/releases/download/v1.0.0/shadcn-svelte.skill"
        );
    }

    #[test]
    fn test_get_release_url_custom_repo() {
        let url = get_release_url("my-skill", Some("2.0.0"), Some("user/repo"));
        assert_eq!(
            url,
            "https://github.com/user/repo/releases/download/v2.0.0/my-skill.skill"
        );
    }

    #[test]
    fn test_install_from_file() {
        let temp = TempDir::new().unwrap();

        // Create a test skill to package
        let skill_dir = temp.path().join("test-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: test-skill
description: A test skill for installation testing with enough characters to pass validation
---

# Test Skill
"#,
        )
        .unwrap();

        fs::create_dir_all(skill_dir.join("references")).unwrap();
        fs::write(skill_dir.join("references/doc.md"), "# Doc").unwrap();

        // Package it
        let package_dir = temp.path().join("packages");
        let package_result = package_skill(&skill_dir, &package_dir).unwrap();

        // Install it
        let install_dir = temp.path().join("installed");
        let result = install_from_file(&package_result.output_path, &install_dir).unwrap();

        assert_eq!(result.skill_name, "test-skill");
        assert!(result.install_path.exists());
        assert!(result.install_path.join("SKILL.md").exists());
        assert!(result.install_path.join("references/doc.md").exists());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_REPO, "antstanley/skill-builder");
        assert_eq!(DEFAULT_INSTALL_DIR, ".claude/skills");
    }
}
