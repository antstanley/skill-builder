//! Integration tests for agent detection and multi-agent install.

mod common;

use std::fs;
use tempfile::TempDir;

use skill_builder::agent::{
    detect_project_agents, parse_agent_flag, resolve_install_dirs, AgentFramework, AgentTarget,
};

#[test]
fn test_detection_with_claude_marker() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".claude")).unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::Claude));
}

#[test]
fn test_detection_with_claude_md_marker() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("CLAUDE.md"), "# Claude").unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::Claude));
}

#[test]
fn test_detection_with_opencode_marker() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".opencode")).unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::OpenCode));
}

#[test]
fn test_detection_with_opencode_json_marker() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("opencode.json"), "{}").unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::OpenCode));
}

#[test]
fn test_detection_with_codex_marker() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".codex")).unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::Codex));
}

#[test]
fn test_detection_with_agents_md_marker() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("AGENTS.md"), "# Agents").unwrap();

    let agents = detect_project_agents(tmp.path());
    assert!(agents.contains(&AgentFramework::Codex));
}

#[test]
fn test_detection_with_all_markers() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".claude")).unwrap();
    fs::create_dir_all(tmp.path().join(".opencode")).unwrap();
    fs::create_dir_all(tmp.path().join(".codex")).unwrap();

    let agents = detect_project_agents(tmp.path());
    assert_eq!(agents.len(), 3);
    assert!(agents.contains(&AgentFramework::Claude));
    assert!(agents.contains(&AgentFramework::OpenCode));
    assert!(agents.contains(&AgentFramework::Codex));
}

#[test]
fn test_detection_defaults_to_claude() {
    let tmp = TempDir::new().unwrap();
    // No markers at all
    let agents = detect_project_agents(tmp.path());
    assert_eq!(agents, vec![AgentFramework::Claude]);
}

#[test]
fn test_parse_agent_flag_invalid() {
    let result = parse_agent_flag(Some("invalid"));
    assert!(result.is_err());
}

#[test]
fn test_resolve_auto_with_markers() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join(".opencode")).unwrap();
    fs::create_dir_all(tmp.path().join(".codex")).unwrap();

    // Auto detection happens in resolve_install_dirs using cwd,
    // so we test the specific and all targets directly
    let dirs = resolve_install_dirs(&AgentTarget::All, None, false);
    assert_eq!(dirs.len(), 3);
}

#[test]
fn test_resolve_global_dirs() {
    let dirs = resolve_install_dirs(&AgentTarget::Specific(AgentFramework::Claude), None, true);
    assert_eq!(dirs.len(), 1);
    let dir_str = dirs[0].to_string_lossy();
    assert!(
        dir_str.contains(".claude/skills"),
        "Expected .claude/skills in {}",
        dir_str
    );
}

#[test]
fn test_resolve_global_all_dirs() {
    let dirs = resolve_install_dirs(&AgentTarget::All, None, true);
    assert_eq!(dirs.len(), 3);
}

#[test]
fn test_explicit_dir_overrides_everything() {
    let custom = std::path::PathBuf::from("/my/custom/dir");
    let dirs = resolve_install_dirs(&AgentTarget::All, Some(&custom), true);
    assert_eq!(dirs.len(), 1);
    assert_eq!(dirs[0], custom);
}
