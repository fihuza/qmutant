use std::path::Path;

use tree_sitter::{Node, Tree};

use crate::error::Error;
use crate::mutant::Position;
use crate::mutator::Mutator;
use crate::parse;

const PREFIX: &str = "qmutant:";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Directive {
    pub line: usize,
    pub text: String,
    action: Action,
    scope: Scope,
    end: usize,
    used: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    DisableNextLine,
    Disable,
    Enable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Scope {
    All,
    Only(Vec<Mutator>),
}

impl Scope {
    fn only_names(&self, excluded: &[Mutator]) -> bool {
        match self {
            Self::All => Mutator::ALL
                .iter()
                .all(|mutator| excluded.contains(mutator)),
            Self::Only(mutators) => mutators.iter().all(|mutator| excluded.contains(mutator)),
        }
    }

    fn covers(&self, mutator: Mutator) -> bool {
        match self {
            Self::All => true,
            Self::Only(mutators) => mutators.contains(&mutator),
        }
    }
}

#[derive(Debug, Default)]
pub struct Directives {
    list: Vec<Directive>,
}

impl Directives {
    pub fn parse(tree: &Tree, source: &str, path: &Path) -> Result<Self, Error> {
        let mut comments = Vec::new();
        parse::walk(tree, |node| {
            if node.kind() == "comment" {
                comments.push(node);
            }
            true
        });
        let list = comments
            .into_iter()
            .filter_map(|node| directive(node, source, path).transpose())
            .collect::<Result<_, _>>()?;
        Ok(Self { list })
    }

    pub fn ignores(&mut self, mutator: Mutator, start: usize, line: usize) -> bool {
        let next_line = self.list.iter_mut().find(|directive| {
            directive.action == Action::DisableNextLine
                && directive.line + 1 == line
                && directive.scope.covers(mutator)
        });
        if let Some(directive) = next_line {
            directive.used = true;
            return true;
        }
        let range = self.list.iter_mut().rev().find(|directive| {
            directive.action != Action::DisableNextLine
                && directive.end <= start
                && directive.scope.covers(mutator)
        });
        match range {
            Some(directive) if directive.action == Action::Disable => {
                directive.used = true;
                true
            }
            _ => false,
        }
    }

    pub fn unused<'a>(&'a self, excluded: &'a [Mutator]) -> impl Iterator<Item = &'a Directive> {
        self.list.iter().filter(move |directive| {
            directive.action != Action::Enable
                && !directive.used
                && !directive.scope.only_names(excluded)
        })
    }
}

fn directive(node: Node, source: &str, path: &Path) -> Result<Option<Directive>, Error> {
    let comment = &source[node.byte_range()];
    let Some(body) = comment
        .strip_prefix("//")
        .map(str::trim)
        .and_then(|body| body.strip_prefix(PREFIX))
    else {
        return Ok(None);
    };
    let line = Position::at(source, node.start_byte()).line;
    let (action, scope) = read(body).map_err(|message| Error::Directive {
        path: path.to_owned(),
        line,
        message,
    })?;
    Ok(Some(Directive {
        line,
        text: comment.to_owned(),
        action,
        scope,
        end: node.end_byte(),
        used: false,
    }))
}

fn read(body: &str) -> Result<(Action, Scope), String> {
    let (spec, reason) = match body.split_once("--") {
        Some((spec, reason)) => (
            spec,
            Some(reason.trim()).filter(|reason| !reason.is_empty()),
        ),
        None => (body, None),
    };
    let mut words = spec.split_whitespace();
    let action = match (words.next(), words.clone().next()) {
        (Some("disable"), Some("next-line")) => {
            words.next();
            Action::DisableNextLine
        }
        (Some("disable"), _) => Action::Disable,
        (Some("enable"), _) => Action::Enable,
        _ => return Err("expected `disable`, `disable next-line` or `enable`".to_owned()),
    };
    let names: Vec<&str> = words
        .flat_map(|word| word.split(','))
        .filter(|name| !name.is_empty())
        .collect();
    let scope = match names.as_slice() {
        [] => return Err("name the mutators, or `all`".to_owned()),
        ["all"] => Scope::All,
        names => Scope::Only(
            names
                .iter()
                .map(|name| name.parse())
                .collect::<Result<_, _>>()?,
        ),
    };
    match (action, reason) {
        (Action::Enable, None) | (Action::Disable | Action::DisableNextLine, Some(_)) => {
            Ok((action, scope))
        }
        (Action::Enable, Some(_)) => Err("`enable` takes no reason".to_owned()),
        (_, None) => {
            Err("a disabled mutant needs a reason: `-- <why it cannot be killed>`".to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse;

    fn directives(source: &str) -> Result<Directives, String> {
        Directives::parse(&parse::parse(source).unwrap(), source, Path::new("A.qml"))
            .map_err(|error| error.to_string())
    }

    fn start_of(source: &str, needle: &str) -> usize {
        source.find(needle).unwrap()
    }

    #[test]
    fn next_line_covers_only_the_following_line_and_its_mutators() {
        let source = "Item {\n    // qmutant: disable next-line EqualityOperator -- why\n    x: a < b\n    y: a < b\n}";
        let mut directives = directives(source).unwrap();
        let first = start_of(source, "a < b");
        assert!(!directives.ignores(Mutator::LogicalOperator, first, 3));
        assert!(directives.ignores(Mutator::EqualityOperator, first, 3));
        assert!(!directives.ignores(Mutator::EqualityOperator, first + 13, 4));
        assert_eq!(directives.unused(&[]).count(), 0);
    }

    #[test]
    fn a_range_lasts_until_it_is_enabled_again() {
        let source = "Item {\n    x: a\n    // qmutant: disable all -- why\n    y: b\n    // qmutant: enable StringLiteral\n    z: c\n}";
        let mut directives = directives(source).unwrap();
        assert!(!directives.ignores(Mutator::StringLiteral, start_of(source, "x: a"), 2));
        assert!(directives.ignores(Mutator::StringLiteral, start_of(source, "y: b"), 4));
        assert!(!directives.ignores(Mutator::StringLiteral, start_of(source, "z: c"), 6));
        assert!(directives.ignores(Mutator::BooleanLiteral, start_of(source, "z: c"), 6));
    }

    #[test]
    fn a_directive_that_ignored_nothing_is_reported_unused() {
        let source = "Item {\n    // qmutant: disable next-line ArrayDeclaration, ObjectLiteral -- why\n    x: 1\n}";
        let directives = directives(source).unwrap();
        let unused: Vec<_> = directives
            .unused(&[])
            .map(|directive| directive.line)
            .collect();
        assert_eq!(unused, [2]);
    }

    #[test]
    fn a_directive_naming_only_excluded_mutators_is_not_reported_unused() {
        let source = "Item {\n    // qmutant: disable next-line StringLiteral -- decoration\n    x: 1\n    // qmutant: disable next-line StringLiteral,EqualityOperator -- both\n    y: 1\n}";
        let directives = directives(source).unwrap();
        let unused: Vec<_> = directives
            .unused(&[Mutator::StringLiteral])
            .map(|directive| directive.line)
            .collect();
        assert_eq!(unused, [4]);
    }

    #[test]
    fn enable_is_never_reported_unused() {
        let directives = directives("Item {\n    // qmutant: enable all\n}").unwrap();
        assert_eq!(directives.unused(&[]).count(), 0);
    }

    #[test]
    fn directives_are_found_in_deeply_nested_code_without_exhausting_the_stack() {
        let depth = 50_000;
        let source = format!(
            "Item {{ x: {}a // qmutant: disable next-line all -- why\n{} }}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        assert_eq!(directives(&source).unwrap().list.len(), 1);
    }

    #[test]
    fn ordinary_comments_are_not_directives() {
        let directives =
            directives("Item {\n    // qmutant is great\n    /* qmutant: disable all */\n}")
                .unwrap();
        assert!(directives.list.is_empty());
    }

    #[test]
    fn malformed_directives_are_refused_with_their_line() {
        let cases = [
            ("disable next-line EqualityOperator", "needs a reason"),
            ("disable all --  ", "needs a reason"),
            ("enable all -- because", "takes no reason"),
            ("disable -- why", "name the mutators"),
            ("disable Nope -- why", "unknown mutator `Nope`"),
            ("skip all -- why", "expected `disable`"),
        ];
        for (body, expected) in cases {
            let error = directives(&format!("Item {{\n    // qmutant: {body}\n}}")).unwrap_err();
            assert!(error.starts_with("A.qml:2: "), "{error}");
            assert!(error.contains(expected), "{body:?} gave {error}");
        }
    }
}
