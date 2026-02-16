//! Agent framework detection and install path resolution.

use std::path::{Path, PathBuf};

/// Supported agent frameworks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentFramework {
    Claude,
    OpenCode,
    Codex,
    Kiro,
}

/// All supported agent frameworks.
pub const ALL_FRAMEWORKS: &[AgentFramework] = &[
    AgentFramework::Claude,
    AgentFramework::OpenCode,
    AgentFramework::Codex,
    AgentFramework::Kiro,
];

impl AgentFramework {
    /// Display name for the agent.
    #[must_use] 
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::OpenCode => "OpenCode",
            Self::Codex => "Codex",
            Self::Kiro => "Kiro",
        }
    }

    /// Project-level skill install directory.
    #[must_use] 
    pub const fn project_skills_dir(&self) -> &'static str {
        match self {
            Self::Claude => ".claude/skills",
            Self::OpenCode => ".opencode/skills",
            Self::Codex => ".agents/skills",
            Self::Kiro => ".kiro/skills",
        }
    }

    /// Global skill install directory (under home).
    #[must_use] 
    pub fn global_skills_dir(&self) -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        match self {
            Self::Claude => home.join(".claude/skills"),
            Self::OpenCode => home.join(".config/opencode/skills"),
            Self::Codex => home.join(".codex/skills"),
            Self::Kiro => home.join(".kiro/skills"),
        }
    }

    /// Directory markers that indicate this agent is configured in a project.
    const fn project_dir_markers(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => &[".claude"],
            Self::OpenCode => &[".opencode"],
            Self::Codex => &[".codex"],
            Self::Kiro => &[".kiro"],
        }
    }

    /// File markers that indicate this agent is configured in a project.
    const fn project_file_markers(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => &["CLAUDE.md"],
            Self::OpenCode => &["opencode.json"],
            Self::Codex => &["AGENTS.md"],
            Self::Kiro => &[],
        }
    }

    /// Directory markers for global detection (relative to home).
    const fn global_dir_markers(&self) -> &'static [&'static str] {
        match self {
            Self::Claude => &[".claude"],
            Self::OpenCode => &[".config/opencode"],
            Self::Codex => &[".codex"],
            Self::Kiro => &[".kiro"],
        }
    }
}

/// Target specification for agent installation.
#[derive(Debug, Clone)]
pub enum AgentTarget {
    Specific(AgentFramework),
    All,
    Auto,
}

/// Parse an `--agent` flag value into an `AgentTarget`.
pub fn parse_agent_flag(value: Option<&str>) -> anyhow::Result<AgentTarget> {
    match value {
        None => Ok(AgentTarget::Auto),
        Some("claude") => Ok(AgentTarget::Specific(AgentFramework::Claude)),
        Some("opencode") => Ok(AgentTarget::Specific(AgentFramework::OpenCode)),
        Some("codex") => Ok(AgentTarget::Specific(AgentFramework::Codex)),
        Some("kiro") => Ok(AgentTarget::Specific(AgentFramework::Kiro)),
        Some("all") => Ok(AgentTarget::All),
        Some(other) => anyhow::bail!(
            "Unknown agent '{other}'. Valid options: claude, opencode, codex, kiro, all"
        ),
    }
}

/// Detect which agent frameworks are configured in a project directory.
#[must_use] 
pub fn detect_project_agents(project_root: &Path) -> Vec<AgentFramework> {
    let mut agents: Vec<AgentFramework> = ALL_FRAMEWORKS
        .iter()
        .copied()
        .filter(|agent| {
            agent
                .project_dir_markers()
                .iter()
                .any(|d| project_root.join(d).is_dir())
                || agent
                    .project_file_markers()
                    .iter()
                    .any(|f| project_root.join(f).exists())
        })
        .collect();

    if agents.is_empty() {
        agents.push(AgentFramework::Claude);
    }

    agents
}

/// Detect which agent frameworks are configured globally.
#[must_use] 
pub fn detect_global_agents() -> Vec<AgentFramework> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut agents: Vec<AgentFramework> = ALL_FRAMEWORKS
        .iter()
        .copied()
        .filter(|agent| {
            agent
                .global_dir_markers()
                .iter()
                .any(|d| home.join(d).is_dir())
        })
        .collect();

    if agents.is_empty() {
        agents.push(AgentFramework::Claude);
    }

    agents
}

