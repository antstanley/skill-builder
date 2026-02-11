# skill-builder

A Rust CLI tool that builds [Claude Code](https://claude.ai/claude-code) skills from any `llms.txt` URL.

## Installation

### From Source

```bash
cargo install --path .
```

### From GitHub Releases

Download the latest binary from the [releases page](https://github.com/antstanley/skill-builder/releases).

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

## Configuration

The `skills.json` file defines available skills:

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
  ]
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Unique identifier for the skill |
| `description` | No | Description shown in listings |
| `llms_txt_url` | Yes | URL to the llms.txt file |
| `base_url` | No | Base URL for docs (auto-derived if null) |
| `path_prefix` | No | Path prefix to strip (auto-detected if null) |

## Project Structure

```
skill-builder/
├── Cargo.toml              # Rust project manifest
├── skills.json             # Skill configuration
├── skills/
│   └── shadcn-svelte/      # Skill source files
│       ├── SKILL.md        # Main skill instructions
│       └── references/     # Reference documentation
├── source/
│   └── shadcn-svelte/      # Downloaded upstream docs
│       ├── llms.txt        # Index with local paths
│       └── docs/           # Raw documentation
├── src/                    # Rust source code
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
```

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
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

1. Update `CHANGELOG.md` with the new version section
2. Commit the changelog: `git commit -m "docs: update changelog for vX.Y.Z"`
3. Create and push a version tag:
   ```bash
   git tag vX.Y.Z
   git push origin main
   git push origin vX.Y.Z
   ```

The release workflow will automatically:
- Run tests
- Build the binary
- Validate and package all skills
- Create a GitHub Release with artifacts

## License

MIT
