use tree_sitter::Node;

use super::{Replacement, swap_operator};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    swap_operator(
        node,
        source,
        "unary_expression",
        |operator| match operator {
            "+" => &["-"],
            "-" => &["+"],
            _ => &[],
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::UnaryOperator, source)
    }

    #[test]
    fn a_sign_is_flipped() {
        assert_eq!(mutate("Item { x: -a }"), ["Item { x: +a }"]);
        assert_eq!(mutate("Item { x: +a }"), ["Item { x: -a }"]);
    }

    #[test]
    fn negation_belongs_to_the_boolean_mutator() {
        assert!(mutate("Item { x: !a }").is_empty());
    }
}
