//! Multi-source install resolution: local repo → remote repo → GitHub.

use anyhow::{Context, Result};
use std::path::Path;

use crate::config::Config;
use crate::install::{install_from_file, install_skill, InstallResult};
use crate::local_storage::LocalStorageClient;
use crate::output::Output;
use crate::repository::Repository;

/// Options controlling install source resolution.
pub struct InstallOptions<'a> {
    pub skill_name: &'a str,
    pub version: Option<&'a str>,
    pub github_repo: Option<&'a str>,
    pub install_dir: &'a Path,
    pub local_only: bool,
    pub remote_only: bool,
    pub github_only: bool,
}

/// Which source a skill was installed from.
#[derive(Debug, PartialEq, Eq)]
pub enum InstallSource {
    Local,
    Remote,
    GitHub,
}

/// Result of a resolved install.
#[derive(Debug)]
pub struct ResolvedInstall {
    pub source: InstallSource,
    pub result: InstallResult,
}

/// Resolve the install source and install the skill.
///
/// Resolution order (when no explicit source flag):
/// 1. Local repository (if configured) → install if found
/// 2. Remote S3 repository (if configured) → install if found
/// 3. GitHub releases (fallback)
///
/// Explicit flags (`--local`, `--remote`, `--github`) skip the cascade.
///
/// # Errors
///
/// Returns an error if the skill cannot be found or installed from any source.
pub fn resolve_and_install(
    config: &Config,
    options: &InstallOptions,
    output: &Output,
) -> Result<ResolvedInstall> {
    let repo_config = config.repository.as_ref();

    // Explicit source flags
    if options.local_only {
        return install_from_local(repo_config, options, output);
    }
    if options.remote_only {
        return install_from_remote(config, options, output);
    }
    if options.github_only {
        return install_from_github(options, output);
    }

    // Cascade: local → remote → GitHub
    if let Some(rc) = repo_config {
        if rc.has_local() {
            match install_from_local(repo_config, options, output) {
                Ok(result) => return Ok(result),
                Err(_) => {
                    output.info(&format!(
                        "Skill '{}' not found in local repository, trying next source...",
                        options.skill_name
                    ));
                }
            }
        }

        if rc.has_remote() {
            match install_from_remote(config, options, output) {
                Ok(result) => return Ok(result),
                Err(_) => {
                    output.info(&format!(
                        "Skill '{}' not found in remote repository, trying GitHub...",
                        options.skill_name
                    ));
                }
            }
        }
    }

    install_from_github(options, output)
}

fn install_from_local(
    repo_config: Option<&crate::config::RepositoryConfig>,
    options: &InstallOptions,
    output: &Output,
) -> Result<ResolvedInstall> {
    let rc = repo_config.context("No repository configured for local install")?;
    let local_path = rc.local_repo_path();
    let client = LocalStorageClient::with_dir(&local_path);

    // Build a Repository backed by local storage
    let repo = Repository::new(client);
    output.info("Looking in local repository...");
    let skill_path = repo
        .download(options.skill_name, options.version, None, output)
        .context("Skill not found in local repository")?;

    let result = install_from_file(&skill_path, options.install_dir, output)?;
    Ok(ResolvedInstall {
        source: InstallSource::Local,
        result,
    })
}

fn install_from_remote(
    config: &Config,
    options: &InstallOptions,
    output: &Output,
) -> Result<ResolvedInstall> {
    let rc = config
        .repository
        .as_ref()
        .context("No repository configured for remote install")?;

    if !rc.has_remote() {
        anyhow::bail!("No remote repository configured (missing bucket_name)");
    }

    let repo = Repository::from_config(rc)?;

    output.info("Looking in remote repository...");
    repo.install(
        options.skill_name,
        options.version,
        options.install_dir,
        output,
    )?;

    Ok(ResolvedInstall {
        source: InstallSource::Remote,
        result: InstallResult {
            skill_name: options.skill_name.to_string(),
            install_path: options.install_dir.join(options.skill_name),
            files_extracted: 0, // repo.install handles extraction internally
        },
    })
}

