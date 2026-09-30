use tree_sitter::Node;

use super::{Replacement, text};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    match node.kind() {
        "true" => vec![Replacement::new(node, "false")],
        "false" => vec![Replacement::new(node, "true")],
        "unary_expression" => negated_argument(node, source)
            .map(|argument| vec![Replacement::new(node, argument)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn negated_argument<'a>(node: Node, source: &'a str) -> Option<&'a str> {
    let operator = node.child_by_field_name("operator")?;
    let argument = node.child_by_field_name("argument")?;
    (text(operator, source) == "!").then(|| text(argument, source))
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::BooleanLiteral, source)
    }

    #[test]
    fn literals_flip() {
        assert_eq!(mutate("Item { x: true }"), ["Item { x: false }"]);
        assert_eq!(mutate("Item { x: false }"), ["Item { x: true }"]);
    }

    #[test]
    fn a_negation_is_dropped() {
        assert_eq!(mutate("Item { x: !a.b }"), ["Item { x: a.b }"]);
    }

    #[test]
    fn a_sign_is_not_a_negation() {
        assert!(mutate("Item { x: -a }").is_empty());
    }
}
