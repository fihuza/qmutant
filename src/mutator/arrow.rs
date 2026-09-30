use tree_sitter::Node;

use super::{Replacement, text};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    if node.kind() != "arrow_function" {
        return Vec::new();
    }
    node.child_by_field_name("body")
        .filter(|body| body.kind() != "statement_block" && text(*body, source) != "undefined")
        .map(|body| vec![Replacement::new(body, "undefined")])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::ArrowFunction, source)
    }

    #[test]
    fn an_expression_body_returns_nothing() {
        assert_eq!(
            mutate("Item { x: xs.map(v => v * 2) }"),
            ["Item { x: xs.map(v => undefined) }"]
        );
    }

    #[test]
    fn block_bodies_and_undefined_bodies_are_left_alone() {
        assert!(mutate("Item { x: xs.map(v => { return v }) }").is_empty());
        assert!(mutate("Item { x: xs.map(v => undefined) }").is_empty());
    }
}
