//! Download llms.txt and referenced documentation files.

use anyhow::{Context, Result};
use regex::Regex;
use reqwest::blocking::Client;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use url::Url;

use crate::config::SkillConfig;
use crate::output::Output;

/// HTTP client with reasonable defaults.
fn create_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(60))
        .user_agent("sb/1.0")
        .build()
        .context("Failed to create HTTP client")
}

/// Download content from a URL.
pub fn download_url(client: &Client, url: &str) -> Result<String> {
    let response = client
        .get(url)
        .send()
        .with_context(|| format!("Failed to fetch {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {}", response.status(), url);
    }

    response
        .text()
        .with_context(|| format!("Failed to read response from {url}"))
}

/// Extract all .md URLs from llms.txt content.
#[must_use] 
pub fn extract_urls(content: &str) -> Vec<String> {
    let re = Regex::new(r"https?://[^\s\)>\]]+\.md").unwrap();
    let urls: HashSet<String> = re
        .find_iter(content)
        .map(|m| m.as_str().to_string())
        .collect();
    let mut urls: Vec<String> = urls.into_iter().collect();
    urls.sort();
    urls
}

/// Auto-detect the common path prefix from a list of URLs.
#[must_use] 
pub fn detect_path_prefix(urls: &[String]) -> Option<String> {
    if urls.is_empty() {
        return None;
    }

    // Parse all URLs and extract their paths
    let paths: Vec<String> = urls
        .iter()
        .filter_map(|u| Url::parse(u).ok())
        .map(|u| u.path().to_string())
        .collect();

    if paths.is_empty() {
        return None;
    }

    // Find common prefix by splitting paths into segments
    let segments: Vec<Vec<&str>> = paths
        .iter()
        .map(|p| p.split('/').filter(|s| !s.is_empty()).collect())
        .collect();

    // Find common prefix segments (excluding the filename)
    let mut common_prefix = Vec::new();
    if let Some(first) = segments.first() {
        // Only look at directory segments (exclude the last one which is the filename)
        let dir_count = if first.len() > 1 { first.len() - 1 } else { 0 };

        for (i, &segment) in first.iter().enumerate().take(dir_count) {
            if segments.iter().all(|s| s.get(i) == Some(&segment)) {
                common_prefix.push(segment);
            } else {
                break;
            }
        }
    }

    if common_prefix.is_empty() {
        None
    } else {
        Some(format!("/{}", common_prefix.join("/")))
    }
}

/// Convert a URL to a local file path within the source directory.
pub fn url_to_local_path(url: &str, path_prefix: Option<&str>) -> Result<PathBuf> {
    let parsed = Url::parse(url).with_context(|| format!("Invalid URL: {url}"))?;
    let mut path = parsed.path().to_string();

    // Strip the path prefix if specified
    if let Some(prefix) = path_prefix {
        if let Some(stripped) = path.strip_prefix(prefix) {
            path = stripped.to_string();
        }
    }

    // Remove leading slash
    let path = path.trim_start_matches('/');

    // Create path under docs/
    Ok(PathBuf::from("docs").join(path))
}

/// Update llms.txt content to use local file paths.
#[must_use] 
pub fn update_llms_txt_paths(content: &str, urls: &[String], path_prefix: Option<&str>) -> String {
    let mut updated = content.to_string();

    for url in urls {
        if let Ok(local_path) = url_to_local_path(url, path_prefix) {
            updated = updated.replace(url, &local_path.to_string_lossy());
        }
    }

    updated
}

/// Download result for a single file.
#[derive(Debug)]
pub struct DownloadResult {
    pub url: String,
    pub local_path: PathBuf,
    pub success: bool,
    pub error: Option<String>,
}

/// Download all documentation for a skill.
pub fn download_skill_docs(
    skill: &SkillConfig,
    source_dir: &Path,
    output: &Output,
) -> Result<Vec<DownloadResult>> {
    let client = create_client()?;

    let pb = output.spinner(&format!("Downloading llms.txt from {}", skill.llms_txt_url));

    let llms_content = download_url(&client, &skill.llms_txt_url)?;
    let urls = extract_urls(&llms_content);
    pb.finish_and_clear();

    output.info(&format!("Found {} .md files to download", urls.len()));

    // Auto-detect path prefix if not specified
    let path_prefix = skill
        .path_prefix
        .clone()
        .or_else(|| detect_path_prefix(&urls));

    if let Some(ref prefix) = path_prefix {
        output.step(&format!("Using path prefix: {prefix}"));
    }

    // Prepare source directory
    let skill_source_dir = source_dir.join(&skill.name);
    let docs_dir = skill_source_dir.join("docs");

    // Clear existing docs
    if docs_dir.exists() {
        for entry in fs::read_dir(&docs_dir)? {
            let entry = entry?;
            if entry.path().is_file() && entry.path().extension().is_some_and(|e| e == "md") {
                fs::remove_file(entry.path())?;
            }
        }
    }

    fs::create_dir_all(&docs_dir)?;

    // Download each file
    let mut results = Vec::new();
    let progress = output.progress_bar(urls.len() as u64, "Downloading docs");

    for url in &urls {
        let local_path = url_to_local_path(url, path_prefix.as_deref())?;
        let full_path = skill_source_dir.join(&local_path);

        // Create parent directories
        if let Some(parent) = full_path.parent() {
            fs::create_dir_all(parent)?;
        }

        match download_url(&client, url) {
            Ok(content) => {
                fs::write(&full_path, &content)?;
                results.push(DownloadResult {
                    url: url.clone(),
                    local_path,
                    success: true,
                    error: None,
                });
            }
            Err(e) => {
                results.push(DownloadResult {
                    url: url.clone(),
                    local_path: local_path.clone(),
                    success: false,
                    error: Some(e.to_string()),
                });
                output.warn(&format!("Failed: {}", local_path.display()));
            }
        }
        progress.inc(1);
    }
    progress.finish_and_clear();

    // Update llms.txt with local paths and save
    let updated_llms = update_llms_txt_paths(&llms_content, &urls, path_prefix.as_deref());
    let llms_path = skill_source_dir.join("llms.txt");
    fs::write(&llms_path, updated_llms)?;

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.iter().filter(|r| !r.success).count();

    output.status("Downloaded", &format!("{success_count} files"));
    if fail_count > 0 {
        output.warn(&format!("Failed to download {fail_count} files"));
    }

    Ok(results)
}