/// Resolve installation directories based on target, explicit dir, and global flag.
///
/// Priority:
/// 1. If `explicit_dir` is Some, return just that path (overrides everything)
/// 2. If target is Specific, return that agent's dir
/// 3. If target is All, return all supported agent dirs
/// 4. If target is Auto, detect agents and return dirs for all detected
#[must_use] 
pub fn resolve_install_dirs(
    target: &AgentTarget,
    explicit_dir: Option<&Path>,
    global: bool,
    project_root: &Path,
) -> Vec<PathBuf> {
    // Explicit dir overrides everything
    if let Some(dir) = explicit_dir {
        return vec![dir.to_path_buf()];
    }

    let agent_to_dir = |agent: &AgentFramework| -> PathBuf {
        if global {
            agent.global_skills_dir()
        } else {
            PathBuf::from(agent.project_skills_dir())
        }
    };

    match target {
        AgentTarget::Specific(agent) => vec![agent_to_dir(agent)],
        AgentTarget::All => ALL_FRAMEWORKS.iter().map(agent_to_dir).collect(),
        AgentTarget::Auto => {
            let agents = if global {
                detect_global_agents()
            } else {
                detect_project_agents(project_root)
            };
            agents.iter().map(agent_to_dir).collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_agent_flag_none() {
        let target = parse_agent_flag(None).unwrap();
        assert!(matches!(target, AgentTarget::Auto));
    }

    #[test]
    fn test_parse_agent_flag_specific() {
        let target = parse_agent_flag(Some("claude")).unwrap();
        assert!(matches!(
            target,
            AgentTarget::Specific(AgentFramework::Claude)
        ));

        let target = parse_agent_flag(Some("opencode")).unwrap();
        assert!(matches!(
            target,
            AgentTarget::Specific(AgentFramework::OpenCode)
        ));

        let target = parse_agent_flag(Some("codex")).unwrap();
        assert!(matches!(
            target,
            AgentTarget::Specific(AgentFramework::Codex)
        ));

        let target = parse_agent_flag(Some("kiro")).unwrap();
        assert!(matches!(
            target,
            AgentTarget::Specific(AgentFramework::Kiro)
        ));
    }

    #[test]
    fn test_parse_agent_flag_all() {
        let target = parse_agent_flag(Some("all")).unwrap();
        assert!(matches!(target, AgentTarget::All));
    }

    #[test]
    fn test_parse_agent_flag_invalid() {
        let result = parse_agent_flag(Some("invalid"));
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_project_agents_claude_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Claude]);
    }

    #[test]
    fn test_detect_project_agents_claude_md() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("CLAUDE.md"), "# Claude").unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Claude]);
    }

    #[test]
    fn test_detect_project_agents_opencode() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".opencode")).unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::OpenCode]);
    }

    #[test]
    fn test_detect_project_agents_opencode_json() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("opencode.json"), "{}").unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::OpenCode]);
    }

    #[test]
    fn test_detect_project_agents_codex_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Codex]);
    }

    #[test]
    fn test_detect_project_agents_agents_md() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("AGENTS.md"), "# Agents").unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Codex]);
    }

    #[test]
    fn test_detect_project_agents_kiro_dir() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".kiro")).unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Kiro]);
    }

    #[test]
    fn test_detect_project_agents_multiple() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".opencode")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".codex")).unwrap();
        std::fs::create_dir_all(tmp.path().join(".kiro")).unwrap();

        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents.len(), 4);
        assert!(agents.contains(&AgentFramework::Claude));
        assert!(agents.contains(&AgentFramework::OpenCode));
        assert!(agents.contains(&AgentFramework::Codex));
        assert!(agents.contains(&AgentFramework::Kiro));
    }

    #[test]
    fn test_detect_project_agents_default_to_claude() {
        let tmp = TempDir::new().unwrap();
        let agents = detect_project_agents(tmp.path());
        assert_eq!(agents, vec![AgentFramework::Claude]);
    }

    #[test]
    fn test_resolve_explicit_dir_overrides() {
        let explicit = PathBuf::from("/custom/path");
        let dirs = resolve_install_dirs(&AgentTarget::All, Some(&explicit), false, Path::new("."));
        assert_eq!(dirs, vec![PathBuf::from("/custom/path")]);
    }

    #[test]
    fn test_resolve_specific_agent() {
        let p = Path::new(".");
        let dirs = resolve_install_dirs(
            &AgentTarget::Specific(AgentFramework::Claude),
            None,
            false,
            p,
        );
        assert_eq!(dirs, vec![PathBuf::from(".claude/skills")]);

        let dirs = resolve_install_dirs(
            &AgentTarget::Specific(AgentFramework::OpenCode),
            None,
            false,
            p,
        );
        assert_eq!(dirs, vec![PathBuf::from(".opencode/skills")]);

        let dirs = resolve_install_dirs(
            &AgentTarget::Specific(AgentFramework::Codex),
            None,
            false,
            p,
        );
        assert_eq!(dirs, vec![PathBuf::from(".agents/skills")]);
    }

    #[test]
    fn test_resolve_specific_kiro() {
        let dirs = resolve_install_dirs(
            &AgentTarget::Specific(AgentFramework::Kiro),
            None,
            false,
            Path::new("."),
        );
        assert_eq!(dirs, vec![PathBuf::from(".kiro/skills")]);
    }

    #[test]
    fn test_resolve_all_agents() {
        let dirs = resolve_install_dirs(&AgentTarget::All, None, false, Path::new("."));
        assert_eq!(dirs.len(), 4);
        assert_eq!(dirs[0], PathBuf::from(".claude/skills"));
        assert_eq!(dirs[1], PathBuf::from(".opencode/skills"));
        assert_eq!(dirs[2], PathBuf::from(".agents/skills"));
        assert_eq!(dirs[3], PathBuf::from(".kiro/skills"));
    }

    #[test]
    fn test_agent_project_skills_dirs() {
        assert_eq!(
            AgentFramework::Claude.project_skills_dir(),
            ".claude/skills"
        );
        assert_eq!(
            AgentFramework::OpenCode.project_skills_dir(),
            ".opencode/skills"
        );
        assert_eq!(AgentFramework::Codex.project_skills_dir(), ".agents/skills");
        assert_eq!(AgentFramework::Kiro.project_skills_dir(), ".kiro/skills");
    }
}
