//! skill-builder CLI - Build Claude Code skills from llms.txt URLs.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use skill_builder::cache::SkillCache;
use skill_builder::config::Config;
use skill_builder::download::{download_from_url, download_skill_docs};
use skill_builder::install::{install_from_file, install_skill};
use skill_builder::package::package_skill;
use skill_builder::repository::Repository;
use skill_builder::s3::S3Client;
use skill_builder::validate::{print_validation_result, validate_skill};

/// Build Claude Code skills from llms.txt URLs.
#[derive(Parser)]
#[command(name = "skill-builder")]
#[command(
    version,
    about,
    long_about = "A CLI tool that builds Claude Code skills from any llms.txt URL.\n\nSkills are built by downloading documentation, validating the skill structure,\npackaging into distributable .skill files, and optionally publishing to an\nS3-compatible repository.\n\nConfigure skills in a skills.json file or use --url for ad-hoc downloads."
)]
#[command(
    after_help = "Examples:\n  skill-builder download my-skill\n  skill-builder validate my-skill\n  skill-builder package my-skill --output dist/\n  skill-builder install my-skill --version 1.0.0\n  skill-builder repo upload my-skill 1.0.0\n  skill-builder cache list"
)]
struct Cli {
    /// Path to skills.json configuration file
    #[arg(short, long, default_value = "skills.json")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Download documentation for a skill
    #[command(
        long_about = "Download documentation for a skill from its llms.txt URL.\n\nFetches the llms.txt index, extracts all linked .md files, and saves them\nlocally. Use a skill name from skills.json or provide a URL directly.",
        after_help = "Examples:\n  skill-builder download my-skill\n  skill-builder download --all\n  skill-builder download --url https://example.com/llms.txt --name my-skill\n  skill-builder download my-skill --source-dir ./docs"
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
        after_help = "Examples:\n  skill-builder validate my-skill\n  skill-builder validate ./path/to/skill\n  skill-builder validate my-skill --skills-dir ./custom-skills"
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
        after_help = "Examples:\n  skill-builder package my-skill\n  skill-builder package my-skill --output ./releases\n  skill-builder package ./path/to/skill --output dist/"
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

    /// Install a skill from GitHub releases or local file
    #[command(
        long_about = "Install a skill from GitHub releases or a local .skill file.\n\nDownloads the .skill archive from a GitHub release (or reads a local file),\nthen extracts it into the installation directory for use with Claude Code.",
        after_help = "Examples:\n  skill-builder install my-skill\n  skill-builder install my-skill --version 1.0.0\n  skill-builder install my-skill --file ./dist/my-skill.skill\n  skill-builder install my-skill --repo user/repo --install-dir ~/.claude/skills"
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

        /// Installation directory
        #[arg(long, default_value = ".claude/skills")]
        install_dir: PathBuf,
    },

    /// List all skills in configuration
    #[command(
        long_about = "List all skills defined in the skills.json configuration file.\n\nDisplays each skill's name, llms.txt URL, and description."
    )]
    List,

    /// Manage the S3-compatible skill repository
    #[command(
        long_about = "Manage skills in an S3-compatible hosted repository.\n\nRequires a 'repository' section in skills.json with bucket_name and optional\nregion/endpoint. Authentication uses the standard AWS credential chain\n(environment variables, ~/.aws/credentials, IAM roles).",
        after_help = "Examples:\n  skill-builder repo upload my-skill 1.0.0\n  skill-builder repo download my-skill --version 1.0.0\n  skill-builder repo install my-skill\n  skill-builder repo delete my-skill --yes\n  skill-builder repo list"
    )]
    Repo {
        #[command(subcommand)]
        action: RepoAction,
    },

    /// Manage the local skill cache
    #[command(
        long_about = "Manage the local skill cache.\n\nSkills downloaded from the repository are cached locally for faster access.\nCache is stored in the platform-appropriate cache directory:\n  Linux:  ~/.cache/skill-builder/skills/\n  macOS:  ~/Library/Caches/skill-builder/skills/",
        after_help = "Examples:\n  skill-builder cache list\n  skill-builder cache clear\n  skill-builder cache clear --skill my-skill"
    )]
    Cache {
        #[command(subcommand)]
        action: CacheAction,
    },
}

