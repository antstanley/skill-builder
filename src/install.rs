//! Install skills from GitHub releases.

use anyhow::{Context, Result};
use reqwest::blocking::Client;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use zip::ZipArchive;

use crate::output::Output;

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
#[must_use] 
pub fn get_release_url(skill_name: &str, version: Option<&str>, repo: Option<&str>) -> String {
    let repo = repo.unwrap_or(DEFAULT_REPO);

    match version {
        Some(v) => format!(
            "https://github.com/{repo}/releases/download/v{v}/{skill_name}.skill"
        ),
        None => format!(
            "https://github.com/{repo}/releases/latest/download/{skill_name}.skill"
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

/// Extract a zip archive into `install_dir`, returning (`skill_name`, `skill_path`, `files_extracted`).
fn extract_archive<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    install_dir: &Path,
) -> Result<(String, PathBuf, usize)> {
    fs::create_dir_all(install_dir)?;

    let mut files_extracted = 0;
    let mut skill_name = String::new();
    let mut skill_path = install_dir.to_path_buf();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

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
        }
    }

    Ok((skill_name, skill_path, files_extracted))
}

/// Download and extract a skill from GitHub releases.
pub fn install_skill(
    skill_name: &str,
    version: Option<&str>,
    repo: Option<&str>,
    install_dir: Option<&Path>,
    output: &Output,
) -> Result<InstallResult> {
    let client = create_client()?;

    let install_dir = install_dir.map_or_else(|| PathBuf::from(DEFAULT_INSTALL_DIR), std::path::Path::to_path_buf);

    let url = get_release_url(skill_name, version, repo);

    output.header(&format!("Installing {skill_name} skill..."));
    let ver_str = version.unwrap_or("latest");
    output.step(&format!("Version: {ver_str}"));
    output.newline();

    let pb = output.spinner(&format!("Downloading from {url}"));

    let response = client
        .get(&url)
        .send()
        .with_context(|| format!("Failed to download {url}"))?;

    if !response.status().is_success() {
        pb.finish_and_clear();
        anyhow::bail!("HTTP {} when downloading {}", response.status(), url);
    }

    let bytes = response.bytes().context("Failed to read response body")?;
    pb.finish_and_clear();

    let pb = output.spinner("Extracting skill");
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;
    let (_, skill_path, files_extracted) = extract_archive(&mut archive, &install_dir)?;
    pb.finish_and_clear();

    output.status(
        "Installed",
        &format!("{} to {}", skill_name, skill_path.display()),
    );

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
    output: &Output,
) -> Result<InstallResult> {
    let skill_file = skill_file.as_ref();
    let install_dir = install_dir.as_ref();

    let pb = output.spinner(&format!("Installing from {}", skill_file.display()));

    let file = File::open(skill_file)
        .with_context(|| format!("Failed to open {}", skill_file.display()))?;
    let mut archive = ZipArchive::new(file)?;
    let (skill_name, skill_path, files_extracted) = extract_archive(&mut archive, install_dir)?;
    pb.finish_and_clear();

    output.status(
        "Installed",
        &format!("{} to {}", skill_name, skill_path.display()),
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

    fn test_output() -> Output {
        Output::new(true)
    }

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
        let out = test_output();
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
        let result = install_from_file(&package_result.output_path, &install_dir, &out).unwrap();

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
