//! Interactive initialization for global config.

use anyhow::{Context, Result};
use dialoguer::Confirm;
use std::fs;

use crate::config::{
    global_config_dir, global_config_path, Config, LocalRepositoryConfig, RepositoryConfig,
};

/// Run the interactive init command.
pub fn run_init() -> Result<()> {
    let config_path = global_config_path();
    let config_dir = global_config_dir();

    // Check if config already exists
    if config_path.exists() {
        let overwrite = Confirm::new()
            .with_prompt(format!(
                "Config already exists at {}. Overwrite?",
                config_path.display()
            ))
            .default(false)
            .interact()
            .context("Failed to read input")?;

        if !overwrite {
            println!("Cancelled.");
            return Ok(());
        }
    }

    // Create config directory
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create directory: {}", config_dir.display()))?;

    println!("Creating global config at {}", config_path.display());
    println!();

    // Ask about local repository
    let setup_local = Confirm::new()
        .with_prompt("Set up a local skill repository?")
        .default(true)
        .interact()
        .context("Failed to read input")?;

    let mut config = Config::default();

    if setup_local {
        let default_path = config_dir.join("local");
        println!("  Local repository path: {}", default_path.display());

        // Create the local repo directory
        fs::create_dir_all(&default_path)
            .with_context(|| format!("Failed to create directory: {}", default_path.display()))?;

        config.repository = Some(RepositoryConfig {
            name: None,
            local: Some(LocalRepositoryConfig {
                path: None, // use default
                cache: false,
            }),
            bucket_name: None,
            region: "us-east-1".to_string(),
            endpoint: None,
        });
    }

    // Write config
    let json = serde_json::to_string_pretty(&config).context("Failed to serialize config")?;
    fs::write(&config_path, &json)
        .with_context(|| format!("Failed to write config: {}", config_path.display()))?;

    println!();
    println!("Created {}", config_path.display());
    println!();
    println!("Next steps:");
    println!("  1. Add skills to the config or a project-level skills.json");
    println!("  2. Run 'sb download <skill>' to fetch documentation");
    println!("  3. Run 'sb --help' for all commands");

    Ok(())
}