/// Download docs from a URL without a config file.
pub fn download_from_url(
    url: &str,
    name: &str,
    source_dir: &Path,
    output: &Output,
) -> Result<Vec<DownloadResult>> {
    let skill = SkillConfig {
        name: name.to_string(),
        description: String::new(),
        llms_txt_url: url.to_string(),
        base_url: None,
        path_prefix: None,
    };

    download_skill_docs(&skill, source_dir, output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_urls_basic() {
        let content = r#"
# Documentation

- [Guide](https://example.com/docs/guide.md)
- [API](https://example.com/docs/api.md)
"#;

        let urls = extract_urls(content);
        assert_eq!(urls.len(), 2);
        assert!(urls.contains(&"https://example.com/docs/guide.md".to_string()));
        assert!(urls.contains(&"https://example.com/docs/api.md".to_string()));
    }

    #[test]
    fn test_extract_urls_deduplicates() {
        let content = r#"
- [Link1](https://example.com/docs/file.md)
- [Link2](https://example.com/docs/file.md)
"#;

        let urls = extract_urls(content);
        assert_eq!(urls.len(), 1);
    }

    #[test]
    fn test_extract_urls_various_formats() {
        let content = r#"
Plain URL: https://example.com/doc.md
Markdown: [text](https://example.com/other.md)
In brackets: <https://example.com/another.md>
"#;

        let urls = extract_urls(content);
        assert_eq!(urls.len(), 3);
    }

    #[test]
    fn test_extract_urls_ignores_non_md() {
        let content = r#"
- https://example.com/image.png
- https://example.com/style.css
- https://example.com/doc.md
"#;

        let urls = extract_urls(content);
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "https://example.com/doc.md");
    }

    #[test]
    fn test_url_to_local_path_no_prefix() {
        let path = url_to_local_path("https://example.com/docs/guide.md", None).unwrap();
        assert_eq!(path, PathBuf::from("docs/docs/guide.md"));
    }

    #[test]
    fn test_url_to_local_path_with_prefix() {
        let path = url_to_local_path("https://example.com/docs/guide.md", Some("/docs")).unwrap();
        assert_eq!(path, PathBuf::from("docs/guide.md"));
    }

    #[test]
    fn test_url_to_local_path_nested() {
        let path = url_to_local_path(
            "https://example.com/docs/components/button.md",
            Some("/docs"),
        )
        .unwrap();
        assert_eq!(path, PathBuf::from("docs/components/button.md"));
    }

    #[test]
    fn test_detect_path_prefix_common() {
        let urls = vec![
            "https://example.com/docs/guide.md".to_string(),
            "https://example.com/docs/api.md".to_string(),
            "https://example.com/docs/components/button.md".to_string(),
        ];

        let prefix = detect_path_prefix(&urls);
        assert_eq!(prefix, Some("/docs".to_string()));
    }

    #[test]
    fn test_detect_path_prefix_deeper() {
        let urls = vec![
            "https://example.com/api/v2/docs/guide.md".to_string(),
            "https://example.com/api/v2/docs/ref.md".to_string(),
        ];

        let prefix = detect_path_prefix(&urls);
        assert_eq!(prefix, Some("/api/v2/docs".to_string()));
    }

    #[test]
    fn test_detect_path_prefix_no_common() {
        let urls = vec![
            "https://example.com/guide.md".to_string(),
            "https://example.com/api.md".to_string(),
        ];

        let prefix = detect_path_prefix(&urls);
        assert_eq!(prefix, None);
    }

    #[test]
    fn test_detect_path_prefix_empty() {
        let urls: Vec<String> = vec![];
        let prefix = detect_path_prefix(&urls);
        assert_eq!(prefix, None);
    }

    #[test]
    fn test_update_llms_txt_paths() {
        let content = r#"
# Docs

- [Guide](https://example.com/docs/guide.md)
- [API](https://example.com/docs/api.md)
"#;

        let urls = vec![
            "https://example.com/docs/guide.md".to_string(),
            "https://example.com/docs/api.md".to_string(),
        ];

        let updated = update_llms_txt_paths(content, &urls, Some("/docs"));

        assert!(updated.contains("docs/guide.md"));
        assert!(updated.contains("docs/api.md"));
        assert!(!updated.contains("https://"));
    }

    #[test]
    fn test_http_url() {
        let urls = extract_urls("Check http://example.com/doc.md for more");
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0], "http://example.com/doc.md");
    }
}
