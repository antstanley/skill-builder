//! Integration tests for download module with mock HTTP.

mod common;

use skill_builder::config::SkillConfig;
use skill_builder::download::{
    detect_path_prefix, download_skill_docs, extract_urls, update_llms_txt_paths, url_to_local_path,
};
use std::fs;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_download_llms_txt_from_mock_server() {
    let mock_server = MockServer::start().await;

    // Mock llms.txt
    Mock::given(method("GET"))
        .and(path("/llms.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"# Documentation

- [Guide](MOCK_URL/docs/guide.md)
- [API](MOCK_URL/docs/api.md)
"#
            .replace("MOCK_URL", &mock_server.uri()),
        ))
        .mount(&mock_server)
        .await;

    // Mock doc files
    Mock::given(method("GET"))
        .and(path("/docs/guide.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# Guide\n\nContent."))
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/docs/api.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# API\n\nReference."))
        .mount(&mock_server)
        .await;

    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    let skill = SkillConfig {
        name: "test-skill".to_string(),
        description: String::new(),
        llms_txt_url: format!("{}/llms.txt", mock_server.uri()),
        base_url: None,
        path_prefix: None,
    };

    // Run blocking operation in a separate thread
    let results = tokio::task::spawn_blocking(move || {
        download_skill_docs(&skill, &temp_path)
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(results.len(), 2);
    assert!(results.iter().all(|r| r.success));

    // Verify files were created
    let skill_dir = temp.path().join("test-skill");
    assert!(skill_dir.join("llms.txt").exists());
    assert!(skill_dir.join("docs/guide.md").exists());
    assert!(skill_dir.join("docs/api.md").exists());
}

#[tokio::test]
async fn test_handle_404_gracefully() {
    let mock_server = MockServer::start().await;

    // Mock llms.txt with a link to a non-existent file
    Mock::given(method("GET"))
        .and(path("/llms.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(&format!(
            "# Docs\n- [Missing]({}/docs/missing.md)",
            mock_server.uri()
        )))
        .mount(&mock_server)
        .await;

    // Return 404 for the doc
    Mock::given(method("GET"))
        .and(path("/docs/missing.md"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;

    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    let skill = SkillConfig {
        name: "test-skill".to_string(),
        description: String::new(),
        llms_txt_url: format!("{}/llms.txt", mock_server.uri()),
        base_url: None,
        path_prefix: None,
    };

    let results = tokio::task::spawn_blocking(move || {
        download_skill_docs(&skill, &temp_path)
    })
    .await
    .unwrap()
    .unwrap();

    // Should have one result with success=false
    assert_eq!(results.len(), 1);
    assert!(!results[0].success);
    assert!(results[0].error.is_some());
}

#[tokio::test]
async fn test_handle_redirect() {
    let mock_server = MockServer::start().await;

    // Mock llms.txt with redirect
    Mock::given(method("GET"))
        .and(path("/llms.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_string(&format!(
            "# Docs\n- [Doc]({}/docs/doc.md)",
            mock_server.uri()
        )))
        .mount(&mock_server)
        .await;

    // Redirect to final location
    Mock::given(method("GET"))
        .and(path("/docs/doc.md"))
        .respond_with(
            ResponseTemplate::new(302).insert_header("Location", &format!("{}/final.md", mock_server.uri())),
        )
        .mount(&mock_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/final.md"))
        .respond_with(ResponseTemplate::new(200).set_body_string("# Final Content"))
        .mount(&mock_server)
        .await;

    let temp = TempDir::new().unwrap();
    let temp_path = temp.path().to_path_buf();

    let skill = SkillConfig {
        name: "test-skill".to_string(),
        description: String::new(),
        llms_txt_url: format!("{}/llms.txt", mock_server.uri()),
        base_url: None,
        path_prefix: None,
    };

    // reqwest follows redirects by default
    let results = tokio::task::spawn_blocking(move || {
        download_skill_docs(&skill, &temp_path)
    })
    .await
    .unwrap()
    .unwrap();

    assert_eq!(results.len(), 1);
    assert!(results[0].success);
}

#[test]
fn test_extract_urls_from_fixture() {
    let content = fs::read_to_string(common::fixture_path("sample_llms.txt")).unwrap();
    let urls = extract_urls(&content);

    assert_eq!(urls.len(), 4);
    assert!(urls.contains(&"https://example.com/docs/guide.md".to_string()));
    assert!(urls.contains(&"https://example.com/docs/api.md".to_string()));
    assert!(urls.contains(&"https://example.com/docs/components/button.md".to_string()));
    assert!(urls.contains(&"https://example.com/docs/components/dialog.md".to_string()));
}

#[test]
fn test_url_to_local_path_integration() {
    let path = url_to_local_path(
        "https://www.shadcn-svelte.com/docs/components/button.md",
        Some("/docs"),
    )
    .unwrap();

    assert_eq!(
        path.to_string_lossy(),
        "docs/components/button.md"
    );
}

#[test]
fn test_detect_path_prefix_from_real_urls() {
    let urls = vec![
        "https://www.shadcn-svelte.com/docs/about.md".to_string(),
        "https://www.shadcn-svelte.com/docs/components/button.md".to_string(),
        "https://www.shadcn-svelte.com/docs/components/dialog.md".to_string(),
        "https://www.shadcn-svelte.com/docs/installation/sveltekit.md".to_string(),
    ];

    let prefix = detect_path_prefix(&urls);
    assert_eq!(prefix, Some("/docs".to_string()));
}

#[test]
fn test_update_llms_txt_paths_integration() {
    let content = r#"# shadcn-svelte

- [About](https://example.com/docs/about.md)
- [Button](https://example.com/docs/components/button.md)
"#;

    let urls = vec![
        "https://example.com/docs/about.md".to_string(),
        "https://example.com/docs/components/button.md".to_string(),
    ];

    let updated = update_llms_txt_paths(content, &urls, Some("/docs"));

    assert!(updated.contains("docs/about.md"));
    assert!(updated.contains("docs/components/button.md"));
    assert!(!updated.contains("https://"));
}
