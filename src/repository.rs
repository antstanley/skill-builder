//! Repository operations orchestrating S3, local storage, and index.

use anyhow::{Context, Result};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use crate::config::RepositoryConfig;
use crate::index::{load_index, save_index, SkillsIndex};
use crate::install::install_from_file;
use crate::local_storage::LocalStorageClient;
use crate::output::Output;
use crate::s3::S3Client;
use crate::storage::StorageOperations;

/// Parameters for uploading a skill to the repository.
pub struct UploadParams<'a> {
    pub name: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    pub llms_txt_url: &'a str,
    pub skill_file: &'a Path,
    pub changelog: Option<&'a Path>,
    pub source_dir: Option<&'a Path>,
}

/// Repository managing skills in S3 with optional local cache.
pub struct Repository<S: StorageOperations> {
    client: S,
    local_cache: Option<LocalStorageClient>,
}

impl<S: StorageOperations> Repository<S> {
    /// Create a new repository without a local cache.
    pub const fn new(client: S) -> Self {
        Self {
            client,
            local_cache: None,
        }
    }

    /// Create a new repository with a local cache layer.
    pub const fn new_with_cache(client: S, local_cache: LocalStorageClient) -> Self {
        Self {
            client,
            local_cache: Some(local_cache),
        }
    }
}

impl Repository<S3Client> {
    /// Create a repository from config, with optional local cache.
    pub fn from_config(repo_config: &RepositoryConfig) -> Result<Self> {
        let client = S3Client::new(repo_config)?;
        if repo_config.local_is_cache() {
            let local_path = repo_config.local_repo_path();
            let local_cache = LocalStorageClient::new(&local_path)?;
            Ok(Self::new_with_cache(client, local_cache))
        } else {
            Ok(Self::new(client))
        }
    }
}

impl<S: StorageOperations> Repository<S> {
    /// Upload a skill to the repository.
    pub fn upload(&self, params: &UploadParams, output: &Output) -> Result<()> {
        let skill_data = std::fs::read(params.skill_file).with_context(|| {
            format!("Failed to read skill file: {}", params.skill_file.display())
        })?;

        // Upload skill file
        let skill_key = format!(
            "skills/{}/{}/{}.skill",
            params.name, params.version, params.name
        );
        let pb = output.spinner(&format!("Uploading {skill_key}"));
        self.client.put_object(&skill_key, &skill_data)?;
        pb.finish_and_clear();
        output.step(&format!("Uploaded: {skill_key}"));

        // Upload changelog if provided
        if let Some(changelog_path) = params.changelog {
            let changelog_data = std::fs::read_to_string(changelog_path).with_context(|| {
                format!("Failed to read changelog: {}", changelog_path.display())
            })?;
            let changelog_key = format!("skills/{}/{}/CHANGELOG.md", params.name, params.version);
            self.client
                .put_object(&changelog_key, changelog_data.as_bytes())?;
            output.step(&format!("Uploaded: {changelog_key}"));
        }

        // Upload source archive if provided
        if let Some(src_dir) = params.source_dir {
            let archive = create_source_archive(src_dir, params.name)?;
            let source_key = format!(
                "source/{}/{}/{}-source.zip",
                params.name, params.version, params.name
            );
            self.client.put_object(&source_key, &archive)?;
            output.step(&format!("Uploaded: {source_key}"));
        }

        // Update index
        let mut index = load_index(&self.client)?;
        index.add_or_update_skill(
            params.name,
            params.description,
            params.llms_txt_url,
            params.version,
            &skill_key,
        );
        save_index(&self.client, &index)?;
        output.step("Updated index");

        Ok(())
    }

    /// Download a skill, using local cache when available. Returns path to the file.
    pub fn download(
        &self,
        name: &str,
        version: Option<&str>,
        output_dir: Option<&Path>,
        output: &Output,
    ) -> Result<PathBuf> {
        let index = load_index(&self.client)?;
        let resolved_version = match version {
            Some(v) => v.to_string(),
            None => index
                .latest_version(name)
                .context(format!("Skill '{name}' not found in repository"))?
                .to_string(),
        };

        // Check local cache first
        if let Some(ref cache) = self.local_cache {
            let cache_key = format!("skills/{name}/{resolved_version}/{name}.skill");
            if cache.object_exists(&cache_key).unwrap_or(false) {
                output.info(&format!(
                    "Using cached version: {name} v{resolved_version}"
                ));
                let data = cache.get_object(&cache_key)?;
                return write_output(name, &data, output_dir);
            }
        }

        // Find S3 path from index
        let entry = index
            .find_skill(name)
            .context(format!("Skill '{name}' not found in repository"))?;
        let s3_path = entry.versions.get(&resolved_version).with_context(|| {
            format!(
                "Version '{resolved_version}' not found for skill '{name}'"
            )
        })?;

        // Download from primary storage
        let pb = output.spinner(&format!("Downloading {name} v{resolved_version}"));
        let data = self.client.get_object(s3_path)?;
        pb.finish_and_clear();

        // Store in local cache
        if let Some(ref cache) = self.local_cache {
            let cache_key = format!("skills/{name}/{resolved_version}/{name}.skill");
            cache.put_object(&cache_key, &data).ok();
        }

        write_output(name, &data, output_dir)
    }

