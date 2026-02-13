//! sb CLI - Build Claude Code skills from llms.txt URLs.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use skill_builder::config::Config;
use skill_builder::download::{download_from_url, download_skill_docs};
use skill_builder::index::load_index;
use skill_builder::install::install_from_file;
use skill_builder::local_storage::LocalStorageClient;
use skill_builder::output::Output;
use skill_builder::repository::{Repository, UploadParams};
use skill_builder::storage::StorageOperations;
use skill_builder::validate::{print_validation_result, validate_skill};

/// Build Claude Code skills from llms.txt URLs.
#[derive(Parser)]
#[command(name = "sb")]
#[command(
    version,
    about,
    long_about = "A CLI tool that builds Claude Code skills from any llms.txt URL.\n\nSkills are built by downloading documentation, validating the skill structure,\npackaging into distributable .skill files, and optionally publishing to an\nS3-compatible repository.\n\nConfigure skills in a skills.json file or use --url for ad-hoc downloads."
)]
#[command(
    after_help = "Examples:\n  sb download my-skill\n  sb validate my-skill\n  sb package my-skill --output dist/\n  sb install my-skill --version 1.0.0\n  sb repo upload my-skill 1.0.0\n  sb local list"
)]
struct Cli {
    /// Path to skills configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Output plain text with prefixed lines for agent consumption
    #[arg(long, global = true)]
    agent_output: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download documentation for a skill
    #[command(
        long_about = "Download documentation for a skill from its llms.txt URL.\n\nFetches the llms.txt index, extracts all linked .md files, and saves them\nlocally. Use a skill name from skills.json or provide a URL directly.",
        after_help = "Examples:\n  sb download my-skill\n  sb download --all\n  sb download --url https://example.com/llms.txt --name my-skill\n  sb download my-skill --source-dir ./docs"
    )]
    Download {
        /// Name of the skill to download (from skills.json)
        skill_name: Option<String>,

        /// Download all skills from config
        #[arg(long)]
        all: bool,

        /// Download from URL directly (without config)
        #[arg(long)]
        url: Option<String>,

        /// Skill name when using --url
        #[arg(long)]
        name: Option<String>,

        /// Source directory for downloaded docs
        #[arg(long, default_value = "source")]
        source_dir: PathBuf,
    },

    /// Validate a skill's structure and metadata
    #[command(
        long_about = "Validate a skill's structure and metadata.\n\nChecks that the skill directory contains a valid SKILL.md with YAML frontmatter\nincluding required name and description fields. Also checks for a references/\ndirectory and warns about unresolved TODOs.",
        after_help = "Examples:\n  sb validate my-skill\n  sb validate ./path/to/skill\n  sb validate my-skill --skills-dir ./custom-skills"
    )]
    Validate {
        /// Name of the skill to validate, or path to skill directory
        skill: String,

        /// Directory containing skills
        #[arg(long, default_value = "skills")]
        skills_dir: PathBuf,
    },

    /// Package a skill into a distributable .skill file
    #[command(
        long_about = "Package a skill into a distributable .skill file.\n\nValidates the skill, then creates a zip archive containing the SKILL.md and\nreferences/ directory. The output file is named <skill-name>.skill.",
        after_help = "Examples:\n  sb package my-skill\n  sb package my-skill --output ./releases\n  sb package ./path/to/skill --output dist/"
    )]
    Package {
        /// Name of the skill to package, or path to skill directory
        skill: String,

        /// Output directory for the .skill file
        #[arg(short, long, default_value = "dist")]
        output: PathBuf,

        /// Directory containing skills
        #[arg(long, default_value = "skills")]
        skills_dir: PathBuf,
    },

    /// Install a skill from local repo, remote repo, or GitHub releases
    #[command(
        long_about = "Install a skill from the local repository, remote S3 repository, or GitHub releases.\n\nBy default, searches local repo → remote repo → GitHub releases in order.\nUse --local, --remote, or --github to restrict to a single source.\nAlternatively, use --file to install from a local .skill file directly.\n\nSkills are installed to all detected agent directories by default.\nUse --agent to target a specific agent, or --install-dir to override.",
        after_help = "Examples:\n  sb install my-skill\n  sb install my-skill --version 1.0.0\n  sb install my-skill --local\n  sb install my-skill --remote\n  sb install my-skill --github --repo user/repo\n  sb install my-skill --file ./dist/my-skill.skill\n  sb install my-skill --install-dir ~/.claude/skills\n  sb install my-skill --agent codex\n  sb install my-skill --agent all\n  sb install my-skill --global"
    )]
    Install {
        /// Name of the skill to install
        skill: String,

        /// Specific version to install (default: latest)
        #[arg(short, long)]
        version: Option<String>,

        /// GitHub repository (owner/repo)
        #[arg(long)]
        repo: Option<String>,

        /// Install from local .skill file instead of downloading
        #[arg(long)]
        file: Option<PathBuf>,

        /// Install from local repository only
        #[arg(long, conflicts_with_all = ["remote", "github", "file"])]
        local: bool,

        /// Install from remote S3 repository only
        #[arg(long, conflicts_with_all = ["local", "github", "file"])]
        remote: bool,

        /// Install from GitHub releases only
        #[arg(long, conflicts_with_all = ["local", "remote", "file"])]
        github: bool,

        /// Installation directory (overrides agent detection)
        #[arg(long)]
        install_dir: Option<PathBuf>,

        /// Target agent framework: claude, opencode, codex, kiro, or all
        #[arg(long)]
        agent: Option<String>,

        /// Install to global agent directories instead of project-level
        #[arg(long)]
        global: bool,
    },

    /// List all skills in configuration
    #[command(
        long_about = "List all skills defined in the skills.json configuration file.\n\nDisplays each skill's name, llms.txt URL, and description."
    )]
    List,

    /// Manage the S3-compatible skill repository
    #[command(
        long_about = "Manage skills in an S3-compatible hosted repository.\n\nRequires a 'repository' section in skills.json with bucket_name and optional\nregion/endpoint. Authentication uses the standard AWS credential chain\n(environment variables, ~/.aws/credentials, IAM roles).",
        after_help = "Examples:\n  sb repo upload my-skill 1.0.0\n  sb repo download my-skill --version 1.0.0\n  sb repo install my-skill\n  sb repo delete my-skill --yes\n  sb repo list"
    )]
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },

    /// Manage the local skill repository
    #[command(
        long_about = "Manage the local skill repository.\n\nSkills can be stored locally for offline access or as a cache for the remote\nrepository. Local repository is stored at $HOME/.skill-builder/local/ by default.",
        after_help = "Examples:\n  sb local list\n  sb local clear\n  sb local clear --skill my-skill"
    )]
    Local {
        #[command(subcommand)]
        action: LocalAction,
    },

    /// Initialize global configuration
    #[command(
        long_about = "Initialize the global skill-builder configuration.\n\nCreates a configuration file at $HOME/.skill-builder/skills.config.json with\noptions for setting up a local skill repository. Run this once to get started.",
        after_help = "Examples:\n  sb init"
    )]
    Init,
}

