use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{}: {message}", path.display())]
    Config { path: PathBuf, message: String },
    #[error("{}:{line}: {message}", path.display())]
    Directive {
        path: PathBuf,
        line: usize,
        message: String,
    },
    #[error("`mutate` matches no files: {}", patterns.join(", "))]
    NothingToMutate { patterns: Vec<String> },
    #[error("{}: only .qml files can be mutated", .0.display())]
    NotQml(PathBuf),
    #[error("{}: the QML grammar could not finish parsing this file", .0.display())]
    Unparseable(PathBuf),
    #[error(
        "tests fail before anything is mutated, so every mutant would count as killed\n{output}"
    )]
    DryRunFailed { output: String },
    #[error("interrupted")]
    Interrupted,
    #[error("{}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Walk(#[from] ignore::Error),
    #[error(transparent)]
    Glob(#[from] globset::Error),
}

pub fn at(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_owned(),
        source,
    }
}
