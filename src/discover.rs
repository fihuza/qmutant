use std::path::{Path, PathBuf};

use globset::{Glob, GlobSetBuilder};
use ignore::WalkBuilder;

use crate::error::Error;

pub fn discover(
    root: &Path,
    patterns: &[String],
    sandbox_dir: &Path,
) -> Result<Vec<PathBuf>, Error> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(Glob::new(pattern)?);
    }
    let globs = builder.build()?;
    let sandboxes = root.join(sandbox_dir);
    let mut found = Vec::new();
    for entry in WalkBuilder::new(root)
        .require_git(false)
        .filter_entry(move |entry| entry.path() != sandboxes)
        .build()
    {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("the walker only yields paths below its root");
        if entry.file_type().is_some_and(|kind| kind.is_file()) && globs.is_match(relative) {
            if relative
                .extension()
                .is_none_or(|extension| extension != "qml")
            {
                return Err(Error::NotQml(relative.to_owned()));
            }
            found.push(relative.to_owned());
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

    fn find(root: &Path, patterns: &[&str]) -> Result<Vec<PathBuf>, String> {
        let patterns: Vec<String> = patterns.iter().map(ToString::to_string).collect();
        discover(root, &patterns, Path::new(".qmutant")).map_err(|error| error.to_string())
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
