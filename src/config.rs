//! Configuration file parsing for skills.json.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// A skill configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillConfig {
    /// Unique name for the skill.
    pub name: String,

    /// Description of what the skill provides.
    #[serde(default)]
    pub description: String,

    /// URL to the llms.txt file.
    pub llms_txt_url: String,

    /// Base URL for resolving relative paths. Auto-derived from `llms_txt_url` if not set.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Path prefix to strip from URLs when creating local paths. Auto-detected if not set.
    #[serde(default)]
    pub path_prefix: Option<String>,
}

impl SkillConfig {
    /// Get the base URL, deriving it from `llms_txt_url` if not explicitly set.
    pub fn get_base_url(&self) -> Result<String> {
        if let Some(ref base) = self.base_url {
            return Ok(base.clone());
        }

        // Derive from llms_txt_url
        let url = url::Url::parse(&self.llms_txt_url).context("Failed to parse llms_txt_url")?;

        Ok(format!(
            "{}://{}",
            url.scheme(),
            url.host_str().unwrap_or("")
        ))
    }
}

/// Local repository configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalRepositoryConfig {
    /// Path to the local repository directory. Defaults to $HOME/.skill-builder/local/.
    #[serde(default)]
    pub path: Option<String>,

    /// Whether to use this as a cache for the remote repository.
    #[serde(default)]
    pub cache: bool,
}

/// Repository configuration for S3-compatible skill storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepositoryConfig {
    /// Display name for the repository.
    #[serde(default)]
    pub name: Option<String>,

    /// Local repository configuration.
    #[serde(default)]
    pub local: Option<LocalRepositoryConfig>,

    /// S3 bucket name.
    #[serde(default)]
    pub bucket_name: Option<String>,

    /// AWS region (defaults to "us-east-1").
    #[serde(default = "default_region")]
    pub region: String,

    /// Custom endpoint URL for S3-compatible providers.
    #[serde(default)]
    pub endpoint: Option<String>,
}

impl RepositoryConfig {
    /// Whether a remote S3 repository is configured.
    #[must_use] 
    pub const fn has_remote(&self) -> bool {
        self.bucket_name.is_some()
    }

    /// Whether a local repository is configured.
    #[must_use] 
    pub const fn has_local(&self) -> bool {
        self.local.is_some()
    }

    /// Get the resolved local repository path (configured or default).
    #[must_use] 
    pub fn local_repo_path(&self) -> PathBuf {
        if let Some(ref local) = self.local {
            if let Some(ref path) = local.path {
                return PathBuf::from(path);
            }
        }
        default_local_repo_path()
    }

    /// Whether local repo acts as a cache for remote.
    #[must_use] 
    pub fn local_is_cache(&self) -> bool {
        self.local
            .as_ref()
            .is_some_and(|l| l.cache && self.has_remote())
    }
}

fn default_region() -> String {
    "us-east-1".to_string()
}

/// Default path for the local skill repository.
#[must_use] 
pub fn default_local_repo_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skill-builder")
        .join("local")
}

/// Path to the global config directory.
#[must_use] 
pub fn global_config_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".skill-builder")
}

/// Path to the global config file.
#[must_use] 
pub fn global_config_path() -> PathBuf {
    global_config_dir().join("skills.config.json")
}

/// Root configuration structure containing all skills.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// List of skill configurations.
    #[serde(default)]
    pub skills: Vec<SkillConfig>,

    /// Optional repository configuration for S3-compatible storage.
    #[serde(default)]
    pub repository: Option<RepositoryConfig>,
}

