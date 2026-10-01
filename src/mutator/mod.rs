mod arithmetic;
mod array;
mod arrow;
mod assignment;
mod block;
mod boolean;
mod conditional;
mod equality;
mod logical;
mod method;
mod object;
mod optional_chaining;
mod string;
mod unary;
mod update;

use std::fmt;
use std::ops::Range;
use std::str::FromStr;

use serde::Deserialize;
use tree_sitter::{Node, Tree};

use crate::parse;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum Mutator {
    ArithmeticOperator,
    ArrayDeclaration,
    ArrowFunction,
    AssignmentOperator,
    BlockStatement,
    BooleanLiteral,
    ConditionalExpression,
    EqualityOperator,
    LogicalOperator,
    MethodExpression,
    ObjectLiteral,
    OptionalChaining,
    StringLiteral,
    UnaryOperator,
    UpdateOperator,
}

impl Mutator {
    pub const ALL: [Self; 15] = [
        Self::ArithmeticOperator,
        Self::ArrayDeclaration,
        Self::ArrowFunction,
        Self::AssignmentOperator,
        Self::BlockStatement,
        Self::BooleanLiteral,
        Self::ConditionalExpression,
        Self::EqualityOperator,
        Self::LogicalOperator,
        Self::MethodExpression,
        Self::ObjectLiteral,
        Self::OptionalChaining,
        Self::StringLiteral,
        Self::UnaryOperator,
        Self::UpdateOperator,
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::ArithmeticOperator => "ArithmeticOperator",
            Self::ArrayDeclaration => "ArrayDeclaration",
            Self::ArrowFunction => "ArrowFunction",
            Self::AssignmentOperator => "AssignmentOperator",
            Self::BlockStatement => "BlockStatement",
            Self::BooleanLiteral => "BooleanLiteral",
            Self::ConditionalExpression => "ConditionalExpression",
            Self::EqualityOperator => "EqualityOperator",
            Self::LogicalOperator => "LogicalOperator",
            Self::MethodExpression => "MethodExpression",
            Self::ObjectLiteral => "ObjectLiteral",
            Self::OptionalChaining => "OptionalChaining",
            Self::StringLiteral => "StringLiteral",
            Self::UnaryOperator => "UnaryOperator",
            Self::UpdateOperator => "UpdateOperator",
        }
    }

    fn replacements(self, node: Node, source: &str) -> Vec<Replacement> {
        match self {
            Self::ArithmeticOperator => arithmetic::mutate(node, source),
            Self::ArrayDeclaration => array::mutate(node),
            Self::ArrowFunction => arrow::mutate(node, source),
            Self::AssignmentOperator => assignment::mutate(node, source),
            Self::BlockStatement => block::mutate(node),
            Self::BooleanLiteral => boolean::mutate(node, source),
            Self::ConditionalExpression => conditional::mutate(node),
            Self::EqualityOperator => equality::mutate(node, source),
            Self::LogicalOperator => logical::mutate(node, source),
            Self::MethodExpression => method::mutate(node, source),
            Self::ObjectLiteral => object::mutate(node),
            Self::OptionalChaining => optional_chaining::mutate(node),
            Self::StringLiteral => string::mutate(node, source),
            Self::UnaryOperator => unary::mutate(node, source),
            Self::UpdateOperator => update::mutate(node, source),
        }
    }
}

impl fmt::Display for Mutator {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

impl TryFrom<String> for Mutator {
    type Error = String;

