use std::ops::Range;
use std::path::PathBuf;

use serde::Serialize;

use crate::mutator::Mutator;

#[derive(Clone, Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub source: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    #[must_use]
    pub fn at(source: &str, byte: usize) -> Self {
        let before = &source[..byte];
        let line_start = before.rfind('\n').map_or(0, |newline| newline + 1);
        Self {
            line: before.matches('\n').count() + 1,
            column: before[line_start..].chars().count() + 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Mutant {
    pub id: usize,
    pub file: usize,
    pub mutator: Mutator,
    pub range: Range<usize>,
    pub start: Position,
    pub end: Position,
    pub replacement: String,
}

impl Mutant {
    #[must_use]
    pub fn apply(&self, source: &str) -> String {
        let mut mutated = String::with_capacity(source.len() + self.replacement.len());
        mutated.push_str(&source[..self.range.start]);
        mutated.push_str(&self.replacement);
        mutated.push_str(&source[self.range.end..]);
        mutated
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Killed,
    Survived,
    Timeout,
    Invalid,
    Error,
    Ignored,
}

#[derive(Clone, Debug)]
pub struct Verdict {
    pub mutant: usize,
    pub status: Status,
    pub reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_are_one_based_and_count_characters() {
        let source = "ab\ncé d";
        assert_eq!(Position::at(source, 0), Position { line: 1, column: 1 });
        assert_eq!(Position::at(source, 3), Position { line: 2, column: 1 });
        assert_eq!(Position::at(source, 7), Position { line: 2, column: 4 });
    }

    #[test]
    fn applying_replaces_exactly_the_range() {
        let mutant = Mutant {
            id: 0,
            file: 0,
            mutator: Mutator::EqualityOperator,
            range: 2..3,
            start: Position { line: 1, column: 3 },
            end: Position { line: 1, column: 4 },
            replacement: "<=".to_owned(),
        };
        assert_eq!(mutant.apply("a < b"), "a <= b");
    }
}
