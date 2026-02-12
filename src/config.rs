//! Configuration file parsing for skills.json.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// A skill configuration entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillConfig {
    /// Unique name for the skill.
    pub name: String,

    /// Description of what the skill provides.
    #[serde(default)]
    pub description: String,

    /// URL to the llms.txt file.
    pub llms_txt_url: String,

    /// Base URL for resolving relative paths. Auto-derived from llms_txt_url if not set.
    #[serde(default)]
    pub base_url: Option<String>,

    /// Path prefix to strip from URLs when creating local paths. Auto-detected if not set.
    #[serde(default)]
    pub path_prefix: Option<String>,
}

impl SkillConfig {
    /// Get the base URL, deriving it from llms_txt_url if not explicitly set.
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

/// Repository configuration for S3-compatible skill storage.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryConfig {
    /// Display name for the repository.
    #[serde(default)]
    pub name: Option<String>,

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

fn default_region() -> String {
    "us-east-1".to_string()
}

/// Root configuration structure containing all skills.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// List of skill configurations.
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
    pub fn find_skill(&self, name: &str) -> Option<&SkillConfig> {
        self.skills.iter().find(|s| s.name == name)
    }

    /// Get all skill names.
    pub fn skill_names(&self) -> Vec<&str> {
        self.skills.iter().map(|s| s.name.as_str()).collect()
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
}
