//! Repository operations orchestrating S3, cache, and index.

use anyhow::{Context, Result};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

use crate::cache::SkillCache;
use crate::index::{load_index, save_index, SkillsIndex};
use crate::install::install_from_file;
use crate::s3::S3Operations;

/// Repository managing skills in S3 with local caching.
pub struct Repository<S: S3Operations> {
    client: S,
    cache: SkillCache,
}

impl<S: S3Operations> Repository<S> {
    /// Create a new repository.
    pub fn new(client: S, cache: SkillCache) -> Self {
        Self { client, cache }
    }

    /// Upload a skill to the repository.
    #[allow(clippy::too_many_arguments)]
    pub fn upload(
        &self,
        name: &str,
        version: &str,
        description: &str,
        llms_txt_url: &str,
        skill_file: &Path,
        changelog: Option<&Path>,
        source_dir: Option<&Path>,
    ) -> Result<()> {
        let skill_data = std::fs::read(skill_file)
            .with_context(|| format!("Failed to read skill file: {}", skill_file.display()))?;

        // Upload skill file
        let skill_key = format!("skills/{}/{}/{}.skill", name, version, name);
        self.client.put_object(&skill_key, &skill_data)?;
        println!("  Uploaded: {}", skill_key);

        // Upload changelog if provided
        if let Some(changelog_path) = changelog {
            let changelog_data = std::fs::read_to_string(changelog_path).with_context(|| {
                format!("Failed to read changelog: {}", changelog_path.display())
            })?;
            let changelog_key = format!("skills/{}/{}/CHANGELOG.md", name, version);
            self.client
                .put_object(&changelog_key, changelog_data.as_bytes())?;
            println!("  Uploaded: {}", changelog_key);
        }

        // Upload source archive if provided
        if let Some(src_dir) = source_dir {
            let archive = create_source_archive(src_dir, name)?;
            let source_key = format!("source/{}/{}/{}-source.zip", name, version, name);
            self.client.put_object(&source_key, &archive)?;
            println!("  Uploaded: {}", source_key);
        }

        // Update index
        let mut index = load_index(&self.client)?;
        index.add_or_update_skill(name, description, llms_txt_url, version, &skill_key);
        save_index(&self.client, &index)?;
        println!("  Updated index");

        Ok(())
    }

    /// Download a skill, using cache when available. Returns path to the cached file.
    pub fn download(
        &self,
        name: &str,
        version: Option<&str>,
        output_dir: Option<&Path>,
    ) -> Result<PathBuf> {
        let index = load_index(&self.client)?;
        let resolved_version = match version {
            Some(v) => v.to_string(),
            None => index
                .latest_version(name)
                .context(format!("Skill '{}' not found in repository", name))?
                .to_string(),
        };

        // Check cache first
        if let Some(cached_path) = self.cache.get(name, &resolved_version) {
            println!("Using cached version: {}", cached_path.display());
            if let Some(out_dir) = output_dir {
                let dest = out_dir.join(format!("{}.skill", name));
                std::fs::create_dir_all(out_dir)?;
                std::fs::copy(&cached_path, &dest)?;
                return Ok(dest);
            }
            return Ok(cached_path);
        }

        // Find S3 path from index
        let entry = index
            .find_skill(name)
            .context(format!("Skill '{}' not found in repository", name))?;
        let s3_path = entry.versions.get(&resolved_version).with_context(|| {
            format!(
                "Version '{}' not found for skill '{}'",
                resolved_version, name
            )
        })?;

        // Download from S3
        println!("Downloading {} v{}...", name, resolved_version);
        let data = self.client.get_object(s3_path)?;

        // Cache it
        let cached_path =
            self.cache
                .store(name, &resolved_version, &data, &format!("s3://{}", s3_path))?;

        if let Some(out_dir) = output_dir {
            let dest = out_dir.join(format!("{}.skill", name));
            std::fs::create_dir_all(out_dir)?;
            std::fs::copy(&cached_path, &dest)?;
            return Ok(dest);
        }

        Ok(cached_path)
    }

    /// Download and install a skill.
    pub fn install(&self, name: &str, version: Option<&str>, install_dir: &Path) -> Result<()> {
        let skill_path = self.download(name, version, None)?;
        install_from_file(&skill_path, install_dir)?;
        Ok(())
    }

