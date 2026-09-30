use tree_sitter::Node;

use super::Replacement;

pub(super) fn mutate(node: Node) -> Vec<Replacement> {
    if node.kind() == "array" && node.named_child_count() > 0 {
        vec![Replacement::new(node, "[]")]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::ArrayDeclaration, source)
    }

    #[test]
    fn elements_are_removed() {
        assert_eq!(mutate("Item { x: [1, 2] }"), ["Item { x: [] }"]);
    }

    #[test]
    fn an_empty_array_is_left_alone() {
        assert!(mutate("Item { x: [] }").is_empty());
        assert!(mutate("Item { x: [ ] }").is_empty());
    }
}
