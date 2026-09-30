use tree_sitter::Node;

use super::{Replacement, text};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    if !matches!(node.kind(), "string" | "template_string") || is_key(node) {
        return Vec::new();
    }
    let literal = text(node, source);
    let quote = &literal[..1];
    let replacement = if literal.len() == 2 {
        format!("{quote}qmutant{quote}")
    } else {
        format!("{quote}{quote}")
    };
    vec![Replacement::new(node, replacement)]
}

fn is_key(node: Node) -> bool {
    node.parent()
        .and_then(|parent| parent.child_by_field_name("key"))
        .is_some_and(|key| key == node)
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::StringLiteral, source)
    }

    #[test]
    fn text_is_emptied_keeping_its_quotes() {
        assert_eq!(mutate(r#"Item { x: "a" }"#), [r#"Item { x: "" }"#]);
        assert_eq!(mutate("Item { x: 'a' }"), ["Item { x: '' }"]);
        assert_eq!(mutate("Item { x: `a${b}` }"), ["Item { x: `` }"]);
    }

    #[test]
    fn empty_text_is_filled() {
        assert_eq!(mutate(r#"Item { x: "" }"#), [r#"Item { x: "qmutant" }"#]);
    }

    #[test]
    fn object_keys_are_left_alone() {
        assert_eq!(
            mutate(r#"Item { x: ({ "k": "v" }) }"#),
            [r#"Item { x: ({ "k": "" }) }"#]
        );
    }
}
