//! Validate skill structure and SKILL.md frontmatter.

use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::Path;

/// Validation error types.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    SkillMdNotFound,
    MissingFrontmatter,
    InvalidYaml(String),
    EmptyFrontmatter,
    MissingName,
    EmptyName,
    MissingDescription,
    EmptyDescription,
    DescriptionTooShort(usize),
    UnresolvedTodo,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SkillMdNotFound => write!(f, "SKILL.md not found"),
            Self::MissingFrontmatter => {
                write!(f, "SKILL.md missing YAML frontmatter (must start with ---)")
            }
            Self::InvalidYaml(msg) => write!(f, "Invalid YAML frontmatter: {}", msg),
            Self::EmptyFrontmatter => write!(f, "Frontmatter is empty"),
            Self::MissingName => write!(f, "Frontmatter missing 'name' field"),
            Self::EmptyName => write!(f, "Frontmatter 'name' field is empty"),
            Self::MissingDescription => write!(f, "Frontmatter missing 'description' field"),
            Self::EmptyDescription => write!(f, "Frontmatter 'description' field is empty"),
            Self::DescriptionTooShort(len) => write!(
                f,
                "Frontmatter 'description' should be at least 50 characters (got {})",
                len
            ),
            Self::UnresolvedTodo => write!(f, "SKILL.md contains unresolved [TODO] placeholders"),
        }
    }
}

/// Result of skill validation.
#[derive(Debug)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn add_error(&mut self, error: ValidationError) {
        self.valid = false;
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// Parsed frontmatter from SKILL.md.
#[derive(Debug, Default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

/// Parse YAML frontmatter from markdown content.
fn parse_frontmatter(content: &str) -> Result<Frontmatter, ValidationError> {
    // Use (?s) for DOTALL mode so . matches newlines
    let re = Regex::new(r"(?s)^---\n(.*?)\n---").unwrap();

    let captures = re
        .captures(content)
        .ok_or(ValidationError::MissingFrontmatter)?;

    let yaml_content = captures.get(1).unwrap().as_str();

    if yaml_content.trim().is_empty() {
        return Err(ValidationError::EmptyFrontmatter);
    }

    let mut frontmatter = Frontmatter::default();

    // Simple YAML parsing for name and description fields
    for line in yaml_content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("name:") {
            frontmatter.name = Some(value.trim().to_string());
        } else if let Some(value) = line.strip_prefix("description:") {
            frontmatter.description = Some(value.trim().to_string());
        }
    }

    Ok(frontmatter)
}

/// Validate a skill directory.
pub fn validate_skill<P: AsRef<Path>>(skill_path: P) -> ValidationResult {
    let skill_path = skill_path.as_ref();
    let mut result = ValidationResult::new();

    // Check SKILL.md exists
    let skill_md_path = skill_path.join("SKILL.md");
    if !skill_md_path.exists() {
        result.add_error(ValidationError::SkillMdNotFound);
        return result;
    }

    // Read content
    let content = match fs::read_to_string(&skill_md_path) {
        Ok(c) => c,
        Err(e) => {
            result.add_error(ValidationError::InvalidYaml(format!(
                "Failed to read SKILL.md: {}",
                e
            )));
            return result;
        }
    };

    // Parse frontmatter
    let frontmatter = match parse_frontmatter(&content) {
        Ok(fm) => fm,
        Err(e) => {
            result.add_error(e);
            return result;
        }
    };

    // Validate name
    match &frontmatter.name {
        None => result.add_error(ValidationError::MissingName),
        Some(name) if name.is_empty() => result.add_error(ValidationError::EmptyName),
        Some(_) => {}
    }

    // Validate description
    match &frontmatter.description {
        None => result.add_error(ValidationError::MissingDescription),
        Some(desc) if desc.is_empty() => result.add_error(ValidationError::EmptyDescription),
        Some(desc) if desc.len() < 50 => {
            result.add_error(ValidationError::DescriptionTooShort(desc.len()))
        }
        Some(_) => {}
    }

    // Check for TODO placeholders
    if content.contains("[TODO") {
        result.add_error(ValidationError::UnresolvedTodo);
    }

    // Check for references directory (warning only)
    let references_path = skill_path.join("references");
    if !references_path.exists() {
        result.add_warning("No references directory found".to_string());
    } else if references_path.is_dir() {
        // Check if references directory is empty
        if let Ok(entries) = fs::read_dir(&references_path) {
            if entries.count() == 0 {
                result.add_warning("References directory is empty".to_string());
            }
        }
    }

    result
}

