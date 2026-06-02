//! Integration tests for `tokenless-core`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use tokenless_core::{Config, SafePath};

#[test]
fn test_config_valid_name() {
    let config = Config::new("my-app").unwrap();
    assert_eq!(config.name(), "my-app");
}

#[test]
fn test_config_empty_name() {
    let result = Config::new("");
    assert!(result.is_err());
}

#[test]
fn test_config_whitespace_name() {
    let result = Config::new("   ");
    assert!(result.is_err());
}

#[test]
fn test_safe_path_valid() {
    let path = SafePath::new("foo/bar/baz.txt").unwrap();
    assert_eq!(path.as_path().to_string_lossy(), "foo/bar/baz.txt");
}

#[test]
fn test_safe_path_rejects_absolute() {
    // Use a platform-appropriate absolute path.
    #[cfg(unix)]
    assert!(SafePath::new("/etc/passwd").is_err());
    #[cfg(windows)]
    assert!(SafePath::new("C:\\Windows\\System32").is_err());
}

#[test]
fn test_safe_path_rejects_parent_dir() {
    assert!(SafePath::new("../secret").is_err());
    assert!(SafePath::new("foo/../../etc/passwd").is_err());
}

#[test]
fn test_safe_path_rejects_null_byte() {
    assert!(SafePath::new("foo\0bar").is_err());
}
