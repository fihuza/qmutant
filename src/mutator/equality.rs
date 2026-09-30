use tree_sitter::Node;

use super::{Replacement, swap_operator};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    swap_operator(
        node,
        source,
        "binary_expression",
        |operator| match operator {
            "<" => &["<=", ">="],
            "<=" => &["<", ">"],
            ">" => &[">=", "<="],
            ">=" => &[">", "<"],
            "==" => &["!="],
            "!=" => &["=="],
            "===" => &["!=="],
            "!==" => &["==="],
            _ => &[],
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::EqualityOperator, source)
    }

    #[test]
    fn relational_operators_move_their_boundary_both_ways() {
        assert_eq!(
            mutate("Item { x: a < b }"),
            ["Item { x: a <= b }", "Item { x: a >= b }"]
        );
        assert_eq!(
            mutate("Item { x: a <= b }"),
            ["Item { x: a < b }", "Item { x: a > b }"]
        );
        assert_eq!(
            mutate("Item { x: a > b }"),
            ["Item { x: a >= b }", "Item { x: a <= b }"]
        );
        assert_eq!(
            mutate("Item { x: a >= b }"),
            ["Item { x: a > b }", "Item { x: a < b }"]
        );
    }

    #[test]
    fn equality_is_negated() {
        assert_eq!(mutate("Item { x: a == b }"), ["Item { x: a != b }"]);
        assert_eq!(mutate("Item { x: a != b }"), ["Item { x: a == b }"]);
        assert_eq!(mutate("Item { x: a === b }"), ["Item { x: a !== b }"]);
        assert_eq!(mutate("Item { x: a !== b }"), ["Item { x: a === b }"]);
    }

    #[test]
    fn other_binary_operators_are_left_alone() {
        assert!(mutate("Item { x: a + b }").is_empty());
    }
}
