use std::path::PathBuf;

use crate::config::Config;
use crate::directive::{Directive, Directives};
use crate::discover::discover;
use crate::error::{self, Error};
use crate::mutant::{Lines, Mutant, SourceFile, Status};
use crate::mutator::mutations;
use crate::parse;

#[derive(Debug)]
pub struct Plan {
    pub files: Vec<SourceFile>,
    pub mutants: Vec<Mutant>,
    pub decided: Vec<Option<Status>>,
    pub unused: Vec<(PathBuf, Directive)>,
}

impl Plan {
    pub fn pending(&self) -> impl Iterator<Item = &Mutant> {
        self.mutants
            .iter()
            .filter(|mutant| self.decided[mutant.id].is_none())
    }
}

pub fn instrument(config: &Config) -> Result<Plan, Error> {
    let mut plan = Plan {
        files: Vec::new(),
        mutants: Vec::new(),
        decided: Vec::new(),
        unused: Vec::new(),
    };
    for path in discover(
        &config.root,
        &config.mutate,
        &config.sandbox_dir,
        &config.ignore,
    )? {
        let absolute = config.root.join(&path);
        let source = std::fs::read_to_string(&absolute).map_err(error::at(&absolute))?;
        let tree = parse::parse(&source).ok_or_else(|| Error::Unparseable(path.clone()))?;
        let mut directives = Directives::parse(&tree, &source, &path)?;
        let lines = Lines::new(&source);
        for mutation in mutations(&tree, &source, &config.exclude_mutators) {
            let start = lines.position(mutation.range.start);
            let mutant = Mutant {
                id: plan.mutants.len(),
                file: plan.files.len(),
                mutator: mutation.mutator,
                start,
                end: lines.position(mutation.range.end),
                range: mutation.range,
                replacement: mutation.replacement,
            };
            let decided = if directives.ignores(mutant.mutator, mutant.range.start, start.line) {
                Some(Status::Ignored)
            } else if parse::is_valid_edit(&tree, &mutant.edit(&lines), &mutant.apply(&source)) {
                None
            } else {
                Some(Status::Invalid)
            };
            plan.decided.push(decided);
            plan.mutants.push(mutant);
        }
        plan.unused.extend(
            directives
                .unused(&config.exclude_mutators)
                .map(|directive| (path.clone(), directive.clone())),
        );
        plan.files.push(SourceFile { path, source });
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::config::Overrides;

    fn config_in(root: &Path) -> Config {
        Config::parse(
            "command = \"true\"",
            Path::new("qmutant.toml"),
            root.to_owned(),
            Overrides::default(),
        )
        .unwrap()
    }

    #[test]
    fn three_thousand_mutants_are_prepared_in_seconds_not_minutes() {
        let root = tempfile::tempdir().unwrap();
        let functions = (0..300)
            .map(|i| {
                format!(
                    "    function f{i}(v) {{ return v < {i} && v !== \"x{i}\" ? v + 1 : [v, 2] }}\n"
                )
            })
            .collect::<Vec<_>>()
            .concat();
        std::fs::write(
            root.path().join("Big.qml"),
            format!("Item {{\n{functions}}}\n"),
        )
        .unwrap();
        let started = Instant::now();
        let plan = instrument(&config_in(root.path())).unwrap();
        assert_eq!(plan.mutants.len(), 3000);
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "took {:?}",
            started.elapsed()
        );
    }
}
