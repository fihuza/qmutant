use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{self, Error};
use crate::project::{self, Entry, Target};

#[derive(Debug)]
pub struct Sandboxes {
    parent: PathBuf,
    base: PathBuf,
    workers: Vec<PathBuf>,
}

impl Sandboxes {
    pub fn create(
        root: &Path,
        sandbox_dir: &Path,
        ignore: &[String],
        count: usize,
        cancel: &AtomicBool,
    ) -> Result<Self, Error> {
        let entries = project::entries(root, sandbox_dir, ignore)?;
        let parent = root.join(sandbox_dir);
        let base = parent.join(std::process::id().to_string());
        if base.exists() {
            fs::remove_dir_all(&base).map_err(error::at(&base))?;
        }
        let mut sandboxes = Self {
            parent,
            base,
            workers: Vec::new(),
        };
        for worker in 0..count {
            let directory = sandboxes.base.join(worker.to_string());
            copy(root, &directory, &entries, cancel)?;
            sandboxes.workers.push(directory);
        }
        Ok(sandboxes)
    }

    #[must_use]
    pub fn worker(&self, index: usize) -> &Path {
        &self.workers[index]
    }

    #[must_use]
    pub fn log(&self, index: usize) -> PathBuf {
        self.base.join(format!("{index}.log"))
    }
}

impl Drop for Sandboxes {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.base) {
            tracing::warn!("could not remove {}: {error}", self.base.display());
        }
        if fs::remove_dir(&self.parent).is_err() {
            tracing::debug!("{} still holds other runs", self.parent.display());
        }
    }
}

fn copy(
    root: &Path,
    directory: &Path,
    entries: &[Entry],
    cancel: &AtomicBool,
) -> Result<(), Error> {
    fs::create_dir_all(directory).map_err(error::at(directory))?;
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Interrupted);
        }
        match entry {
            Entry::Directory(path) => {
                let target = directory.join(path);
                fs::create_dir_all(&target).map_err(error::at(&target))?;
            }
            Entry::File(path) => {
                let target = directory.join(path);
                fs::copy(root.join(path), &target).map_err(error::at(&target))?;
            }
            Entry::Link(path, destination) => {
                let target = directory.join(path);
                let destination = match destination {
                    Target::Within(within) => directory.join(within),
                    Target::Elsewhere(elsewhere) => elsewhere.clone(),
                };
                std::os::unix::fs::symlink(destination, &target).map_err(error::at(&target))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn project() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        fs::create_dir_all(path.join("tests/qml")).unwrap();
        fs::create_dir_all(path.join(".git")).unwrap();
        fs::create_dir_all(path.join("build")).unwrap();
        fs::write(path.join("A.qml"), "Item {}").unwrap();
        fs::write(path.join(".hidden"), "kept").unwrap();
        fs::write(path.join(".git/HEAD"), "ref").unwrap();
        fs::write(path.join("build/out"), "skip").unwrap();
        fs::write(path.join("tests/run.sh"), "exit 0").unwrap();
        fs::set_permissions(path.join("tests/run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        std::os::unix::fs::symlink("A.qml", path.join("Link.qml")).unwrap();
        root
    }

    #[test]
    fn each_worker_gets_a_full_copy_without_git_or_ignored_paths() {
        let root = project();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &["build/".to_owned()],
            2,
            &AtomicBool::new(false),
        )
        .unwrap();
        for worker in 0..2 {
            let copy = sandboxes.worker(worker);
            assert_eq!(fs::read_to_string(copy.join("A.qml")).unwrap(), "Item {}");
            assert_eq!(fs::read_to_string(copy.join(".hidden")).unwrap(), "kept");
            assert!(!copy.join(".git").exists());
            assert!(!copy.join("build").exists());
            assert!(!copy.join(".qmutant").exists());
            let mode = fs::metadata(copy.join("tests/run.sh"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
            assert_eq!(
                fs::read_link(copy.join("Link.qml")).unwrap(),
                copy.join("A.qml")
            );
        }
        assert!(sandboxes.log(1).starts_with(root.path().join(".qmutant")));
    }

    #[test]
    fn a_cancelled_copy_stops_and_leaves_nothing_behind() {
        let root = project();
        let outcome = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(true),
        );
        assert!(matches!(outcome, Err(Error::Interrupted)), "{outcome:?}");
        assert!(!root.path().join(".qmutant").exists());
    }

    #[test]
    fn links_into_the_project_point_into_the_copy() {
        let root = project();
        std::os::unix::fs::symlink(root.path().join("tests"), root.path().join("absolute"))
            .unwrap();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();
        let copy = sandboxes.worker(0);
        assert_eq!(
            fs::read_link(copy.join("absolute")).unwrap(),
            copy.join("tests")
        );
        assert!(copy.join("absolute/run.sh").exists());
    }

    #[test]
    fn links_out_of_the_project_still_reach_their_target() {
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("shared.qml"), "Item {}").unwrap();
        let root = tempfile::tempdir_in(outside.path()).unwrap();
        fs::write(root.path().join("A.qml"), "Item {}").unwrap();
        std::os::unix::fs::symlink("../shared.qml", root.path().join("Shared.qml")).unwrap();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();
        let copy = sandboxes.worker(0);
        assert_eq!(
            fs::read_link(copy.join("Shared.qml")).unwrap(),
            outside.path().join("shared.qml")
        );
        assert_eq!(
            fs::read_to_string(copy.join("Shared.qml")).unwrap(),
            "Item {}"
        );
    }

    #[test]
    fn special_files_are_left_out_of_the_copy() {
        let root = project();
        nix::unistd::mkfifo(&root.path().join("pipe"), nix::sys::stat::Mode::S_IRWXU).unwrap();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(sandboxes.worker(0).join("A.qml").exists());
        assert!(!sandboxes.worker(0).join("pipe").exists());
    }

    #[test]
    fn dropping_removes_every_copy_and_the_empty_parent() {
        let root = project();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(root.path().join(".qmutant").exists());
        drop(sandboxes);
        assert!(!root.path().join(".qmutant").exists());
    }

    #[test]
    fn a_parent_holding_another_run_is_kept() {
        let root = project();
        let other = root.path().join(".qmutant/other");
        fs::create_dir_all(&other).unwrap();
        drop(
            Sandboxes::create(
                root.path(),
                Path::new(".qmutant"),
                &[],
                1,
                &AtomicBool::new(false),
            )
            .unwrap(),
        );
        assert!(other.exists());
    }

    #[test]
    fn a_stale_copy_from_the_same_pid_is_replaced() {
        let root = project();
        let stale = root
            .path()
            .join(".qmutant")
            .join(std::process::id().to_string());
        fs::create_dir_all(stale.join("0")).unwrap();
        fs::write(stale.join("0/leftover"), "").unwrap();
        let sandboxes = Sandboxes::create(
            root.path(),
            Path::new(".qmutant"),
            &[],
            1,
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(!sandboxes.worker(0).join("leftover").exists());
    }
}
