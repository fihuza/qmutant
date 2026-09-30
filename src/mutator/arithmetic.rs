use tree_sitter::Node;

use super::{Replacement, is_string, swap_operator};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    let concatenates =
        is_string(node.child_by_field_name("left")) || is_string(node.child_by_field_name("right"));
    if concatenates {
        return Vec::new();
    }
    swap_operator(
        node,
        source,
        "binary_expression",
        |operator| match operator {
            "+" => &["-"],
            "-" => &["+"],
            "*" => &["/"],
            "/" | "%" => &["*"],
            _ => &[],
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::ArithmeticOperator, source)
    }

    #[test]
    fn each_operator_becomes_its_counterpart() {
        assert_eq!(mutate("Item { x: a + b }"), ["Item { x: a - b }"]);
        assert_eq!(mutate("Item { x: a - b }"), ["Item { x: a + b }"]);
        assert_eq!(mutate("Item { x: a * b }"), ["Item { x: a / b }"]);
        assert_eq!(mutate("Item { x: a / b }"), ["Item { x: a * b }"]);
        assert_eq!(mutate("Item { x: a % b }"), ["Item { x: a * b }"]);
    }

    #[test]
    fn string_concatenation_is_left_alone() {
        assert!(mutate(r#"Item { x: "a" + b }"#).is_empty());
        assert!(mutate("Item { x: a + `b` }").is_empty());
    }

    #[test]
    fn logical_operators_are_left_alone() {
        assert!(mutate("Item { x: a && b }").is_empty());
    }
}
