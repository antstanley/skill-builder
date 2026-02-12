# skill-builder

A Rust CLI tool that builds [Claude Code](https://claude.ai/claude-code) skills from any `llms.txt` URL.

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

1. **Add a skill to `skills.json`:**

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

2. **Download the documentation:**

```bash
skill-builder download my-library
```

3. **Create/update the skill in `skills/my-library/`** with a `SKILL.md` and `references/` directory.

4. **Validate and package:**

```bash
skill-builder validate my-library
skill-builder package my-library --output dist/
```

## CLI Reference

### Download Documentation

```bash
# Download docs for a skill defined in skills.json
skill-builder download shadcn-svelte

# Download all skills
skill-builder download --all

# Download from URL directly (no config needed)
skill-builder download --url https://example.com/llms.txt --name my-skill

# Specify source directory
skill-builder download shadcn-svelte --source-dir ./source
```

### Validate a Skill

```bash
# Validate by name (looks in skills/ directory)
skill-builder validate shadcn-svelte

# Validate by path
skill-builder validate ./skills/shadcn-svelte

# Specify skills directory
skill-builder validate my-skill --skills-dir ./custom-skills
```

### Package a Skill

```bash
# Package to dist/ directory
skill-builder package shadcn-svelte

# Specify output directory
skill-builder package shadcn-svelte --output ./releases
```

### Install a Skill

```bash
# Install latest version from GitHub releases
skill-builder install shadcn-svelte

# Install specific version
skill-builder install shadcn-svelte --version 1.0.0

# Install from local .skill file
skill-builder install shadcn-svelte --file ./dist/shadcn-svelte.skill

# Specify installation directory
skill-builder install shadcn-svelte --install-dir ~/.claude/skills
```

### List Configured Skills

```bash
skill-builder list
```

### Skill Repository (S3-Compatible)

Manage skills in an S3-compatible hosted repository with local caching.

```bash
# Upload a skill to the repository
skill-builder repo upload my-skill 1.0.0
skill-builder repo upload my-skill 1.0.0 --file ./dist/my-skill.skill --changelog CHANGELOG.md --source-dir ./source

# Download a skill from the repository
skill-builder repo download my-skill
skill-builder repo download my-skill --version 1.0.0 --output ./downloads

# Install a skill directly from the repository
skill-builder repo install my-skill
skill-builder repo install my-skill --version 1.0.0 --install-dir .claude/skills

# Delete a skill from the repository
skill-builder repo delete my-skill --yes
skill-builder repo delete my-skill --version 1.0.0 --yes

# List skills in the repository
skill-builder repo list
skill-builder repo list --skill my-skill
```

### Local Cache

Downloaded skills are cached locally for faster subsequent access.

```bash
# List cached skills
skill-builder cache list

# Clear all cached skills
skill-builder cache clear

# Clear cache for a specific skill
skill-builder cache clear --skill my-skill
```

Cache location:
- **Linux:** `~/.cache/skill-builder/skills/`
- **macOS:** `~/Library/Caches/skill-builder/skills/`

## Configuration

The `skills.json` file defines available skills and optional repository settings:

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

The `repository` section is optional. When present, it enables the `repo` subcommands.

| Field | Required | Description |
|-------|----------|-------------|
| `name` | No | Display name for the repository |
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
│   ├── config.rs           # Configuration parsing
│   ├── download.rs         # Document downloading
│   ├── validate.rs         # Skill validation
│   ├── package.rs          # Skill packaging
│   ├── install.rs          # Skill installation
│   ├── s3.rs               # S3-compatible storage client
│   ├── index.rs            # Skills index management
│   ├── cache.rs            # Local skill caching
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
