//! # Permissions Module
//!
//! This module provides a centralized and clear way, preventing security-sensitive logic from being
//! scattered across the codebase.

use anyhow::{Result, anyhow};
use std::path::Path;

/// Checks if a given file path is within the list of accessible paths.
///
/// This function is crucial for sandboxing the agent's file system access. It
/// handles two cases:
/// 1. If the path exists, it checks if the path itself is within an accessible root.
/// 2. If the path does not exist (e.g., for file creation), it checks if the
///    parent directory is within an accessible root.
///
/// # Arguments
/// * `path_to_check` - The path to validate.
/// * `accessible_paths` - A slice of root paths that are permitted for operations.
///
/// # Returns
/// * `Ok(())` if the path is accessible.
/// * `Err(anyhow::Error)` if the path is not accessible, cannot be canonicalized,
///   or does not have a parent directory (for non-existent paths).
pub fn is_path_accessible(path_to_check: &Path, accessible_paths: &[String]) -> Result<()> {
    let path_to_canonicalize = if path_to_check.exists() {
        path_to_check.to_path_buf()
    } else {
        let parent = path_to_check.parent().ok_or_else(|| {
            anyhow!(
                "Cannot check accessibility for '{}' because it has no parent directory.",
                path_to_check.display()
            )
        })?;
        // If the parent is empty, it means the path is relative to the current directory.
        if parent.as_os_str().is_empty() {
            Path::new(".").to_path_buf()
        } else {
            parent.to_path_buf()
        }
    };

    let canonical_path = match path_to_canonicalize.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Err(anyhow!(
                "Failed to resolve path '{}': {}. It might not exist or there's a permission issue.",
                path_to_canonicalize.display(),
                e
            ));
        }
    };

    let is_allowed = accessible_paths.iter().any(|p| {
        if let Ok(canonical_accessible_path) = Path::new(p).canonicalize() {
            canonical_path.starts_with(canonical_accessible_path)
        } else {
            false
        }
    });

    if !is_allowed {
        return Err(anyhow!(
            "Operation on path '{}' is not allowed. It's not within any of the accessible paths: {:?}.",
            path_to_check.display(),
            accessible_paths
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::Builder;

    // Helper to set up a temporary directory structure for tests.
    fn setup_test_dirs() -> (tempfile::TempDir, String, String) {
        let tmp_dir = Builder::new().prefix("perm-test-").tempdir().unwrap();
        let accessible_dir = tmp_dir.path().join("accessible");
        let inaccessible_dir = tmp_dir.path().join("inaccessible");

        fs::create_dir_all(&accessible_dir).unwrap();
        fs::create_dir_all(&inaccessible_dir).unwrap();

        fs::write(accessible_dir.join("file.txt"), "content").unwrap();
        fs::write(inaccessible_dir.join("secret.txt"), "secret").unwrap();

        (
            tmp_dir,
            accessible_dir.to_str().unwrap().to_string(),
            inaccessible_dir.to_str().unwrap().to_string(),
        )
    }

    #[test]
    fn test_existing_file_in_accessible_path() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();
        let path_to_check = Path::new(&accessible).join("file.txt");
        let accessible_paths = vec![accessible];

        assert!(is_path_accessible(&path_to_check, &accessible_paths).is_ok());
    }

    #[test]
    fn test_existing_file_in_inaccessible_path() {
        let (_tmp_dir, accessible, inaccessible) = setup_test_dirs();
        let path_to_check = Path::new(&inaccessible).join("secret.txt");
        let accessible_paths = vec![accessible];

        let result = is_path_accessible(&path_to_check, &accessible_paths);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is not allowed"));
    }

    #[test]
    fn test_new_file_in_accessible_path() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();
        let path_to_check = Path::new(&accessible).join("new_file.txt");
        let accessible_paths = vec![accessible];

        assert!(is_path_accessible(&path_to_check, &accessible_paths).is_ok());
    }

    #[test]
    fn test_new_file_in_inaccessible_path() {
        let (_tmp_dir, accessible, inaccessible) = setup_test_dirs();
        let path_to_check = Path::new(&inaccessible).join("new_secret.txt");
        let accessible_paths = vec![accessible];

        let result = is_path_accessible(&path_to_check, &accessible_paths);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("is not allowed"));
    }

    #[test]
    fn test_nested_path_is_accessible() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();
        let nested_dir = Path::new(&accessible).join("deeply/nested/dir");
        fs::create_dir_all(&nested_dir).unwrap();
        let path_to_check = nested_dir.join("nested_file.txt");
        let accessible_paths = vec![accessible];

        assert!(is_path_accessible(&path_to_check, &accessible_paths).is_ok());
    }

    #[test]
    fn test_path_is_not_accessible_if_parent_is_not() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();
        // Here, the accessible path is a subdirectory, so its parent is not accessible.
        let accessible_subdir = Path::new(&accessible).join("subdir");
        fs::create_dir_all(&accessible_subdir).unwrap();

        // The path to check is the parent of the only accessible dir.
        let path_to_check = Path::new(&accessible);
        let accessible_paths = vec![accessible_subdir.to_str().unwrap().to_string()];

        let result = is_path_accessible(path_to_check, &accessible_paths);
        assert!(result.is_err());
    }

    #[test]
    fn test_non_existent_path_with_non_existent_parent() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();
        let path_to_check = Path::new(&_inaccessible).join("no_such_dir/file.txt");
        let accessible_paths = vec![accessible];

        let result = is_path_accessible(&path_to_check, &accessible_paths);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to resolve path")
        );
    }

    #[test]
    fn test_multiple_accessible_paths() {
        let (tmp_dir, accessible, inaccessible) = setup_test_dirs();
        let another_accessible_dir = tmp_dir.path().join("another_accessible");
        fs::create_dir(&another_accessible_dir).unwrap();

        let path_in_first = Path::new(&accessible).join("file.txt");
        let path_in_second = another_accessible_dir.join("another_file.txt");
        let path_in_inaccessible = Path::new(&inaccessible).join("secret.txt");

        let accessible_paths = vec![
            accessible,
            another_accessible_dir.to_str().unwrap().to_string(),
        ];

        assert!(is_path_accessible(&path_in_first, &accessible_paths).is_ok());
        assert!(is_path_accessible(&path_in_second, &accessible_paths).is_ok());
        assert!(is_path_accessible(&path_in_inaccessible, &accessible_paths).is_err());
    }

    #[test]
    fn test_new_file_in_current_directory_relative_path() {
        let (_tmp_dir, accessible, _inaccessible) = setup_test_dirs();

        // Temporarily change the current directory to our accessible test directory
        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(&accessible).unwrap();

        // Path is just a filename, implying the current working directory
        let path_to_check = Path::new("new_file_in_cwd.txt");

        // Accessible paths includes the current directory denoted by "."
        let accessible_paths = vec![".".to_string()];

        // The check should succeed because we are in an accessible directory.
        let result = is_path_accessible(path_to_check, &accessible_paths);
        assert!(
            result.is_ok(),
            "Failed with error: {:?}",
            result.err().map(|e| e.to_string())
        );

        // Restore the original working directory
        std::env::set_current_dir(original_cwd).unwrap();
    }
}