#[derive(Subcommand)]
enum RepoAction {
    /// Upload a skill to the repository
    #[command(
        long_about = "Upload a .skill file to the S3 repository.\n\nIf --file is not specified, defaults to dist/<skill>.skill. Skill metadata\n(description, llms_txt_url) is read from skills.json if available.\nOptionally include a CHANGELOG.md and/or archive the source directory.",
        after_help = "Examples:\n  sb repo upload my-skill 1.0.0\n  sb repo upload my-skill 1.0.0 --file ./my-skill.skill\n  sb repo upload my-skill 1.0.0 --changelog CHANGELOG.md --source-dir ./source"
    )]
    Upload {
        /// Skill name
        skill: String,

        /// Version to upload (e.g. "1.0.0")
        version: String,

        /// Path to the .skill file [default: dist/<skill>.skill]
        #[arg(long)]
        file: Option<PathBuf>,

        /// Path to a CHANGELOG.md file to include
        #[arg(long)]
        changelog: Option<PathBuf>,

        /// Path to source directory to archive and upload
        #[arg(long)]
        source_dir: Option<PathBuf>,
    },

    /// Download a skill from the repository
    #[command(
        long_about = "Download a .skill file from the S3 repository.\n\nDownloads the specified version (or latest) and caches it locally.\nIf --output is specified, copies the file to that directory.",
        after_help = "Examples:\n  sb repo download my-skill\n  sb repo download my-skill --version 1.0.0\n  sb repo download my-skill --output ./downloads"
    )]
    Download {
        /// Skill name
        skill: String,

        /// Version to download (default: latest)
        #[arg(long)]
        version: Option<String>,

        /// Output directory
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Download and install a skill from the repository
    #[command(
        long_about = "Download a skill from the S3 repository and install it.\n\nCombines download and install in one step: fetches the .skill file\n(using cache when available) and extracts it to the install directory.",
        after_help = "Examples:\n  sb repo install my-skill\n  sb repo install my-skill --version 1.0.0\n  sb repo install my-skill --install-dir ~/.claude/skills\n  sb repo install my-skill --agent codex\n  sb repo install my-skill --global"
    )]
    Install {
        /// Skill name
        skill: String,

        /// Version to install (default: latest)
        #[arg(long)]
        version: Option<String>,

        /// Installation directory (overrides agent detection)
        #[arg(long)]
        install_dir: Option<PathBuf>,

        /// Target agent framework: claude, opencode, codex, kiro, or all
        #[arg(long)]
        agent: Option<String>,

        /// Install to global agent directories instead of project-level
        #[arg(long)]
        global: bool,
    },

    /// Delete a skill from the repository
    #[command(
        long_about = "Delete a skill (or a specific version) from the S3 repository.\n\nRemoves the .skill file, changelog, and source archive from S3 and updates\nthe skills index. Also clears matching entries from the local cache.\nRequires --yes to confirm.",
        after_help = "Examples:\n  sb repo delete my-skill --yes\n  sb repo delete my-skill --version 1.0.0 --yes"
    )]
    Delete {
        /// Skill name
        skill: String,

        /// Specific version to delete (default: all versions)
        #[arg(long)]
        version: Option<String>,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// List skills in the repository
    #[command(
        long_about = "List all skills in the S3 repository.\n\nDisplays each skill's name, description, source URL, and available versions.\nOptionally filter to a single skill.",
        after_help = "Examples:\n  sb repo list\n  sb repo list --skill my-skill"
    )]
    List {
        /// Filter by skill name
        #[arg(long)]
        skill: Option<String>,
    },
}