    /// Download and install a skill.
    pub fn install(
        &self,
        name: &str,
        version: Option<&str>,
        install_dir: &Path,
        output: &Output,
    ) -> Result<()> {
        let skill_path = self.download(name, version, None, output)?;
        install_from_file(&skill_path, install_dir, output)?;
        Ok(())
    }

    /// Delete a skill version (or all versions) from the repository.
    pub fn delete(&self, name: &str, version: Option<&str>, output: &Output) -> Result<()> {
        let mut index = load_index(&self.client)?;

        let delete_version_keys = |client: &S, n: &str, v: &str, out: &Output| {
            let keys = [
                format!("skills/{n}/{v}/{n}.skill"),
                format!("skills/{n}/{v}/CHANGELOG.md"),
                format!("source/{n}/{v}/{n}-source.zip"),
            ];
            for key in &keys {
                if let Err(e) = client.delete_object(key) {
                    out.warn(&format!("Failed to delete {key}: {e}"));
                }
            }
        };

        if let Some(ver) = version {
            delete_version_keys(&self.client, name, ver, output);
            index.remove_version(name, ver);
            output.step(&format!("Deleted version {ver} of {name}"));
        } else {
            let entry = index.find_skill(name);
            if let Some(entry) = entry {
                for ver in entry.versions.keys() {
                    delete_version_keys(&self.client, name, ver, output);
                }
            }
            index.remove_skill(name);
            output.step(&format!("Deleted all versions of {name}"));
        }

        save_index(&self.client, &index)?;

        // Clear local cache
        if let Some(ref cache) = self.local_cache {
            if let Some(ver) = version {
                let cache_key = format!("skills/{name}/{ver}/{name}.skill");
                cache.delete_object(&cache_key).ok();
            } else {
                let prefix = format!("skills/{name}/");
                if let Ok(keys) = cache.list_objects(&prefix) {
                    for key in keys {
                        cache.delete_object(&key).ok();
                    }
                }
            }
        }

        Ok(())
    }

    /// List all skills in the repository.
    pub fn list(&self, skill_filter: Option<&str>) -> Result<SkillsIndex> {
        let index = load_index(&self.client)?;

        if let Some(name) = skill_filter {
            let mut filtered = SkillsIndex::new();
            if let Some(entry) = index.find_skill(name) {
                filtered.skills.push(entry.clone());
            }
            Ok(filtered)
        } else {
            Ok(index)
        }
    }
}

/// Write skill data to output directory or a temp file.
fn write_output(name: &str, data: &[u8], output_dir: Option<&Path>) -> Result<PathBuf> {
    let dir = match output_dir {
        Some(d) => d.to_path_buf(),
        None => std::env::temp_dir().join("skill-builder"),
    };
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{name}.skill"));
    std::fs::write(&dest, data)?;
    Ok(dest)
}

