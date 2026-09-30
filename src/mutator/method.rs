use tree_sitter::Node;

use super::{Replacement, text};

const SWAPS: [(&str, &str); 12] = [
    ("endsWith", "startsWith"),
    ("startsWith", "endsWith"),
    ("every", "some"),
    ("some", "every"),
    ("max", "min"),
    ("min", "max"),
    ("toLowerCase", "toUpperCase"),
    ("toUpperCase", "toLowerCase"),
    ("toLocaleLowerCase", "toLocaleUpperCase"),
    ("toLocaleUpperCase", "toLocaleLowerCase"),
    ("trimEnd", "trimStart"),
    ("trimStart", "trimEnd"),
];

const REMOVABLE: [&str; 8] = [
    "charAt",
    "filter",
    "reverse",
    "slice",
    "sort",
    "substr",
    "substring",
    "trim",
];

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    if node.kind() != "call_expression" {
        return Vec::new();
    }
    let Some(member) = node
        .child_by_field_name("function")
        .filter(|function| function.kind() == "member_expression")
    else {
        return Vec::new();
    };
    let (Some(object), Some(property)) = (
        member.child_by_field_name("object"),
        member.child_by_field_name("property"),
    ) else {
        return Vec::new();
    };
    let name = text(property, source);
    if let Some((_, swapped)) = SWAPS.iter().find(|(from, _)| *from == name) {
        vec![Replacement::new(property, *swapped)]
    } else if REMOVABLE.contains(&name) {
        vec![Replacement::new(node, text(object, source))]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn mutate(source: &str) -> Vec<String> {
        render_one(Mutator::MethodExpression, source)
    }

    #[test]
    fn paired_methods_swap() {
        assert_eq!(
            mutate("Item { x: s.startsWith(p) }"),
            ["Item { x: s.endsWith(p) }"]
        );
        assert_eq!(
            mutate("Item { x: Math.max(a, b) }"),
            ["Item { x: Math.min(a, b) }"]
        );
        assert_eq!(
            mutate("Item { x: xs.every(f) }"),
            ["Item { x: xs.some(f) }"]
        );
    }

    #[test]
    fn a_transforming_call_is_removed() {
        assert_eq!(mutate("Item { x: s.trim() }"), ["Item { x: s }"]);
        assert_eq!(mutate("Item { x: a.b.filter(f) }"), ["Item { x: a.b }"]);
    }

    #[test]
    fn other_calls_are_left_alone() {
        assert!(mutate("Item { x: s.push(1) }").is_empty());
        assert!(mutate("Item { x: trim(s) }").is_empty());
    }
}
