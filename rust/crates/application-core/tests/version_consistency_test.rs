use application_core::{AETHER_RELEASE_CHANNEL, AETHER_VERSION};

#[test]
fn test_version_consistency() {
    assert_eq!(AETHER_VERSION, "0.1.0-beta.2");
    assert_eq!(AETHER_RELEASE_CHANNEL, "beta.2");
}

#[test]
fn test_version_semantic_validity() {
    let parts: Vec<&str> = AETHER_VERSION.split('-').collect();
    assert_eq!(parts.len(), 2, "Expected version with prerelease tag");
    let semver_nums: Vec<&str> = parts[0].split('.').collect();
    assert_eq!(semver_nums.len(), 3, "Expected major.minor.patch format");
    assert_eq!(semver_nums[0], "0");
    assert_eq!(semver_nums[1], "1");
    assert_eq!(semver_nums[2], "0");
    assert_eq!(parts[1], "beta.2");
}
