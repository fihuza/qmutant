use std::fs;
use std::path::{Component, Path, PathBuf};

use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

use crate::error::{self, Error};

#[derive(Debug, PartialEq, Eq)]
pub enum Entry {
    Directory(PathBuf),
    File(PathBuf),
    Link(PathBuf, Target),
}

#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Within(PathBuf),
    Elsewhere(PathBuf),
}

pub fn entries(root: &Path, sandbox_dir: &Path, ignore: &[String]) -> Result<Vec<Entry>, Error> {
    let mut overrides = OverrideBuilder::new(root);
    overrides.add("!/.git/")?;
    overrides.add(&format!("!/{}/", sandbox_dir.display()))?;
    for pattern in ignore {
        overrides.add(&format!("!{pattern}"))?;
    }
    let mut entries = Vec::new();
    for entry in WalkBuilder::new(root)
        .hidden(false)
        .require_git(false)
        .overrides(overrides.build()?)
        .build()
    {
        let entry = entry?;
        let relative = entry
            .path()
            .strip_prefix(root)
            .expect("the walker only yields paths below its root")
            .to_owned();
        let Some(kind) = entry.file_type() else {
            continue;
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        if kind.is_dir() {
            entries.push(Entry::Directory(relative));
        } else if kind.is_file() {
            entries.push(Entry::File(relative));
        } else if kind.is_symlink() {
            let target = fs::read_link(entry.path()).map_err(error::at(entry.path()))?;
            let target = classify(root, &relative, &target);
            entries.push(Entry::Link(relative, target));
        } else {
            tracing::debug!(
                "{} is not a file, directory or link; skipped",
                relative.display()
            );
        }
    }
    Ok(entries)
}

fn classify(root: &Path, link: &Path, target: &Path) -> Target {
    let directory = root
        .join(link)
        .parent()
        .map(Path::to_owned)
        .unwrap_or_default();
    let resolved = normalize(&directory.join(target));
    match resolved.strip_prefix(root) {
        Ok(within) => Target::Within(within.to_owned()),
        Err(_) => Target::Elsewhere(resolved),
    }
}

fn normalize(path: &Path) -> PathBuf {
    path.components()
        .fold(PathBuf::new(), |mut normal, component| {
            match component {
                Component::CurDir => {}
                Component::ParentDir => {
                    normal.pop();
                }
                other => normal.push(other),
            }
            normal
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_targets_are_resolved_against_the_link_and_the_root() {
        let root = Path::new("/project");
        let classify =
            |link: &str, target: &str| classify(root, Path::new(link), Path::new(target));
        assert_eq!(
            classify("ui/Link.qml", "Main.qml"),
            Target::Within("ui/Main.qml".into())
        );
        assert_eq!(
            classify("ui/Link.qml", "../A.qml"),
            Target::Within("A.qml".into())
        );
        assert_eq!(
            classify("ui/Link.qml", "./x/../B.qml"),
            Target::Within("ui/B.qml".into())
        );
        assert_eq!(
            classify("Link", "/project/tests"),
            Target::Within("tests".into())
        );
        assert_eq!(
            classify("Link", "../shared"),
            Target::Elsewhere("/shared".into())
        );
        assert_eq!(
            classify("Link", "/usr/lib/qt6"),
            Target::Elsewhere("/usr/lib/qt6".into())
        );
        assert_eq!(
            classify("Link", "/project-other/x"),
            Target::Elsewhere("/project-other/x".into())
        );
    }
}