    /// Delete a skill version (or all versions) from the repository.
    pub fn delete(&self, name: &str, version: Option<&str>) -> Result<()> {
        let mut index = load_index(&self.client)?;

        if let Some(ver) = version {
            // Delete specific version
            let skill_key = format!("skills/{}/{}/{}.skill", name, ver, name);
            let changelog_key = format!("skills/{}/{}/CHANGELOG.md", name, ver);
            let source_key = format!("source/{}/{}/{}-source.zip", name, ver, name);

            self.client.delete_object(&skill_key).ok();
            self.client.delete_object(&changelog_key).ok();
            self.client.delete_object(&source_key).ok();

            index.remove_version(name, ver);
            println!("  Deleted version {} of {}", ver, name);
        } else {
            // Delete all versions
            let entry = index.find_skill(name);
            if let Some(entry) = entry {
                for ver in entry.versions.keys().cloned().collect::<Vec<_>>() {
                    let skill_key = format!("skills/{}/{}/{}.skill", name, ver, name);
                    let changelog_key = format!("skills/{}/{}/CHANGELOG.md", name, ver);
                    let source_key = format!("source/{}/{}/{}-source.zip", name, ver, name);

                    self.client.delete_object(&skill_key).ok();
                    self.client.delete_object(&changelog_key).ok();
                    self.client.delete_object(&source_key).ok();
                }
            }

            index.remove_skill(name);
            println!("  Deleted all versions of {}", name);
        }

        save_index(&self.client, &index)?;

        // Clear local cache
        if let Some(ver) = version {
            self.cache.remove(name, ver).ok();
        } else {
            self.cache.remove_all(name).ok();
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

    let prefix = format!("{}-source", name);
    add_dir_to_zip(&mut zip, &base, &base, &prefix, options)?;

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::s3::mock::MockS3Client;
    use tempfile::TempDir;

    fn setup() -> (Repository<MockS3Client>, TempDir) {
        let tmp = TempDir::new().unwrap();
        let cache = SkillCache::with_dir(tmp.path().join("cache"));
        let client = MockS3Client::new();
        let repo = Repository::new(client, cache);
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

    #[test]
    fn test_upload_and_list() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "A test skill",
            "https://example.com/llms.txt",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        let index = repo.list(None).unwrap();
        assert_eq!(index.skills.len(), 1);
        assert_eq!(index.skills[0].name, "test-skill");
        assert_eq!(index.skills[0].versions.len(), 1);
    }

    #[test]
    fn test_upload_and_download() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "A test skill",
            "https://example.com/llms.txt",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        let downloaded = repo.download("test-skill", Some("1.0.0"), None).unwrap();
        assert!(downloaded.exists());
    }

    #[test]
    fn test_download_uses_cache() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "A test",
            "https://example.com/llms.txt",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        // First download should cache
        let path1 = repo.download("test-skill", Some("1.0.0"), None).unwrap();
        // Second download should use cache
        let path2 = repo.download("test-skill", Some("1.0.0"), None).unwrap();
        assert_eq!(path1, path2);
    }

    #[test]
    fn test_download_to_output_dir() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "A test",
            "https://example.com/llms.txt",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        let output_dir = tmp.path().join("output");
        let path = repo
            .download("test-skill", Some("1.0.0"), Some(&output_dir))
            .unwrap();
        assert!(path.starts_with(&output_dir));
        assert!(path.exists());
    }

    #[test]
    fn test_delete_version() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            None,
        )
        .unwrap();
        repo.upload(
            "test-skill",
            "2.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        repo.delete("test-skill", Some("1.0.0")).unwrap();

        let index = repo.list(None).unwrap();
        let entry = index.find_skill("test-skill").unwrap();
        assert_eq!(entry.versions.len(), 1);
        assert!(entry.versions.contains_key("2.0.0"));
    }

    #[test]
    fn test_delete_all_versions() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        repo.delete("test-skill", None).unwrap();

        let index = repo.list(None).unwrap();
        assert!(index.skills.is_empty());
    }

    #[test]
    fn test_list_with_filter() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload("skill-a", "1.0.0", "a", "url", &skill_file, None, None)
            .unwrap();
        repo.upload("skill-b", "1.0.0", "b", "url", &skill_file, None, None)
            .unwrap();

        let filtered = repo.list(Some("skill-a")).unwrap();
        assert_eq!(filtered.skills.len(), 1);
        assert_eq!(filtered.skills[0].name, "skill-a");
    }

    #[test]
    fn test_download_latest_version() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        repo.upload(
            "test-skill",
            "1.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            None,
        )
        .unwrap();
        repo.upload(
            "test-skill",
            "2.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            None,
        )
        .unwrap();

        // Download without specifying version should get latest
        let path = repo.download("test-skill", None, None).unwrap();
        assert!(path.exists());
        // Should have cached as 2.0.0
        assert!(path.to_string_lossy().contains("2.0.0"));
    }

    #[test]
    fn test_upload_with_changelog() {
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let changelog = tmp.path().join("CHANGELOG.md");
        std::fs::write(&changelog, "# Changelog\n\n## 1.0.0\n- Initial release").unwrap();

        repo.upload(
            "test-skill",
            "1.0.0",
            "desc",
            "url",
            &skill_file,
            Some(&changelog),
            None,
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
        let (repo, tmp) = setup();
        let skill_file = create_test_skill(tmp.path());

        let source_dir = tmp.path().join("source");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(source_dir.join("main.rs"), "fn main() {}").unwrap();

        repo.upload(
            "test-skill",
            "1.0.0",
            "desc",
            "url",
            &skill_file,
            None,
            Some(&source_dir),
        )
        .unwrap();

        // Verify source archive was uploaded
        assert!(repo
            .client
            .object_exists("source/test-skill/1.0.0/test-skill-source.zip")
            .unwrap());
    }
}