/// Checks if a shell command is allowed based on a prefix whitelist.
pub fn is_command_allowed(command: &str, allowed_prefixes: &[String]) -> Result<()> {
    if allowed_prefixes.is_empty() {
        return Ok(()); // If whitelist is empty, all commands are allowed.
    }

    let is_allowed = allowed_prefixes
        .iter()
        .any(|prefix| command.starts_with(prefix));

    if is_allowed {
        Ok(())
    } else {
        Err(anyhow!(
            "Command `{}` is not allowed. It does not start with any of the allowed prefixes: {:?}.",
            command,
            allowed_prefixes
        ))
    }
}

#[cfg(test)]
mod command_tests {
    use super::*;

    #[test]
    fn test_command_allowed_by_empty_whitelist() {
        let command = "ls -l";
        let allowed_prefixes: Vec<String> = vec![];
        assert!(is_command_allowed(command, &allowed_prefixes).is_ok());
    }

    #[test]
    fn test_command_allowed_by_single_prefix() {
        let command = "ls -l";
        let allowed_prefixes = vec!["ls".to_string()];
        assert!(is_command_allowed(command, &allowed_prefixes).is_ok());
    }

    #[test]
    fn test_command_not_allowed_by_prefix() {
        let command = "rm -rf /";
        let allowed_prefixes = vec!["ls".to_string(), "echo".to_string()];
        assert!(is_command_allowed(command, &allowed_prefixes).is_err());
    }

    #[test]
    fn test_command_allowed_by_multiple_prefixes() {
        let command = "echo 'hello'";
        let allowed_prefixes = vec!["ls".to_string(), "echo".to_string()];
        assert!(is_command_allowed(command, &allowed_prefixes).is_ok());
    }

    #[test]
    fn test_full_path_command_allowed() {
        let command = "/bin/ls -a";
        let allowed_prefixes = vec!["/bin/ls".to_string()];
        assert!(is_command_allowed(command, &allowed_prefixes).is_ok());
    }

    #[test]
    fn test_full_path_command_not_allowed() {
        let command = "/usr/bin/rm -rf /";
        let allowed_prefixes = vec!["/bin/ls".to_string()];
        assert!(is_command_allowed(command, &allowed_prefixes).is_err());
    }
}