impl Config {
    /// Load configuration from a file path.
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        Self::parse(&content)
    }

    /// Parse configuration from a JSON string.
    pub fn parse(content: &str) -> Result<Self> {
        serde_json::from_str(content).context("Failed to parse config JSON")
    }

    /// Find a skill by name.
    #[must_use] 
    pub fn find_skill(&self, name: &str) -> Option<&SkillConfig> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Get all skill names.
    #[must_use] 
    pub fn skill_names(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.name.as_str()).collect()
    }

    /// Merge another config into this one. Skills merge by name (other wins),
    /// repository replaces entirely if present in other.
    pub fn merge(&mut self, other: &Self) {
        // Merge skills by name - other's skills take priority
        for other_skill in &other.skills {
            if let Some(pos) = self.skills.iter().position(|s| s.name == other_skill.name) {
                self.skills[pos] = other_skill.clone();
            } else {
                self.skills.push(other_skill.clone());
            }
        }

        // Repository: other replaces entirely if present
        if other.repository.is_some() {
            self.repository = other.repository.clone();
        }
    }

    /// Load config with fallback hierarchy:
    /// CLI --config flag → Project skills.json (if exists) → Global config (if exists) → Built-in defaults
    pub fn load_with_fallback(config_path: Option<&Path>) -> Result<Self> {
        // If explicit config path provided, load it directly
        if let Some(path) = config_path {
            return Self::load(path);
        }

        // Try project-local skills.json
        let project_config = Path::new("skills.json");
        if project_config.exists() {
            return Self::load(project_config);
        }

        // Try global config
        let global = global_config_path();
        if global.exists() {
            return Self::load(&global);
        }

        // Return defaults
        Ok(Self::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_config() {
        let json = r#"{
            "skills": [
                {
                    "name": "test-skill",
                    "description": "A test skill",
                    "llms_txt_url": "https://example.com/llms.txt"
                }
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        assert_eq!(config.skills.len(), 1);
        assert_eq!(config.skills[0].name, "test-skill");
        assert_eq!(config.skills[0].description, "A test skill");
        assert_eq!(
            config.skills[0].llms_txt_url,
            "https://example.com/llms.txt"
        );
    }

    #[test]
    fn test_parse_empty_skills_array() {
        let json = r#"{"skills": []}"#;
        let config = Config::parse(json).unwrap();
        assert!(config.skills.is_empty());
    }

    #[test]
    fn test_parse_missing_optional_fields() {
        let json = r#"{
            "skills": [
                {
                    "name": "minimal-skill",
                    "llms_txt_url": "https://example.com/llms.txt"
                }
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        assert_eq!(config.skills[0].description, "");
        assert!(config.skills[0].base_url.is_none());
        assert!(config.skills[0].path_prefix.is_none());
    }

    #[test]
    fn test_parse_all_optional_fields() {
        let json = r#"{
            "skills": [
                {
                    "name": "full-skill",
                    "description": "Full description",
                    "llms_txt_url": "https://example.com/llms.txt",
                    "base_url": "https://custom.example.com",
                    "path_prefix": "/docs"
                }
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        let skill = &config.skills[0];
        assert_eq!(
            skill.base_url.as_deref(),
            Some("https://custom.example.com")
        );
        assert_eq!(skill.path_prefix.as_deref(), Some("/docs"));
    }

    #[test]
    fn test_error_on_missing_required_name() {
        let json = r#"{
            "skills": [
                {
                    "llms_txt_url": "https://example.com/llms.txt"
                }
            ]
        }"#;

        let result = Config::parse(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_on_missing_required_llms_txt_url() {
        let json = r#"{
            "skills": [
                {
                    "name": "test-skill"
                }
            ]
        }"#;

        let result = Config::parse(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_on_invalid_json() {
        let json = r#"{ invalid json }"#;
        let result = Config::parse(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_find_skill_by_name() {
        let json = r#"{
            "skills": [
                {"name": "first", "llms_txt_url": "https://first.com/llms.txt"},
                {"name": "second", "llms_txt_url": "https://second.com/llms.txt"}
            ]
        }"#;

        let config = Config::parse(json).unwrap();

        let found = config.find_skill("second");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "second");

        let not_found = config.find_skill("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_skill_names() {
        let json = r#"{
            "skills": [
                {"name": "alpha", "llms_txt_url": "https://a.com/llms.txt"},
                {"name": "beta", "llms_txt_url": "https://b.com/llms.txt"}
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        let names = config.skill_names();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_get_base_url_explicit() {
        let skill = SkillConfig {
            name: "test".to_string(),
            description: String::new(),
            llms_txt_url: "https://example.com/llms.txt".to_string(),
            base_url: Some("https://custom.com".to_string()),
            path_prefix: None,
        };

        assert_eq!(skill.get_base_url().unwrap(), "https://custom.com");
    }

    #[test]
    fn test_get_base_url_derived() {
        let skill = SkillConfig {
            name: "test".to_string(),
            description: String::new(),
            llms_txt_url: "https://www.example.com/path/llms.txt".to_string(),
            base_url: None,
            path_prefix: None,
        };

        assert_eq!(skill.get_base_url().unwrap(), "https://www.example.com");
    }

    #[test]
    fn test_parse_config_without_repository() {
        let json = r#"{
            "skills": [
                {"name": "test", "llms_txt_url": "https://example.com/llms.txt"}
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        assert!(config.repository.is_none());
    }

    #[test]
    fn test_parse_config_with_repository() {
        let json = r#"{
            "skills": [],
            "repository": {
                "name": "my-repo",
                "bucket_name": "my-bucket",
                "region": "eu-west-1",
                "endpoint": "https://s3.example.com"
            }
        }"#;

        let config = Config::parse(json).unwrap();
        let repo = config.repository.unwrap();
        assert_eq!(repo.name.as_deref(), Some("my-repo"));
        assert_eq!(repo.bucket_name.as_deref(), Some("my-bucket"));
        assert_eq!(repo.region, "eu-west-1");
        assert_eq!(repo.endpoint.as_deref(), Some("https://s3.example.com"));
    }

    #[test]
    fn test_parse_repository_default_region() {
        let json = r#"{
            "skills": [],
            "repository": {
                "bucket_name": "my-bucket"
            }
        }"#;

        let config = Config::parse(json).unwrap();
        let repo = config.repository.unwrap();
        assert_eq!(repo.region, "us-east-1");
        assert!(repo.name.is_none());
        assert!(repo.endpoint.is_none());
    }

    #[test]
    fn test_multiple_skills_config() {
        let json = r#"{
            "skills": [
                {
                    "name": "shadcn-svelte",
                    "description": "Expert guidance for shadcn-svelte",
                    "llms_txt_url": "https://www.shadcn-svelte.com/llms.txt"
                },
                {
                    "name": "another-lib",
                    "description": "Another library skill",
                    "llms_txt_url": "https://another.example.com/llms.txt",
                    "base_url": "https://docs.another.example.com",
                    "path_prefix": "/api"
                }
            ]
        }"#;

        let config = Config::parse(json).unwrap();
        assert_eq!(config.skills.len(), 2);

        let shadcn = config.find_skill("shadcn-svelte").unwrap();
        assert_eq!(
            shadcn.get_base_url().unwrap(),
            "https://www.shadcn-svelte.com"
        );

        let another = config.find_skill("another-lib").unwrap();
        assert_eq!(
            another.get_base_url().unwrap(),
            "https://docs.another.example.com"
        );
    }

    #[test]
    fn test_parse_local_repository_config() {
        let json = r#"{
            "skills": [],
            "repository": {
                "local": {
                    "path": "/tmp/my-local-repo",
                    "cache": true
                },
                "bucket_name": "my-bucket"
            }
        }"#;

        let config = Config::parse(json).unwrap();
        let repo = config.repository.unwrap();
        assert!(repo.has_remote());
        assert!(repo.has_local());
        assert!(repo.local_is_cache());
        assert_eq!(repo.local_repo_path(), PathBuf::from("/tmp/my-local-repo"));
    }

    #[test]
    fn test_local_repo_default_path() {
        let json = r#"{
            "skills": [],
            "repository": {
                "local": {}
            }
        }"#;

        let config = Config::parse(json).unwrap();
        let repo = config.repository.unwrap();
        assert!(repo.has_local());
        assert!(!repo.has_remote());
        assert!(!repo.local_is_cache());
        assert_eq!(repo.local_repo_path(), default_local_repo_path());
    }

    #[test]
    fn test_has_remote_false_without_bucket() {
        let json = r#"{
            "skills": [],
            "repository": {
                "local": {"path": "/tmp/local"}
            }
        }"#;

        let config = Config::parse(json).unwrap();
        let repo = config.repository.unwrap();
        assert!(!repo.has_remote());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.skills.is_empty());
        assert!(config.repository.is_none());
    }

    #[test]
    fn test_config_merge_skills() {
        let mut base = Config::parse(
            r#"{"skills": [
            {"name": "a", "llms_txt_url": "https://a.com/llms.txt", "description": "old"},
            {"name": "b", "llms_txt_url": "https://b.com/llms.txt"}
        ]}"#,
        )
        .unwrap();

        let other = Config::parse(
            r#"{"skills": [
            {"name": "a", "llms_txt_url": "https://a.com/llms.txt", "description": "new"},
            {"name": "c", "llms_txt_url": "https://c.com/llms.txt"}
        ]}"#,
        )
        .unwrap();

        base.merge(&other);
        assert_eq!(base.skills.len(), 3);
        assert_eq!(base.find_skill("a").unwrap().description, "new");
        assert!(base.find_skill("b").is_some());
        assert!(base.find_skill("c").is_some());
    }

    #[test]
    fn test_config_merge_repository() {
        let mut base =
            Config::parse(r#"{"skills": [], "repository": {"bucket_name": "old"}}"#).unwrap();
        let other =
            Config::parse(r#"{"skills": [], "repository": {"bucket_name": "new"}}"#).unwrap();

        base.merge(&other);
        assert_eq!(base.repository.unwrap().bucket_name.as_deref(), Some("new"));
    }

    #[test]
    fn test_config_merge_no_repository_keeps_base() {
        let mut base =
            Config::parse(r#"{"skills": [], "repository": {"bucket_name": "base"}}"#).unwrap();
        let other = Config::parse(r#"{"skills": []}"#).unwrap();

        base.merge(&other);
        assert_eq!(
            base.repository.unwrap().bucket_name.as_deref(),
            Some("base")
        );
    }

    #[test]
    fn test_global_config_paths() {
        let dir = global_config_dir();
        assert!(dir.to_string_lossy().contains(".skill-builder"));

        let path = global_config_path();
        assert!(path.to_string_lossy().contains("skills.config.json"));
    }
}
