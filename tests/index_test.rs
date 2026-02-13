//! Integration tests for the index module.

use skill_builder::index::{load_index, save_index, SkillsIndex};
use skill_builder::s3::mock::MockS3Client;

#[test]
fn test_load_empty_index() {
    let client = MockS3Client::new();
    let index = load_index(&client).unwrap();
    assert!(index.skills.is_empty());
}

#[test]
fn test_save_and_load_roundtrip() {
    let client = MockS3Client::new();

    let mut index = SkillsIndex::new();
    index.add_or_update_skill(
        "test-skill",
        "A test skill for round-trip testing",
        "https://example.com/llms.txt",
        "1.0.0",
        "skills/test-skill/1.0.0/test-skill.skill",
    );
    index.add_or_update_skill(
        "test-skill",
        "A test skill for round-trip testing",
        "https://example.com/llms.txt",
        "2.0.0",
        "skills/test-skill/2.0.0/test-skill.skill",
    );

    save_index(&client, &index).unwrap();
    let loaded = load_index(&client).unwrap();

    assert_eq!(loaded.skills.len(), 1);
    assert_eq!(loaded.skills[0].versions.len(), 2);
    assert_eq!(loaded.skills[0].name, "test-skill");
}

#[test]
fn test_multiple_skills_roundtrip() {
    let client = MockS3Client::new();

    let mut index = SkillsIndex::new();
    index.add_or_update_skill("alpha", "Alpha skill", "url-a", "1.0.0", "path-a");
    index.add_or_update_skill("beta", "Beta skill", "url-b", "1.0.0", "path-b");

    save_index(&client, &index).unwrap();
    let loaded = load_index(&client).unwrap();

    assert_eq!(loaded.skills.len(), 2);
    assert!(loaded.find_skill("alpha").is_some());
    assert!(loaded.find_skill("beta").is_some());
}

#[test]
fn test_index_latest_version() {
    let mut index = SkillsIndex::new();
    index.add_or_update_skill("s", "d", "u", "1.0.0", "p");
    index.add_or_update_skill("s", "d", "u", "3.0.0", "p");
    index.add_or_update_skill("s", "d", "u", "2.5.0", "p");

    assert_eq!(index.latest_version("s"), Some("3.0.0"));
}

#[test]
fn test_index_remove_and_save() {
    let client = MockS3Client::new();

    let mut index = SkillsIndex::new();
    index.add_or_update_skill("a", "d", "u", "1.0.0", "p");
    index.add_or_update_skill("b", "d", "u", "1.0.0", "p");
    save_index(&client, &index).unwrap();

    let mut loaded = load_index(&client).unwrap();
    loaded.remove_skill("a");
    save_index(&client, &loaded).unwrap();

    let reloaded = load_index(&client).unwrap();
    assert_eq!(reloaded.skills.len(), 1);
    assert_eq!(reloaded.skills[0].name, "b");
}