fn install_from_github(options: &InstallOptions, output: &Output) -> Result<ResolvedInstall> {
    output.info("Installing from GitHub releases...");
    let result = install_skill(
        options.skill_name,
        options.version,
        options.github_repo,
        Some(options.install_dir),
        output,
    )?;

    Ok(ResolvedInstall {
        source: InstallSource::GitHub,
        result,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LocalRepositoryConfig, RepositoryConfig};
    use crate::package::package_skill;
    use crate::repository::UploadParams;
    use tempfile::TempDir;

    fn test_output() -> Output {
        Output::new(true, false)
    }

    fn create_test_skill_in_local_repo(local_path: &Path) -> String {
        let out = test_output();
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("resolver-test");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: resolver-test
description: A test skill for resolver testing with enough characters to pass validation
---

# Resolver Test Skill
"#,
        )
        .unwrap();
        std::fs::write(skill_dir.join("references/doc.md"), "# Doc").unwrap();

        let dist = tmp.path().join("dist");
        let package_result = package_skill(&skill_dir, &dist).unwrap();

        // Upload to local repo via LocalStorageClient as a Repository
        let client = LocalStorageClient::new(local_path).unwrap();
        let repo = Repository::new(client);
        repo.upload(
            &UploadParams {
                name: "resolver-test",
                version: "1.0.0",
                description: "A test skill",
                llms_txt_url: "https://example.com/llms.txt",
                skill_file: &package_result.output_path,
                changelog: None,
                source_dir: None,
            },
            &out,
        )
        .unwrap();

        "resolver-test".to_string()
    }

    #[test]
    fn test_install_from_local_repo() {
        let out = test_output();
        let tmp = TempDir::new().unwrap();
        let local_path = tmp.path().join("local");
        let install_dir = tmp.path().join("installed");

        create_test_skill_in_local_repo(&local_path);

        let config = Config {
            skills: vec![],
            repository: Some(RepositoryConfig {
                name: None,
                local: Some(LocalRepositoryConfig {
                    path: Some(local_path.to_string_lossy().to_string()),
                    cache: false,
                }),
                bucket_name: None,
                region: "us-east-1".to_string(),
                endpoint: None,
            }),
        };

        let options = InstallOptions {
            skill_name: "resolver-test",
            version: Some("1.0.0"),
            install_dir: &install_dir,
            github_repo: None,
            local_only: true,
            remote_only: false,
            github_only: false,
        };

        let resolved = resolve_and_install(&config, &options, &out).unwrap();
        assert_eq!(resolved.source, InstallSource::Local);
        assert!(install_dir.join("resolver-test/SKILL.md").exists());
    }

    #[test]
    fn test_local_not_found_falls_through_to_github_error() {
        let out = test_output();
        let tmp = TempDir::new().unwrap();
        let local_path = tmp.path().join("local");
        std::fs::create_dir_all(&local_path).unwrap();
        let install_dir = tmp.path().join("installed");

        let config = Config {
            skills: vec![],
            repository: Some(RepositoryConfig {
                name: None,
                local: Some(LocalRepositoryConfig {
                    path: Some(local_path.to_string_lossy().to_string()),
                    cache: false,
                }),
                bucket_name: None,
                region: "us-east-1".to_string(),
                endpoint: None,
            }),
        };

        let options = InstallOptions {
            skill_name: "nonexistent-skill",
            version: Some("1.0.0"),
            install_dir: &install_dir,
            github_repo: None,
            local_only: false,
            remote_only: false,
            github_only: false,
        };

        // This will fail because GitHub won't have it either, but it should
        // cascade past local repo without panicking
        let result = resolve_and_install(&config, &options, &out);
        assert!(result.is_err());
    }

    #[test]
    fn test_local_only_fails_when_not_found() {
        let out = test_output();
        let tmp = TempDir::new().unwrap();
        let local_path = tmp.path().join("local");
        std::fs::create_dir_all(&local_path).unwrap();
        let install_dir = tmp.path().join("installed");

        let config = Config {
            skills: vec![],
            repository: Some(RepositoryConfig {
                name: None,
                local: Some(LocalRepositoryConfig {
                    path: Some(local_path.to_string_lossy().to_string()),
                    cache: false,
                }),
                bucket_name: None,
                region: "us-east-1".to_string(),
                endpoint: None,
            }),
        };

        let options = InstallOptions {
            skill_name: "nonexistent",
            version: Some("1.0.0"),
            install_dir: &install_dir,
            github_repo: None,
            local_only: true,
            remote_only: false,
            github_only: false,
        };

        let result = resolve_and_install(&config, &options, &out);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_config_falls_through_to_github() {
        let out = test_output();
        let tmp = TempDir::new().unwrap();
        let install_dir = tmp.path().join("installed");

        let config = Config::default();

        let options = InstallOptions {
            skill_name: "nonexistent-skill",
            version: Some("99.99.99"),
            install_dir: &install_dir,
            github_repo: None,
            local_only: false,
            remote_only: false,
            github_only: false,
        };

        // Should fail at GitHub (no such release), but shouldn't panic
        let result = resolve_and_install(&config, &options, &out);
        assert!(result.is_err());
    }
}