#[derive(Subcommand)]
enum RepoAction {
    /// Upload a skill to the repository
    #[command(
        long_about = "Upload a .skill file to the S3 repository.\n\nIf --file is not specified, defaults to dist/<skill>.skill. Skill metadata\n(description, llms_txt_url) is read from skills.json if available.\nOptionally include a CHANGELOG.md and/or archive the source directory.",
        after_help = "Examples:\n  skill-builder repo upload my-skill 1.0.0\n  skill-builder repo upload my-skill 1.0.0 --file ./my-skill.skill\n  skill-builder repo upload my-skill 1.0.0 --changelog CHANGELOG.md --source-dir ./source"
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
        after_help = "Examples:\n  skill-builder repo download my-skill\n  skill-builder repo download my-skill --version 1.0.0\n  skill-builder repo download my-skill --output ./downloads"
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
        after_help = "Examples:\n  skill-builder repo install my-skill\n  skill-builder repo install my-skill --version 1.0.0\n  skill-builder repo install my-skill --install-dir ~/.claude/skills"
    )]
    Install {
        /// Skill name
        skill: String,

        /// Version to install (default: latest)
        #[arg(long)]
        version: Option<String>,

        /// Installation directory
        #[arg(long, default_value = ".claude/skills")]
        install_dir: PathBuf,
    },

    /// Delete a skill from the repository
    #[command(
        long_about = "Delete a skill (or a specific version) from the S3 repository.\n\nRemoves the .skill file, changelog, and source archive from S3 and updates\nthe skills index. Also clears matching entries from the local cache.\nRequires --yes to confirm.",
        after_help = "Examples:\n  skill-builder repo delete my-skill --yes\n  skill-builder repo delete my-skill --version 1.0.0 --yes"
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
        after_help = "Examples:\n  skill-builder repo list\n  skill-builder repo list --skill my-skill"
    )]
    List {
        /// Filter by skill name
        #[arg(long)]
        skill: Option<String>,
    },
}

#[derive(Subcommand)]
enum CacheAction {
    /// List all cached skills
    #[command(
        long_about = "List all skills and versions stored in the local cache.\n\nShows each cached skill name and version, plus the cache directory path."
    )]
    List,

    /// Clear cached skills
    #[command(
        long_about = "Clear skills from the local cache.\n\nRemoves all cached skills, or only a specific skill if --skill is provided.",
        after_help = "Examples:\n  skill-builder cache clear\n  skill-builder cache clear --skill my-skill"
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
                println!("Downloading from URL: {}", url);
                println!("Skill name: {}", name);
                println!();

                let results = download_from_url(&url, &name, &source_dir)?;
                let failures: Vec<_> = results.iter().filter(|r| !r.success).collect();

                if !failures.is_empty() {
                    anyhow::bail!("{} files failed to download", failures.len());
                }

                return Ok(());
            }

            // Load config
            let config = Config::load(&cli.config)?;

            if all {
                // Download all skills
                println!("Downloading all {} skills...", config.skills.len());
                println!();

                for skill in &config.skills {
                    println!("=== {} ===", skill.name);
                    if let Err(e) = download_skill_docs(skill, &source_dir) {
                        eprintln!("Failed to download {}: {}", skill.name, e);
                    }
                    println!();
                }
            } else if let Some(name) = skill_name {
                // Download specific skill
                let skill = config
                    .find_skill(&name)
                    .with_context(|| format!("Skill '{}' not found in config", name))?;

                let results = download_skill_docs(skill, &source_dir)?;
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

            println!("Validating: {}", skill_path.display());
            println!();

            let result = validate_skill(&skill_path);
            print_validation_result(&result);

            if !result.valid {
                process::exit(1);
            }
        }

        Commands::Package {
            skill,
            output,
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

            package_skill(&skill_path, &output)?;
        }

        Commands::Install {
            skill,
            version,
            repo,
            file,
            install_dir,
        } => {
            if let Some(file_path) = file {
                // Install from local file
                install_from_file(&file_path, &install_dir)?;
            } else {
                // Install from GitHub releases
                install_skill(
                    &skill,
                    version.as_deref(),
                    repo.as_deref(),
                    Some(&install_dir),
                )?;
            }
        }

        Commands::List => {
            let config = Config::load(&cli.config)?;

            if config.skills.is_empty() {
                println!("No skills configured in {}", cli.config.display());
            } else {
                println!("Configured skills:");
                println!();
                for skill in &config.skills {
                    println!("  {} - {}", skill.name, skill.llms_txt_url);
                    if !skill.description.is_empty() {
                        println!(
                            "    {}",
                            if skill.description.len() > 60 {
                                format!("{}...", &skill.description[..60])
                            } else {
                                skill.description.clone()
                            }
                        );
                    }
                }
            }
        }

        Commands::Repo { action } => {
            handle_repo_command(&cli.config, action)?;
        }

        Commands::Cache { action } => {
            handle_cache_command(action)?;
        }
    }

    Ok(())
}

