use std::path::PathBuf;

use crate::config::Config;
use crate::directive::{Directive, Directives};
use crate::discover::discover;
use crate::error::{self, Error};
use crate::mutant::{Mutant, Position, SourceFile, Status};
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
        for mutation in mutations(&tree, &source, &config.exclude_mutators) {
            let start = Position::at(&source, mutation.range.start);
            let mutant = Mutant {
                id: plan.mutants.len(),
                file: plan.files.len(),
                mutator: mutation.mutator,
                start,
                end: Position::at(&source, mutation.range.end),
                range: mutation.range,
                replacement: mutation.replacement,
            };
            let decided = if directives.ignores(mutant.mutator, mutant.range.start, start.line) {
                Some(Status::Ignored)
            } else if parse::is_valid(&mutant.apply(&source)) {
                None
            } else {
                Some(Status::Invalid)
            };
            plan.decided.push(decided);
            plan.mutants.push(mutant);
        }
        plan.unused.extend(
            directives
                .unused()
                .map(|directive| (path.clone(), directive.clone())),
        );
        plan.files.push(SourceFile { path, source });
    }
    Ok(plan)
}
