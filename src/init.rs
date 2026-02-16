//! Interactive initialization for global config.

use anyhow::{Context, Result};
use dialoguer::Confirm;
use std::fs;

use crate::config::{
    global_config_dir, global_config_path, Config, LocalRepositoryConfig, RepositoryConfig,
};
use crate::output::Output;

/// Run the interactive init command.
///
/// # Errors
///
/// Returns an error if the config directory or file cannot be created.
pub fn run_init(output: &Output) -> Result<()> {
    let config_path = global_config_path();
    let config_dir = global_config_dir();

    // Check if config already exists
    if config_path.exists() {
        if output.is_agent_mode() {
            // In agent mode, skip interactive prompts — overwrite by default
            output.info(&format!(
                "Overwriting existing config at {}",
                config_path.display()
            ));
        } else {
            let overwrite = Confirm::new()
                .with_prompt(format!(
                    "Config already exists at {}. Overwrite?",
                    config_path.display()
                ))
                .default(false)
                .interact()
                .context("Failed to read input")?;

            if !overwrite {
                output.info("Cancelled.");
                return Ok(());
            }
        }
    }

    // Create config directory
    fs::create_dir_all(&config_dir)
        .with_context(|| format!("Failed to create directory: {}", config_dir.display()))?;

    output.info(&format!(
        "Creating global config at {}",
        config_path.display()
    ));
    output.newline();

    // Ask about local repository
    let setup_local = if output.is_agent_mode() {
        true // Default to yes in agent mode
    } else {
        Confirm::new()
            .with_prompt("Set up a local skill repository?")
            .default(true)
            .interact()
            .context("Failed to read input")?
    };

    let mut config = Config::default();

    if setup_local {
        let default_path = config_dir.join("local");
        output.step(&format!(
            "Local repository path: {}",
            default_path.display()
        ));

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

    output.newline();
    output.status("Created", &format!("{}", config_path.display()));
    output.newline();
    output.header("Next steps:");
    output.step("1. Add skills to the config or a project-level skills.json");
    output.step("2. Run 'sb download <skill>' to fetch documentation");
    output.step("3. Run 'sb --help' for all commands");

    Ok(())
}
