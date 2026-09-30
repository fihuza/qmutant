use tree_sitter::Node;

use super::Replacement;

pub(super) fn mutate(node: Node) -> Vec<Replacement> {
    let parent = node.parent().map(|parent| parent.kind());
    match (node.kind(), parent) {
        ("optional_chain", Some("member_expression")) => vec![Replacement::new(node, ".")],
        ("optional_chain", _) | ("?.", Some("call_expression")) => {
            vec![Replacement::new(node, "")]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::OptionalChaining, source)
    }

    #[test]
    fn member_access_is_no_longer_guarded() {
        assert_eq!(mutate("Item { x: a?.b }"), ["Item { x: a.b }"]);
    }

    #[test]
    fn subscripts_and_calls_are_no_longer_guarded() {
        assert_eq!(mutate("Item { x: a?.[1] }"), ["Item { x: a[1] }"]);
        assert_eq!(mutate("Item { x: f?.() }"), ["Item { x: f() }"]);
    }
}
