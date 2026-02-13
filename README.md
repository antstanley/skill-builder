# skill-builder

A Rust CLI tool that builds [Agent Skills](https://agentskills.io) from any `llms.txt` URL. Skills work across Claude Code, OpenCode, Codex, and Kiro — the tool auto-detects which agents are configured and installs to all of them.

## Installation

### Quick Install (Linux/macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/antstanley/skill-builder/main/install.sh | sh
```

The install script auto-detects your OS and architecture and downloads the correct binary.

| Environment Variable | Description |
|---------------------|-------------|
| `SKILL_BUILDER_INSTALL_DIR` | Override install directory |
| `SKILL_BUILDER_VERSION` | Install a specific version (e.g. `v1.0.0`) |

### From Source

```bash
cargo install --path .
```

### From GitHub Releases

Download the latest binary for your platform from the [releases page](https://github.com/antstanley/skill-builder/releases).

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `skill-builder-x86_64-linux-gnu.tar.gz` |
| Linux ARM64 | `skill-builder-aarch64-linux-gnu.tar.gz` |
| macOS x86_64 | `skill-builder-x86_64-apple-darwin.tar.gz` |
| macOS ARM64 | `skill-builder-aarch64-apple-darwin.tar.gz` |
| Windows x86_64 | `skill-builder-x86_64-pc-windows-msvc.zip` |

## Quick Start

1. **Initialize global config:**

```bash
sb init
```

2. **Add a skill to `skills.json`:**

```json
{
  "skills": [
    {
      "name": "my-library",
      "description": "Expert guidance for my-library...",
      "llms_txt_url": "https://my-library.dev/llms.txt"
    }
  ]
}
```

3. **Download the documentation:**

```bash
sb download my-library
```

4. **Create/update the skill in `skills/my-library/`** with a `SKILL.md` and `references/` directory.

5. **Validate, package, and install:**

```bash
sb validate my-library
sb package my-library --output dist/
sb install my-library --file dist/my-library.skill
```

## Multi-Agent Support

Agent Skills is an open standard with identical `SKILL.md` format across multiple coding agents. `sb` auto-detects which agents are configured in your project and installs skills to all of them.

### Supported Agents

| Agent | Detection Markers | Project Install Dir | Global Install Dir |
|-------|-------------------|--------------------|--------------------|
| Claude Code | `.claude/` dir or `CLAUDE.md` | `.claude/skills/` | `~/.claude/skills/` |
| OpenCode | `.opencode/` dir or `opencode.json` | `.opencode/skills/` | `~/.config/opencode/skills/` |
| Codex | `.codex/` dir or `AGENTS.md` | `.agents/skills/` | `~/.codex/skills/` |
| Kiro | `.kiro/` dir | `.kiro/skills/` | `~/.kiro/skills/` |

If no agent markers are found, defaults to Claude Code.

### Targeting Agents

```bash
# Auto-detect agents and install to all detected (default)
sb install my-skill --file dist/my-skill.skill

# Install to a specific agent only
sb install my-skill --file dist/my-skill.skill --agent codex

# Install to all three agents regardless of detection
sb install my-skill --file dist/my-skill.skill --agent all

# Install to global agent directories
sb install my-skill --file dist/my-skill.skill --global

# Override with explicit directory (bypasses agent detection)
sb install my-skill --file dist/my-skill.skill --install-dir ./custom/path
```

The `--agent` and `--global` flags also work on `sb repo install`.

## Agent Output Mode

For consumption by AI agents and automation pipelines, `sb` supports a structured plain-text output mode with prefixed lines:

```bash
# Enable via flag
sb --agent-output validate my-skill

# Enable via environment variable
SB_AGENT_OUTPUT=1 sb validate my-skill
```

| Prefix | Meaning |
|--------|---------|
| `[OK]` | Success status |
| `[INFO]` | Informational message |
| `[STEP]` | Progress step |
| `[WARN]` | Warning |
| `[ERROR]` | Error |

In human mode (the default), output uses colors, spinners, and progress bars. Colors are disabled automatically when piped or when `NO_COLOR` is set.

## CLI Reference

### Initialize Global Config

```bash
sb init
```

Creates a configuration file at `$HOME/.skill-builder/skills.config.json` with options for setting up a local skill repository.

### Download Documentation

```bash
# Download docs for a skill defined in skills.json
sb download shadcn-svelte

# Download all skills
sb download --all

# Download from URL directly (no config needed)
sb download --url https://example.com/llms.txt --name my-skill

# Specify source directory
sb download shadcn-svelte --source-dir ./source
```

### Validate a Skill

```bash
# Validate by name (looks in skills/ directory)
sb validate shadcn-svelte

# Validate by path
sb validate ./skills/shadcn-svelte

# Specify skills directory
sb validate my-skill --skills-dir ./custom-skills
```

### Package a Skill

```bash
# Package to dist/ directory
sb package shadcn-svelte

# Specify output directory
sb package shadcn-svelte --output ./releases
```

### Install a Skill

By default, `sb install` searches local repo, remote repo, then GitHub releases in order. Skills are installed to all detected agent directories.

```bash
# Install (cascades: local -> remote -> GitHub)
sb install shadcn-svelte

# Install specific version
sb install shadcn-svelte --version 1.0.0

# Install from local repository only
sb install shadcn-svelte --local

# Install from remote S3 repository only
sb install shadcn-svelte --remote

# Install from GitHub releases only
sb install shadcn-svelte --github
sb install shadcn-svelte --github --repo user/repo

# Install from local .skill file
sb install shadcn-svelte --file ./dist/shadcn-svelte.skill

# Target a specific agent
sb install shadcn-svelte --agent codex

# Install to all agent directories
sb install shadcn-svelte --agent all

# Install globally
sb install shadcn-svelte --global

# Override installation directory
sb install shadcn-svelte --install-dir ~/.claude/skills
```

### List Configured Skills

```bash
sb list
```

### Skill Repository (S3-Compatible)

Manage skills in an S3-compatible hosted repository with local caching.

```bash
# Upload a skill to the repository
sb repo upload my-skill 1.0.0
sb repo upload my-skill 1.0.0 --file ./dist/my-skill.skill --changelog CHANGELOG.md --source-dir ./source

# Download a skill from the repository
sb repo download my-skill
sb repo download my-skill --version 1.0.0 --output ./downloads

# Install a skill directly from the repository
sb repo install my-skill
sb repo install my-skill --version 1.0.0
sb repo install my-skill --agent codex --global

# Delete a skill from the repository
sb repo delete my-skill --yes
sb repo delete my-skill --version 1.0.0 --yes

# List skills in the repository
sb repo list
sb repo list --skill my-skill
```

### Local Repository

Skills can be stored locally for offline access or as a cache for the remote repository.

```bash
# List locally stored skills
sb local list

# Clear all locally stored skills
sb local clear

# Clear a specific skill
sb local clear --skill my-skill
```

Default local repository location: `$HOME/.skill-builder/local/`

### Global Flags

| Flag | Description |
|------|-------------|
| `--config <path>` | Path to skills configuration file |
| `--agent-output` | Output plain text with prefixed lines for agent consumption |

## Configuration

### Config Fallback Hierarchy

`sb` loads configuration in this order (first found wins):

1. CLI `--config` flag (explicit path)
2. Project-local `skills.json` (if exists in current directory)
3. Global config at `$HOME/.skill-builder/skills.config.json`
4. Built-in defaults (empty config)

### Config Format

```json
{
  "skills": [
    {
      "name": "shadcn-svelte",
      "description": "Expert guidance for shadcn-svelte...",
      "llms_txt_url": "https://www.shadcn-svelte.com/llms.txt",
      "base_url": null,
      "path_prefix": null
    }
  ],
  "repository": {
    "name": "my-skill-repo",
    "local": {
      "path": null,
      "cache": false
    },
    "bucket_name": "my-skills-bucket",
    "region": "us-east-1",
    "endpoint": "https://s3.example.com"
  }
}
```

### Skill Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique identifier for the skill |
| `description` | No | Description shown in listings |
| `llms_txt_url` | Yes | URL to the llms.txt file |
| `base_url` | No | Base URL for docs (auto-derived if null) |
| `path_prefix` | No | Path prefix to strip (auto-detected if null) |

### Repository Fields

The `repository` section is optional. When present, it enables the `repo` and `local` subcommands.

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Display name for the repository |
| `local.path` | No | Local repository path (default: `$HOME/.skill-builder/local/`) |
| `local.cache` | No | Use local repo as cache for remote (default: `false`) |
| `bucket_name` | Yes (for repo commands) | S3 bucket name |
| `region` | No | AWS region (default: `us-east-1`) |
| `endpoint` | No | Custom endpoint for S3-compatible providers (MinIO, R2, etc.) |

Authentication uses the standard AWS credential chain (environment variables, `~/.aws/credentials`, IAM roles).

### Repository S3 Bucket Layout

```
<bucket>/
  skills_index.json
  skills/<skill_name>/<version>/
    <skill_name>.skill
    CHANGELOG.md
  source/<skill_name>/<version>/
    <skill_name>-source.zip
```

## Project Structure

```
skill-builder/
├── Cargo.toml              # Rust project manifest
├── install.sh              # Cross-platform install script
├── skills.json             # Skill configuration
├── skills/
│   └── shadcn-svelte/      # Skill source files
│       ├── SKILL.md        # Main skill instructions
│       └── references/     # Reference documentation
├── source/
│   └── shadcn-svelte/      # Downloaded upstream docs
│       ├── llms.txt        # Index with local paths
│       └── docs/           # Raw documentation
├── src/
│   ├── main.rs             # CLI entry point
│   ├── lib.rs              # Library root
│   ├── agent.rs            # Agent framework detection (Claude, OpenCode, Codex)
│   ├── config.rs           # Configuration parsing with fallback
│   ├── download.rs         # Document downloading
│   ├── validate.rs         # Skill validation
│   ├── package.rs          # Skill packaging
│   ├── install.rs          # Skill installation (GitHub)
│   ├── install_resolver.rs # Multi-source install resolution
│   ├── init.rs             # Interactive init command
│   ├── output.rs           # Output abstraction (human/agent modes)
│   ├── s3.rs               # S3-compatible storage client
│   ├── storage.rs          # StorageOperations trait
│   ├── local_storage.rs    # Filesystem storage backend
│   ├── index.rs            # Skills index management
│   └── repository.rs       # Repository operations
└── tests/                  # Integration tests
```

## Development

### Prerequisites

- Rust 1.70+
- [cargo-nextest](https://nexte.st/) (recommended for running tests)

### Running Tests

```bash
# Install nextest
cargo install cargo-nextest

# Run all tests
cargo nextest run

# Run specific test file
cargo nextest run config_test

# Run tests matching pattern
cargo nextest run --filter-expr 'test(validate)'

# Run with CI profile (retries, fail-fast)
cargo nextest run --profile ci

# Run only unit tests
cargo nextest run --lib

# Run only integration tests
cargo nextest run --test '*'

# Run install script tests
bash tests/install_script_test.sh
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized with LTO)
cargo build --release
```

### Linting

```bash
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

## Included Skills

### shadcn-svelte

A skill for [shadcn-svelte](https://shadcn-svelte.com) - a Svelte 5 port of shadcn/ui with beautifully designed, accessible components.

**Triggers on:**
- shadcn-svelte components
- Bits UI integration
- SvelteKit UI component setup
- Forms with Superforms/Formsnap
- Dark mode with mode-watcher
- Tailwind CSS v4 theming

## Release Process

1. Create a `release-vX.Y.Z` branch with your changes
2. Update `CHANGELOG.md` with the new version section
3. Tag the release: `git tag vX.Y.Z`
4. Push the branch and tag, then open a PR to `main`
5. Merge the PR to trigger the release workflow

The release workflow will automatically:
- Build cross-platform binaries (Linux x86_64/ARM64, macOS x86_64/ARM64, Windows x86_64)
- Run tests and shellcheck
- Validate and package all configured skills
- Create a GitHub Release with all artifacts and the install script

You can also trigger a release manually via `workflow_dispatch` with a tag input.

## License

MIT
