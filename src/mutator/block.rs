use tree_sitter::Node;

use super::Replacement;

pub(super) fn mutate(node: Node) -> Vec<Replacement> {
    let mut cursor = node.walk();
    let has_statements = node
        .named_children(&mut cursor)
        .any(|child| child.kind() != "comment");
    if node.kind() == "statement_block" && has_statements {
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
        render_one(Mutator::BlockStatement, source)
    }

    #[test]
    fn a_body_is_emptied() {
        assert_eq!(
            mutate("Item { function f() { g() } }"),
            ["Item { function f() {} }"]
        );
    }

    #[test]
    fn a_handler_body_is_emptied() {
        assert_eq!(
            mutate("Item { onClicked: { g() } }"),
            ["Item { onClicked: {} }"]
        );
    }

    #[test]
    fn a_body_holding_only_comments_is_left_alone() {
        assert!(mutate("Item { function f() { /* nothing */ } }").is_empty());
        assert!(mutate("Item { function f() {} }").is_empty());
    }
}