/// Create a zip archive of a source directory.
fn create_source_archive(source_dir: &Path, name: &str) -> Result<Vec<u8>> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(buffer);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let base = source_dir.to_path_buf();

    fn add_dir_to_zip(
        zip: &mut zip::ZipWriter<Cursor<Vec<u8>>>,
        dir: &Path,
        base: &Path,
        prefix: &str,
        options: zip::write::SimpleFileOptions,
    ) -> Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.strip_prefix(base)?;
            let archive_name = format!("{}/{}", prefix, name.to_string_lossy());

            if path.is_dir() {
                zip.add_directory(&archive_name, options)?;
                add_dir_to_zip(zip, &path, base, prefix, options)?;
            } else {
                zip.start_file(&archive_name, options)?;
                let mut file = std::fs::File::open(&path)?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                zip.write_all(&buf)?;
            }
        }
        Ok(())
    }

    let prefix = format!("{name}-source");
    add_dir_to_zip(&mut zip, &base, &base, &prefix, options)?;

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3::mock::MockS3Client;
    use tempfile::TempDir;

    fn test_output() -> Output {
        Output::new(true) // Use agent mode in tests to avoid terminal issues
    }

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

    fn create_test_skill(dir: &Path) -> PathBuf {
        let skill_dir = dir.join("test-skill");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            r#"---
name: test-skill
description: A test skill for repository testing with enough characters to pass validation
---

# Test Skill
"#,
        )
        .unwrap();
        std::fs::write(skill_dir.join("references/doc.md"), "# Doc").unwrap();

        // Package it
        let dist = dir.join("dist");
        crate::package::package_skill(&skill_dir, &dist).unwrap();
        dist.join("test-skill.skill")
    }

    fn upload_params<'a>(
        name: &'a str,
        version: &'a str,
        skill_file: &'a Path,
    ) -> UploadParams<'a> {
        UploadParams {
            name,
            version,
            description: "desc",
            llms_txt_url: "https://example.com/llms.txt",
            skill_file,
            changelog: None,
            source_dir: None,
        }
    }

    #[test]
    fn test_upload_and_list() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let mut params = upload_params("test-skill", "1.0.0", &skill_file);
        params.description = "A test skill";
        repo.upload(&params, &out).unwrap();

        let index = repo.list(None).unwrap();
        assert_eq!(index.skills.len(), 1);
        assert_eq!(index.skills[0].name, "test-skill");
        assert_eq!(index.skills[0].versions.len(), 1);
    }

    #[test]
    fn test_upload_and_download() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let mut params = upload_params("test-skill", "1.0.0", &skill_file);
        params.description = "A test skill";
        repo.upload(&params, &out).unwrap();

        let downloaded = repo
            .download("test-skill", Some("1.0.0"), None, &out)
            .unwrap();
        assert!(downloaded.exists());
    }

    #[test]
    fn test_download_uses_cache() {
        let out = test_output();
        let (repo, tmp) = setup_with_cache();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(&upload_params("test-skill", "1.0.0", &skill_file), &out)
            .unwrap();

        // First download should cache
        let path1 = repo
            .download("test-skill", Some("1.0.0"), None, &out)
            .unwrap();
        // Second download should use cache
        let path2 = repo
            .download("test-skill", Some("1.0.0"), None, &out)
            .unwrap();
        assert!(path1.exists());
        assert!(path2.exists());
    }

    #[test]
    fn test_download_to_output_dir() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(&upload_params("test-skill", "1.0.0", &skill_file), &out)
            .unwrap();

        let output_dir = tmp.path().join("output");
        let path = repo
            .download("test-skill", Some("1.0.0"), Some(&output_dir), &out)
            .unwrap();
        assert!(path.starts_with(&output_dir));
        assert!(path.exists());
    }

    #[test]
    fn test_delete_version() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(&upload_params("test-skill", "1.0.0", &skill_file), &out)
            .unwrap();
        repo.upload(&upload_params("test-skill", "2.0.0", &skill_file), &out)
            .unwrap();

        repo.delete("test-skill", Some("1.0.0"), &out).unwrap();

        let index = repo.list(None).unwrap();
        let entry = index.find_skill("test-skill").unwrap();
        assert_eq!(entry.versions.len(), 1);
        assert!(entry.versions.contains_key("2.0.0"));
    }

    #[test]
    fn test_delete_all_versions() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(&upload_params("test-skill", "1.0.0", &skill_file), &out)
            .unwrap();

        repo.delete("test-skill", None, &out).unwrap();

        let index = repo.list(None).unwrap();
        assert!(index.skills.is_empty());
    }

    #[test]
    fn test_list_with_filter() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            &UploadParams {
                name: "skill-a",
                description: "a",
                llms_txt_url: "url",
                ..upload_params("skill-a", "1.0.0", &skill_file)
            },
            &out,
        )
        .unwrap();
        repo.upload(
            &UploadParams {
                name: "skill-b",
                description: "b",
                llms_txt_url: "url",
                ..upload_params("skill-b", "1.0.0", &skill_file)
            },
            &out,
        )
        .unwrap();

        let filtered = repo.list(Some("skill-a")).unwrap();
        assert_eq!(filtered.skills.len(), 1);
        assert_eq!(filtered.skills[0].name, "skill-a");
    }

    #[test]
    fn test_download_latest_version() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(&upload_params("test-skill", "1.0.0", &skill_file), &out)
            .unwrap();
        repo.upload(&upload_params("test-skill", "2.0.0", &skill_file), &out)
            .unwrap();

        // Download without specifying version should get latest
        let path = repo.download("test-skill", None, None, &out).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_upload_with_changelog() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let changelog = tmp.path().join("CHANGELOG.md");
        std::fs::write(&changelog, "# Changelog\n\n## 1.0.0\n- Initial release").unwrap();

        repo.upload(
            &UploadParams {
                changelog: Some(&changelog),
                ..upload_params("test-skill", "1.0.0", &skill_file)
            },
            &out,
        )
        .unwrap();

        // Verify changelog was uploaded
        let data = repo
            .client
            .get_object("skills/test-skill/1.0.0/CHANGELOG.md")
            .unwrap();
        assert!(String::from_utf8(data).unwrap().contains("Initial release"));
    }

    #[test]
    fn test_upload_with_source() {
        let out = test_output();
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("main.rs"), "fn main() {}").unwrap();

        repo.upload(
            &UploadParams {
                source_dir: Some(&source_dir),
                ..upload_params("test-skill", "1.0.0", &skill_file)
            },
            &out,
        )
        .unwrap();

        // Verify source archive was uploaded
        assert!(repo
            .client
            .object_exists("source/test-skill/1.0.0/test-skill-source.zip")
            .unwrap());
    }
}