    fn try_from(name: String) -> Result<Self, Self::Error> {
        name.parse()
    }
}

impl FromStr for Mutator {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|mutator| mutator.name() == name)
            .ok_or_else(|| format!("unknown mutator `{name}`"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutation {
    pub mutator: Mutator,
    pub range: Range<usize>,
    pub replacement: String,
}

struct Replacement {
    range: Range<usize>,
    text: String,
}

impl Replacement {
    fn new(node: Node, text: impl Into<String>) -> Self {
        Self {
            range: node.byte_range(),
            text: text.into(),
        }
    }
}

#[must_use]
pub fn mutations(tree: &Tree, source: &str, excluded: &[Mutator]) -> Vec<Mutation> {
    let enabled: Vec<Mutator> = Mutator::ALL
        .into_iter()
        .filter(|mutator| !excluded.contains(mutator))
        .collect();
    let mut found = Vec::new();
    parse::walk(tree, |node| {
        if is_declaration(node) {
            return false;
        }
        for &mutator in &enabled {
            found.extend(
                mutator
                    .replacements(node, source)
                    .into_iter()
                    .filter(|replacement| source[replacement.range.clone()] != replacement.text)
                    .map(|replacement| Mutation {
                        mutator,
                        range: replacement.range,
                        replacement: replacement.text,
                    }),
            );
        }
        true
    });
    found
}

fn is_declaration(node: Node) -> bool {
    matches!(
        node.kind(),
        "ui_pragma" | "ui_import" | "ui_signal" | "comment"
    )
}

fn text<'a>(node: Node, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn swap_operator(
    node: Node,
    source: &str,
    kind: &str,
    table: fn(&str) -> &'static [&'static str],
) -> Vec<Replacement> {
    if node.kind() != kind {
        return Vec::new();
    }
    node.child_by_field_name("operator")
        .map(|operator| {
            table(text(operator, source))
                .iter()
                .map(|swapped| Replacement::new(operator, *swapped))
                .collect()
        })
        .unwrap_or_default()
}

fn is_string(node: Option<Node>) -> bool {
    node.is_some_and(|node| matches!(node.kind(), "string" | "template_string"))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::parse;

    pub(crate) fn render(source: &str) -> Vec<String> {
        let tree = parse::parse(source).unwrap();
        mutations(&tree, source, &[])
            .into_iter()
            .map(|mutation| {
                let start = crate::mutant::Position::at(source, mutation.range.start);
                format!(
                    "{}:{} {}: `{}` -> `{}`",
                    start.line,
                    start.column,
                    mutation.mutator,
                    source[mutation.range]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                    mutation.replacement
                )
            })
            .collect()
    }

    pub(crate) fn render_one(mutator: Mutator, source: &str) -> Vec<String> {
        let excluded: Vec<Mutator> = Mutator::ALL
            .into_iter()
            .filter(|other| *other != mutator)
            .collect();
        let tree = parse::parse(source).unwrap();
        mutations(&tree, source, &excluded)
            .into_iter()
            .map(|mutation| {
                let mut mutated = source.to_owned();
                mutated.replace_range(mutation.range, &mutation.replacement);
                mutated.trim().to_owned()
            })
            .collect()
    }

    #[test]
    fn names_round_trip_through_parsing() {
        for mutator in Mutator::ALL {
            assert_eq!(mutator.name().parse::<Mutator>(), Ok(mutator));
            assert_eq!(mutator.to_string(), mutator.name());
        }
    }

    #[test]
    fn an_unknown_name_is_refused() {
        assert_eq!(
            "Equality".parse::<Mutator>(),
            Err("unknown mutator `Equality`".to_owned())
        );
    }

    #[test]
    fn declarations_are_never_mutated() {
        let source = r#"pragma Singleton
import QtQuick 2.15
import "Model.js" as Model
Item {
    id: root
    signal done(string why)
    // a < b && "c"
    property int count
}
"#;
        assert!(render(source).is_empty());
    }

    #[test]
    fn deeply_nested_code_is_walked_without_exhausting_the_stack() {
        let depth = 50_000;
        let source = format!(
            "Item {{ x: {}a < b{} }}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        let tree = parse::parse(&source).unwrap();
        let found = mutations(&tree, &source, &[]);
        assert!(
            found
                .iter()
                .any(|mutation| mutation.mutator == Mutator::EqualityOperator)
        );
    }

    #[test]
    fn an_excluded_mutator_yields_nothing() {
        let source = "Item { visible: a < b }";
        let tree = parse::parse(source).unwrap();
        assert!(mutations(&tree, source, &[Mutator::EqualityOperator]).is_empty());
    }

    #[test]
    fn every_mutator_is_found_in_a_realistic_component() {
        insta::assert_debug_snapshot!(render(include_str!(
            "../../tests/fixtures/components/Everything.qml"
        )));
    }
}
