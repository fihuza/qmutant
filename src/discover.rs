use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};

use crate::error::Error;
use crate::project::{self, Entry};

pub fn discover(
    root: &Path,
    patterns: &[String],
    sandbox_dir: &Path,
    ignore: &[String],
) -> Result<Vec<PathBuf>, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    let globs = builder.build()?;
    let mut found = Vec::new();
    for entry in project::entries(root, sandbox_dir, ignore)? {
        let Entry::File(relative) = entry else {
            continue;
        };
        if globs.is_match(&relative) {
            if relative
                .extension()
                .is_none_or(|extension| extension != "qml")
            {
                return Err(Error::NotQml(relative));
            }
            found.push(relative);
        }
    }
    if found.is_empty() {
        return Err(Error::NothingToMutate {
            patterns: patterns.to_vec(),
        });
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for file in files {
            let path = root.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        root
    }

    fn find_ignoring(
        root: &Path,
        patterns: &[&str],
        ignore: &[&str],
    ) -> Result<Vec<PathBuf>, String> {
        let patterns: Vec<String> = patterns.iter().map(ToString::to_string).collect();
        let ignore: Vec<String> = ignore.iter().map(ToString::to_string).collect();
        discover(root, &patterns, Path::new(".qmutant"), &ignore).map_err(|error| error.to_string())
    }

    fn find(root: &Path, patterns: &[&str]) -> Result<Vec<PathBuf>, String> {
        find_ignoring(root, patterns, &[])
    }

    #[test]
    fn paths_the_config_ignores_are_never_discovered() {
        let root = project(&["A.qml", "ui/Main.qml"]);
        assert_eq!(
            find_ignoring(root.path(), &["**/*.qml"], &["ui/"]).unwrap(),
            [PathBuf::from("A.qml")]
        );
    }

    #[test]
    fn hidden_files_are_discovered_like_any_other() {
        let root = project(&[".ui/Main.qml"]);
        assert_eq!(
            find(root.path(), &[".ui/*.qml"]).unwrap(),
            [PathBuf::from(".ui/Main.qml")]
        );
    }

    #[test]
    fn matching_files_are_listed_sorted_and_relative() {
        let root = project(&["b/B.qml", "A.qml", "Model.js"]);
        assert_eq!(
            find(root.path(), &["**/*.qml"]).unwrap(),
            [PathBuf::from("A.qml"), PathBuf::from("b/B.qml")]
        );
    }

    #[test]
    fn ignored_files_and_sandboxes_are_skipped() {
        let root = project(&["A.qml", "build/B.qml", ".qmutant/1/A.qml", ".gitignore"]);
        std::fs::write(root.path().join(".gitignore"), "build/\n").unwrap();
        assert_eq!(
            find(root.path(), &["**/*.qml"]).unwrap(),
            [PathBuf::from("A.qml")]
        );
    }

    #[test]
    fn a_match_that_is_not_qml_is_refused() {
        let root = project(&["Model.js"]);
        assert_eq!(
            find(root.path(), &["*.js"]).unwrap_err(),
            "Model.js: only .qml files can be mutated"
        );
    }

    #[test]
    fn matching_nothing_is_refused() {
        let root = project(&["A.qml"]);
        assert_eq!(
            find(root.path(), &["src/*.qml", "B.qml"]).unwrap_err(),
            "`mutate` matches no files: src/*.qml, B.qml"
        );
    }

    #[test]
    fn a_malformed_glob_is_refused() {
        let root = project(&["A.qml"]);
        assert!(
            find(root.path(), &["a[.qml"])
                .unwrap_err()
                .contains("a[.qml")
        );
    }
}
