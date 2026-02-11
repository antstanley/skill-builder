//! skill-builder CLI - Build Claude Code skills from llms.txt URLs.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;

use skill_builder::config::Config;
use skill_builder::download::{download_from_url, download_skill_docs};
use skill_builder::install::{install_from_file, install_skill};
use skill_builder::package::package_skill;
use skill_builder::validate::{print_validation_result, validate_skill};

/// Build Claude Code skills from llms.txt URLs.
#[derive(Parser)]
#[command(name = "skill-builder")]
#[command(version, about, long_about = None)]
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
    Validate {
        /// Name of the skill to validate, or path to skill directory
        skill: String,

        /// Directory containing skills
        #[arg(long, default_value = "skills")]
        skills_dir: PathBuf,
    },

    /// Package a skill into a distributable .skill file
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
    List,
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
                anyhow::bail!(
                    "Please specify a skill name, --all, or --url with --name"
                );
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
                install_skill(&skill, version.as_deref(), repo.as_deref(), Some(&install_dir))?;
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
    }

    Ok(())
}