#[derive(Subcommand)]
enum LocalAction {
    /// List all skills in the local repository
    #[command(
        long_about = "List all skills and versions stored in the local repository.\n\nShows each skill name and version, plus the repository directory path."
    )]
    List,

    /// Clear skills from the local repository
    #[command(
        long_about = "Clear skills from the local repository.\n\nRemoves all locally stored skills, or only a specific skill if --skill is provided.",
        after_help = "Examples:\n  sb local clear\n  sb local clear --skill my-skill"
    )]
    Clear {
        /// Only clear a specific skill (default: clear all)
        #[arg(long)]
        skill: Option<String>,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {:#}", e);
        process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let output = Output::new(cli.agent_output);

    match cli.command {
        Commands::Download {
            skill_name,
            all,
            url,
            name,
            source_dir,
        } => {
            // Handle --url override (no config needed)
            if let Some(url) = url {
                let name = name.context("--name is required when using --url")?;
                output.info(&format!("Downloading from URL: {}", url));
                output.step(&format!("Skill name: {}", name));
                output.newline();

                let results = download_from_url(&url, &name, &source_dir, &output)?;
                let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();

                if !failures.is_empty() {
                    anyhow::bail!("{} files failed to download", failures.len());
                }

                return Ok(());
            }

            // Load config
            let config = Config::load_with_fallback(cli.config.as_deref())?;

            if all {
                // Download all skills
                output.header(&format!(
                    "Downloading all {} skills...",
                    config.skills.len()
                ));
                output.newline();

                for skill in &config.skills {
                    output.header(&format!("=== {} ===", skill.name));
                    if let Err(e) = download_skill_docs(skill, &source_dir, &output) {
                        output.error(&format!("Failed to download {}: {}", skill.name, e));
                    }
                    output.newline();
                }
            } else if let Some(name) = skill_name {
                // Download specific skill
                let skill = config
                    .find_skill(&name)
                    .with_context(|| format!("Skill '{}' not found in config", name))?;

                let results = download_skill_docs(skill, &source_dir, &output)?;
                let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();

                if !failures.is_empty() {
                    anyhow::bail!("{} files failed to download", failures.len());
                }
            } else {
                anyhow::bail!("Please specify a skill name, --all, or --url with --name");
            }
        }

        Commands::Validate { skill, skills_dir } => {
            // Determine skill path
            let skill_path = if PathBuf::from(&skill).exists() {
                PathBuf::from(&skill)
            } else {
                skills_dir.join(&skill)
            };

            if !skill_path.exists() {
                anyhow::bail!("Skill directory not found: {}", skill_path.display());
            }

            output.info(&format!("Validating: {}", skill_path.display()));
            output.newline();

            let result = validate_skill(&skill_path);
            print_validation_result(&result, &output);

            if !result.valid {
                process::exit(1);
            }
        }

        Commands::Package {
            skill,
            output: output_dir,
            skills_dir,
        } => {
            // Determine skill path
            let skill_path = if PathBuf::from(&skill).exists() {
                PathBuf::from(&skill)
            } else {
                skills_dir.join(&skill)
            };

            if !skill_path.exists() {
                anyhow::bail!("Skill directory not found: {}", skill_path.display());
            }

            skill_builder::package::package_skill_with_output(&skill_path, &output_dir, &output)?;
        }

        Commands::Install {
            skill,
            version,
            repo,
            file,
            local,
            remote,
            github,
            install_dir,
            agent,
            global,
        } => {
            // Resolve target directories
            let agent_target = skill_builder::agent::parse_agent_flag(agent.as_deref())?;
            let install_dirs = skill_builder::agent::resolve_install_dirs(
                &agent_target,
                install_dir.as_deref(),
                global,
                std::path::Path::new("."),
            );

            if let Some(file_path) = file {
                // Install from local file to each target directory
                for dir in &install_dirs {
                    output.info(&format!("Installing to {}", dir.display()));
                    install_from_file(&file_path, dir, &output)?;
                }
            } else {
                // Use the install resolver for source cascade
                let config = Config::load_with_fallback(cli.config.as_deref())?;
                for dir in &install_dirs {
                    output.info(&format!("Installing to {}", dir.display()));
                    let options = skill_builder::install_resolver::InstallOptions {
                        skill_name: &skill,
                        version: version.as_deref(),
                        github_repo: repo.as_deref(),
                        install_dir: dir,
                        local_only: local,
                        remote_only: remote,
                        github_only: github,
                    };
                    skill_builder::install_resolver::resolve_and_install(
                        &config, &options, &output,
                    )?;
                }
            }
        }

        Commands::List => {
            let config = Config::load_with_fallback(cli.config.as_deref())?;

            if config.skills.is_empty() {
                output.info("No skills configured.");
            } else {
                output.header("Configured skills:");
                output.newline();
                let mut rows = Vec::new();
                for skill in &config.skills {
                    let desc = if skill.description.chars().count() > 60 {
                        let truncated: String = skill.description.chars().take(60).collect();
                        format!("{}...", truncated)
                    } else {
                        skill.description.clone()
                    };
                    rows.push(vec![skill.name.clone(), skill.llms_txt_url.clone(), desc]);
                }
                output.table(&rows);
            }
        }

        Commands::Repo { action } => {
            handle_repo_command(cli.config.as_deref(), action, &output)?;
        }

        Commands::Local { action } => {
            handle_local_command(cli.config.as_deref(), action, &output)?;
        }

        Commands::Init => {
            skill_builder::init::run_init(&output)?;
        }
    }

    Ok(())
}

fn handle_repo_command(
    config_path: Option<&std::path::Path>,
    action: RepoAction,
    output: &Output,
) -> Result<()> {
    let config = Config::load_with_fallback(config_path)?;
    let repo_config = config
        .repository
        .as_ref()
        .context("No 'repository' section found in config. Add one to use repo commands.")?;

    let repo = Repository::from_config(repo_config)?;

    match action {
        RepoAction::Upload {
            skill,
            version,
            file,
            changelog,
            source_dir,
        } => {
            let skill_file = if let Some(f) = file {
                f
            } else {
                // Default to dist/<skill>.skill
                PathBuf::from(format!("dist/{}.skill", skill))
            };

            if !skill_file.exists() {
                anyhow::bail!("Skill file not found: {}", skill_file.display());
            }

            // Look up skill info from config
            let skill_config = config.find_skill(&skill);
            let description = skill_config.map(|s| s.description.as_str()).unwrap_or("");
            let llms_txt_url = skill_config.map(|s| s.llms_txt_url.as_str()).unwrap_or("");

            output.header(&format!("Uploading {} v{}...", skill, version));
            repo.upload(
                &UploadParams {
                    name: &skill,
                    version: &version,
                    description,
                    llms_txt_url,
                    skill_file: &skill_file,
                    changelog: changelog.as_deref(),
                    source_dir: source_dir.as_deref(),
                },
                output,
            )?;
            output.status("Done", &format!("Uploaded {} v{}", skill, version));
        }

        RepoAction::Download {
            skill,
            version,
            output: output_dir,
        } => {
            let path = repo.download(&skill, version.as_deref(), output_dir.as_deref(), output)?;
            output.status("Downloaded", &format!("{}", path.display()));
        }

        RepoAction::Install {
            skill,
            version,
            install_dir,
            agent,
            global,
        } => {
            let agent_target = skill_builder::agent::parse_agent_flag(agent.as_deref())?;
            let install_dirs = skill_builder::agent::resolve_install_dirs(
                &agent_target,
                install_dir.as_deref(),
                global,
                std::path::Path::new("."),
            );

            for dir in &install_dirs {
                repo.install(&skill, version.as_deref(), dir, output)?;
            }
        }

        RepoAction::Delete {
            skill,
            version,
            yes,
        } => {
            if !yes {
                let target = if let Some(ref v) = version {
                    format!("{} v{}", skill, v)
                } else {
                    format!("{} (all versions)", skill)
                };
                output.warn(&format!(
                    "This will permanently delete {} from the repository. Use --yes to confirm.",
                    target
                ));
                process::exit(1);
            }

            output.header(&format!("Deleting {}...", skill));
            repo.delete(&skill, version.as_deref(), output)?;
            output.status("Done", &format!("Deleted {}", skill));
        }

        RepoAction::List { skill } => {
            let index = repo.list(skill.as_deref())?;

            if index.skills.is_empty() {
                output.info("No skills found in repository.");
            } else {
                output.header("Repository skills:");
                output.newline();
                for entry in &index.skills {
                    output.info(&format!("  {} - {}", entry.name, entry.description));
                    if !entry.llms_txt_url.is_empty() {
                        output.step(&format!("Source: {}", entry.llms_txt_url));
                    }
                    let mut versions: Vec<&str> =
                        entry.versions.keys().map(|s| s.as_str()).collect();
                    versions.sort();
                    versions.reverse();
                    output.step(&format!("Versions: {}", versions.join(", ")));
                }
            }
        }
    }

    Ok(())
}

fn handle_local_command(
    config_path: Option<&std::path::Path>,
    action: LocalAction,
    output: &Output,
) -> Result<()> {
    let config = Config::load_with_fallback(config_path)?;

    let local_path = config
        .repository
        .as_ref()
        .map(|r| r.local_repo_path())
        .unwrap_or_else(skill_builder::config::default_local_repo_path);

    let client = LocalStorageClient::with_dir(&local_path);

    match action {
        LocalAction::List => {
            let index = load_index(&client);
            match index {
                Ok(index) if !index.skills.is_empty() => {
                    output.header("Local repository skills:");
                    output.newline();
                    let mut rows = Vec::new();
                    for entry in &index.skills {
                        let mut versions: Vec<&str> =
                            entry.versions.keys().map(|s| s.as_str()).collect();
                        versions.sort();
                        versions.reverse();
                        for ver in &versions {
                            rows.push(vec![entry.name.clone(), format!("v{}", ver)]);
                        }
                    }
                    output.table(&rows);
                    output.newline();
                    output.info(&format!("Local repository: {}", local_path.display()));
                }
                _ => {
                    // Also list raw skill files if no index
                    let keys = client.list_objects("skills/").unwrap_or_default();
                    if keys.is_empty() {
                        output.info("No skills in local repository.");
                    } else {
                        output.header("Local repository skills:");
                        output.newline();
                        for key in &keys {
                            if key.ends_with(".skill") {
                                output.step(key);
                            }
                        }
                    }
                    output.info(&format!("Local repository: {}", local_path.display()));
                }
            }
        }

        LocalAction::Clear { skill } => {
            if let Some(name) = skill {
                let prefix = format!("skills/{}/", name);
                let keys = client.list_objects(&prefix).unwrap_or_default();
                for key in &keys {
                    if let Err(e) = client.delete_object(key) {
                        output.warn(&format!("Failed to delete {}: {}", key, e));
                    }
                }
                output.status("Cleared", &format!("local repository for {}", name));
            } else {
                let keys = client.list_objects("skills/").unwrap_or_default();
                for key in &keys {
                    if let Err(e) = client.delete_object(key) {
                        output.warn(&format!("Failed to delete {}: {}", key, e));
                    }
                }
                output.status("Cleared", "all skills from local repository");
            }
        }
    }

    Ok(())
}