fn handle_repo_command(config_path: &PathBuf, action: RepoAction) -> Result<()> {
    let config = Config::load(config_path)?;
    let repo_config = config
        .repository
        .as_ref()
        .context("No 'repository' section found in config. Add one to use repo commands.")?;

    let client = S3Client::new(repo_config)?;
    let cache = SkillCache::new()?;
    let repo = Repository::new(client, cache);

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

            println!("Uploading {} v{}...", skill, version);
            repo.upload(
                &skill,
                &version,
                description,
                llms_txt_url,
                &skill_file,
                changelog.as_deref(),
                source_dir.as_deref(),
            )?;
            println!("Done!");
        }

        RepoAction::Download {
            skill,
            version,
            output,
        } => {
            let path = repo.download(&skill, version.as_deref(), output.as_deref())?;
            println!("Downloaded to: {}", path.display());
        }

        RepoAction::Install {
            skill,
            version,
            install_dir,
        } => {
            repo.install(&skill, version.as_deref(), &install_dir)?;
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
                eprintln!(
                    "Warning: This will permanently delete {} from the repository.",
                    target
                );
                eprintln!("Use --yes to confirm.");
                process::exit(1);
            }

            println!("Deleting {}...", skill);
            repo.delete(&skill, version.as_deref())?;
            println!("Done!");
        }

        RepoAction::List { skill } => {
            let index = repo.list(skill.as_deref())?;

            if index.skills.is_empty() {
                println!("No skills found in repository.");
            } else {
                println!("Repository skills:");
                println!();
                for entry in &index.skills {
                    println!("  {} - {}", entry.name, entry.description);
                    if !entry.llms_txt_url.is_empty() {
                        println!("    Source: {}", entry.llms_txt_url);
                    }
                    let mut versions: Vec<&str> =
                        entry.versions.keys().map(|s| s.as_str()).collect();
                    versions.sort();
                    versions.reverse();
                    println!("    Versions: {}", versions.join(", "));
                }
            }
        }
    }

    Ok(())
}

fn handle_cache_command(action: CacheAction) -> Result<()> {
    let cache = SkillCache::new()?;

    match action {
        CacheAction::List => {
            let entries = cache.list_cached()?;

            if entries.is_empty() {
                println!("No cached skills.");
                println!("Cache directory: {}", cache.cache_dir().display());
            } else {
                println!("Cached skills:");
                println!();
                for (name, version) in &entries {
                    println!("  {} v{}", name, version);
                }
                println!();
                println!("Cache directory: {}", cache.cache_dir().display());
            }
        }

        CacheAction::Clear { skill } => {
            if let Some(name) = skill {
                cache.remove_all(&name)?;
                println!("Cleared cache for {}", name);
            } else {
                let entries = cache.list_cached()?;
                for (name, version) in &entries {
                    cache.remove(name, version)?;
                }
                println!("Cleared all cached skills.");
            }
        }
    }

    Ok(())
}
