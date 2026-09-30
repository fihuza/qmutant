use tree_sitter::Node;

use super::Replacement;

pub(super) fn mutate(node: Node) -> Vec<Replacement> {
    if node.kind() == "object" && node.named_child_count() > 0 {
        vec![Replacement::new(node, "{}")]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::ObjectLiteral, source)
    }

    #[test]
    fn properties_are_removed() {
        assert_eq!(mutate("Item { x: ({ a: 1 }) }"), ["Item { x: ({}) }"]);
    }

    #[test]
    fn an_empty_object_is_left_alone() {
        assert!(mutate("Item { x: ({}) }").is_empty());
        assert!(mutate("Item { x: ({ }) }").is_empty());
    }
}
