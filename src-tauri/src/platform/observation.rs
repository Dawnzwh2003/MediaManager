//! Lexical validation only. Resolving real filesystem paths belongs to the scanner.
use std::path::{Component, Path};

pub fn is_valid_directory_observation(root: &Path, path: &Path) -> bool {
    root.is_absolute()
        && path.is_absolute()
        && !root.components().any(|part| part == Component::ParentDir)
        && !path.components().any(|part| part == Component::ParentDir)
        && path.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        std::env::temp_dir().join("media-manager-observation-tests")
    }

    #[test]
    fn accepts_absolute_root_and_descendants_without_touching_disk() {
        let root = root();
        assert!(is_valid_directory_observation(&root, &root));
        assert!(is_valid_directory_observation(&root, &root.join("child")));
    }

    #[test]
    fn rejects_parent_traversal_even_when_lexical_parent_matches_root() {
        let root = root();
        let traversal = root.join("..");
        // This candidate passed both checks used by the old repository code.
        assert!(traversal.starts_with(&root));
        assert_eq!(traversal.parent(), Some(root.as_path()));
        assert!(!is_valid_directory_observation(&root, &traversal));
        assert!(!is_valid_directory_observation(
            &root,
            &root.join("child").join("..").join("other")
        ));
    }

    #[test]
    fn rejects_a_root_containing_parent_traversal() {
        let root = root().join("..").join("elsewhere");
        assert!(!is_valid_directory_observation(&root, &root));
    }

    #[test]
    fn rejects_relative_paths_and_unrelated_absolute_directories() {
        let root = root();
        assert!(!is_valid_directory_observation(
            &root,
            Path::new("relative")
        ));
        assert!(!is_valid_directory_observation(
            Path::new("relative"),
            &root
        ));
        assert!(!is_valid_directory_observation(
            &root,
            &root.parent().unwrap().join("unrelated")
        ));
    }
}
