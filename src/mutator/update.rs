use tree_sitter::Node;

use super::{Replacement, swap_operator};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    swap_operator(
        node,
        source,
        "update_expression",
        |operator| match operator {
            "++" => &["--"],
            "--" => &["++"],
            _ => &[],
        },
    )
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::UpdateOperator, source)
    }

    #[test]
    fn increments_and_decrements_swap_in_either_position() {
        assert_eq!(
            mutate("Item { function f() { a++; --b } }"),
            [
                "Item { function f() { a--; --b } }",
                "Item { function f() { a++; ++b } }"
            ]
        );
    }
}