/// Print validation result using the Output abstraction.
pub fn print_validation_result(result: &ValidationResult, output: &crate::output::Output) {
    if result.valid {
        output.status("Valid", "Skill is valid!");
        for warning in &result.warnings {
            output.warn(warning);
        }
    } else {
        output.error("Validation failed:");
        for error in &result.errors {
            output.step(&format!("- {}", error));
        }
        for warning in &result.warnings {
            output.warn(warning);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_skill(dir: &Path, skill_md_content: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("SKILL.md"), skill_md_content).unwrap();
    }

    #[test]
    fn test_valid_skill() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("test-skill");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
description: A test skill with a description that is at least fifty characters long
---

# Test Skill

Content here.
"#,
        );

        // Create references directory
        fs::create_dir_all(skill_dir.join("references")).unwrap();
        fs::write(skill_dir.join("references/example.md"), "# Example").unwrap();

        let result = validate_skill(&skill_dir);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_missing_skill_md() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("empty-skill");
        fs::create_dir_all(&skill_dir).unwrap();

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::SkillMdNotFound));
    }

    #[test]
    fn test_missing_frontmatter() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("no-frontmatter");

        create_test_skill(
            &skill_dir,
            r#"# Test Skill

No frontmatter here.
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::MissingFrontmatter));
    }

    #[test]
    fn test_empty_frontmatter() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("empty-frontmatter");

        create_test_skill(
            &skill_dir,
            r#"---

---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::EmptyFrontmatter));
    }

    #[test]
    fn test_missing_name() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("missing-name");

        create_test_skill(
            &skill_dir,
            r#"---
description: A test skill with a description that is at least fifty characters long
---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::MissingName));
    }

    #[test]
    fn test_empty_name() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("empty-name");

        create_test_skill(
            &skill_dir,
            r#"---
name:
description: A test skill with a description that is at least fifty characters long
---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::EmptyName));
    }

    #[test]
    fn test_missing_description() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("missing-desc");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::MissingDescription));
    }

    #[test]
    fn test_description_too_short() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("short-desc");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
description: Too short
---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|e| matches!(e, ValidationError::DescriptionTooShort(_))));
    }

    #[test]
    fn test_unresolved_todo() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("with-todo");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
description: A test skill with a description that is at least fifty characters long
---

# Test Skill

[TODO: Fill this in]
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(!result.valid);
        assert!(result.errors.contains(&ValidationError::UnresolvedTodo));
    }

    #[test]
    fn test_warning_no_references() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("no-refs");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
description: A test skill with a description that is at least fifty characters long
---

# Test Skill
"#,
        );

        let result = validate_skill(&skill_dir);
        assert!(result.valid);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("No references directory")));
    }

    #[test]
    fn test_warning_empty_references() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("empty-refs");

        create_test_skill(
            &skill_dir,
            r#"---
name: test-skill
description: A test skill with a description that is at least fifty characters long
---

# Test Skill
"#,
        );

        fs::create_dir_all(skill_dir.join("references")).unwrap();

        let result = validate_skill(&skill_dir);
        assert!(result.valid);
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("References directory is empty")));
    }

    #[test]
    fn test_validation_error_display() {
        assert_eq!(
            ValidationError::SkillMdNotFound.to_string(),
            "SKILL.md not found"
        );
        assert_eq!(
            ValidationError::DescriptionTooShort(10).to_string(),
            "Frontmatter 'description' should be at least 50 characters (got 10)"
        );
    }
}
