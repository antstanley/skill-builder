//! Skills index management for S3 repository.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage::StorageOperations;

const INDEX_KEY: &str = "skills_index.json";

/// A single skill entry in the index.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexEntry {
    /// Skill name.
    pub name: String,

    /// Description of the skill.
    pub description: String,

    /// URL to the llms.txt source.
    pub llms_txt_url: String,

    /// Map of version -> S3 location path.
    pub versions: HashMap<String, String>,
}

/// The top-level skills index stored in S3.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillsIndex {
    /// All indexed skills.
    pub skills: Vec<IndexEntry>,
}

impl SkillsIndex {
    /// Create an empty index.
    #[must_use] 
    pub const fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// Find a skill by name.
    #[must_use] 
    pub fn find_skill(&self, name: &str) -> Option<&IndexEntry> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Find a mutable skill by name.
    pub fn find_skill_mut(&mut self, name: &str) -> Option<&mut IndexEntry> {
        self.skills.iter_mut().find(|s| s.name == name)
    }

    /// Add or update a skill entry. Returns true if it was an update.
    pub fn add_or_update_skill(
        &mut self,
        name: &str,
        description: &str,
        llms_txt_url: &str,
        version: &str,
        s3_path: &str,
    ) -> bool {
        if let Some(entry) = self.find_skill_mut(name) {
            entry.description = description.to_string();
            entry.llms_txt_url = llms_txt_url.to_string();
            entry
                .versions
                .insert(version.to_string(), s3_path.to_string());
            true
        } else {
            let mut versions = HashMap::new();
            versions.insert(version.to_string(), s3_path.to_string());
            self.skills.push(IndexEntry {
                name: name.to_string(),
                description: description.to_string(),
                llms_txt_url: llms_txt_url.to_string(),
                versions,
            });
            false
        }
    }

    /// Remove a skill entirely. Returns true if it existed.
    pub fn remove_skill(&mut self, name: &str) -> bool {
        let len_before = self.skills.len();
        self.skills.retain(|s| s.name != name);
        self.skills.len() < len_before
    }

    /// Remove a specific version of a skill. Returns true if it existed.
    /// Also removes the skill entry if no versions remain.
    pub fn remove_version(&mut self, name: &str, version: &str) -> bool {
        if let Some(entry) = self.find_skill_mut(name) {
            let existed = entry.versions.remove(version).is_some();
            if entry.versions.is_empty() {
                self.remove_skill(name);
            }
            existed
        } else {
            false
        }
    }

    /// Get the latest version of a skill using semantic version comparison.
    #[must_use] 
    pub fn latest_version(&self, name: &str) -> Option<&str> {
        self.find_skill(name).and_then(|entry| {
            entry
                .versions
                .keys()
                .max_by(|a, b| compare_semver(a, b))
                .map(std::string::String::as_str)
        })
    }
}

