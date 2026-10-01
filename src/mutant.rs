use std::ops::Range;
use std::path::PathBuf;

use serde::Serialize;
use tree_sitter::{InputEdit, Point};

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
        Lines::new(source).position(byte)
    }
}

pub struct Lines<'a> {
    source: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    #[must_use]
    pub fn new(source: &'a str) -> Self {
        let starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(newline, _)| newline + 1))
            .collect();
        Self { source, starts }
    }

    fn line_of(&self, byte: usize) -> usize {
        self.starts.partition_point(|&start| start <= byte)
    }

    #[must_use]
    pub fn position(&self, byte: usize) -> Position {
        let line = self.line_of(byte);
        let start = self.starts[line - 1];
        Position {
            line,
            column: self.source[start..byte].chars().count() + 1,
        }
    }

    #[must_use]
    pub fn point(&self, byte: usize) -> Point {
        let row = self.line_of(byte) - 1;
        Point::new(row, byte - self.starts[row])
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
    pub fn edit(&self, lines: &Lines) -> InputEdit {
        let start = lines.point(self.range.start);
        let new_end = match self.replacement.rfind('\n') {
            Some(newline) => Point::new(
                start.row + self.replacement.matches('\n').count(),
                self.replacement.len() - newline - 1,
            ),
            None => Point::new(start.row, start.column + self.replacement.len()),
        };
        InputEdit {
            start_byte: self.range.start,
            old_end_byte: self.range.end,
            new_end_byte: self.range.start + self.replacement.len(),
            start_position: start,
            old_end_position: lines.point(self.range.end),
            new_end_position: new_end,
        }
    }

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
    fn line_starts_answer_positions_and_points() {
        let source = "ab\ncé d\n";
        let lines = Lines::new(source);
        assert_eq!(lines.position(0), Position { line: 1, column: 1 });
        assert_eq!(lines.position(2), Position { line: 1, column: 3 });
        assert_eq!(lines.position(3), Position { line: 2, column: 1 });
        assert_eq!(lines.position(7), Position { line: 2, column: 4 });
        assert_eq!(lines.position(9), Position { line: 3, column: 1 });
        assert_eq!(lines.point(7), Point::new(1, 4));
    }

    #[test]
    fn an_edit_spans_the_old_range_and_the_new_text() {
        let source = "x\na < b";
        let mutant = Mutant {
            id: 0,
            file: 0,
            mutator: Mutator::EqualityOperator,
            range: 4..5,
            start: Position { line: 2, column: 3 },
            end: Position { line: 2, column: 4 },
            replacement: "<=".to_owned(),
        };
        let edit = mutant.edit(&Lines::new(source));
        assert_eq!(
            (edit.start_byte, edit.old_end_byte, edit.new_end_byte),
            (4, 5, 6)
        );
        assert_eq!(edit.start_position, Point::new(1, 2));
        assert_eq!(edit.old_end_position, Point::new(1, 3));
        assert_eq!(edit.new_end_position, Point::new(1, 4));
        let multiline = Mutant {
            replacement: "{\n  }".to_owned(),
            ..mutant
        };
        let edit = multiline.edit(&Lines::new(source));
        assert_eq!(edit.new_end_position, Point::new(2, 3));
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