impl Default for SkillsIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple semantic version comparison (major.minor.patch).
fn compare_semver(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };

    let va = parse(a);
    let vb = parse(b);

    for i in 0..3 {
        let pa = va.get(i).copied().unwrap_or(0);
        let pb = vb.get(i).copied().unwrap_or(0);
        match pa.cmp(&pb) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

/// Load the skills index from S3. Returns an empty index if not found.
///
/// # Errors
///
/// Returns an error if the index exists but cannot be parsed.
pub fn load_index<S: StorageOperations>(client: &S) -> Result<SkillsIndex> {
    match client.get_object(INDEX_KEY) {
        Ok(data) => {
            let json = String::from_utf8(data).context("Skills index is not valid UTF-8")?;
            serde_json::from_str(&json).context("Failed to parse skills index")
        }
        Err(_) => Ok(SkillsIndex::new()),
    }
}

/// Save the skills index to S3.
///
/// # Errors
///
/// Returns an error if the index cannot be serialized or written to storage.
pub fn save_index<S: StorageOperations>(client: &S, index: &SkillsIndex) -> Result<()> {
    let json = serde_json::to_string_pretty(index).context("Failed to serialize skills index")?;
    client.put_object(INDEX_KEY, json.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_index_is_empty() {
        let index = SkillsIndex::new();
        assert!(index.skills.is_empty());
    }

    #[test]
    fn test_add_skill() {
        let mut index = SkillsIndex::new();
        let updated = index.add_or_update_skill(
            "test-skill",
            "A test skill",
            "https://example.com/llms.txt",
            "1.0.0",
            "skills/test-skill/1.0.0/test-skill.skill",
        );
        assert!(!updated);
        assert_eq!(index.skills.len(), 1);
        assert_eq!(index.find_skill("test-skill").unwrap().name, "test-skill");
    }

    #[test]
    fn test_update_skill() {
        let mut index = SkillsIndex::new();
        index.add_or_update_skill(
            "test-skill",
            "v1",
            "https://example.com/llms.txt",
            "1.0.0",
            "skills/test-skill/1.0.0/test-skill.skill",
        );
        let updated = index.add_or_update_skill(
            "test-skill",
            "v2",
            "https://example.com/llms.txt",
            "2.0.0",
            "skills/test-skill/2.0.0/test-skill.skill",
        );
        assert!(updated);
        assert_eq!(index.skills.len(), 1);
        assert_eq!(index.find_skill("test-skill").unwrap().versions.len(), 2);
    }

    #[test]
    fn test_remove_skill() {
        let mut index = SkillsIndex::new();
        index.add_or_update_skill("a", "desc", "url", "1.0.0", "path");
        index.add_or_update_skill("b", "desc", "url", "1.0.0", "path");

        assert!(index.remove_skill("a"));
        assert_eq!(index.skills.len(), 1);
        assert!(!index.remove_skill("nonexistent"));
    }

    #[test]
    fn test_remove_version() {
        let mut index = SkillsIndex::new();
        index.add_or_update_skill("a", "desc", "url", "1.0.0", "path1");
        index.add_or_update_skill("a", "desc", "url", "2.0.0", "path2");

        assert!(index.remove_version("a", "1.0.0"));
        assert_eq!(index.find_skill("a").unwrap().versions.len(), 1);

        // Removing last version should remove the skill entirely
        assert!(index.remove_version("a", "2.0.0"));
        assert!(index.find_skill("a").is_none());
    }

    #[test]
    fn test_latest_version() {
        let mut index = SkillsIndex::new();
        index.add_or_update_skill("a", "d", "u", "1.0.0", "p");
        index.add_or_update_skill("a", "d", "u", "2.1.0", "p");
        index.add_or_update_skill("a", "d", "u", "1.5.0", "p");

        assert_eq!(index.latest_version("a"), Some("2.1.0"));
    }

    #[test]
    fn test_latest_version_nonexistent() {
        let index = SkillsIndex::new();
        assert!(index.latest_version("nope").is_none());
    }

    #[test]
    fn test_compare_semver() {
        assert_eq!(compare_semver("1.0.0", "1.0.0"), std::cmp::Ordering::Equal);
        assert_eq!(
            compare_semver("2.0.0", "1.0.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_semver("1.0.0", "2.0.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_semver("1.2.0", "1.1.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_semver("1.0.1", "1.0.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_index_serialization_roundtrip() {
        let mut index = SkillsIndex::new();
        index.add_or_update_skill(
            "test",
            "A test skill",
            "https://example.com/llms.txt",
            "1.0.0",
            "skills/test/1.0.0/test.skill",
        );

        let json = serde_json::to_string(&index).unwrap();
        let deserialized: SkillsIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(index, deserialized);
    }

    #[test]
    fn test_load_save_index_with_mock() {
        use crate::s3::mock::MockS3Client;

        let client = MockS3Client::new();

        // Loading from empty should give empty index
        let index = load_index(&client).unwrap();
        assert!(index.skills.is_empty());

        // Save and reload
        let mut index = SkillsIndex::new();
        index.add_or_update_skill("s", "d", "u", "1.0.0", "p");
        save_index(&client, &index).unwrap();

        let loaded = load_index(&client).unwrap();
        assert_eq!(loaded, index);
    }
}
