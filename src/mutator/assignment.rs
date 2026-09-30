use tree_sitter::Node;

use super::{Replacement, is_string, swap_operator};

pub(super) fn mutate(node: Node, source: &str) -> Vec<Replacement> {
    let appends_text = is_string(node.child_by_field_name("right"));
    swap_operator(
        node,
        source,
        "augmented_assignment_expression",
        if appends_text { textual } else { numeric },
    )
}

fn numeric(operator: &str) -> &'static [&'static str] {
    match operator {
        "+=" => &["-="],
        "-=" => &["+="],
        "*=" => &["/="],
        "/=" | "%=" => &["*="],
        _ => textual(operator),
    }
}

fn textual(operator: &str) -> &'static [&'static str] {
    match operator {
        "&&=" => &["||="],
        "||=" | "??=" => &["&&="],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use crate::mutator::Mutator;
    use crate::mutator::tests::render_one;

    fn function(body: &str) -> String {
        format!("Item {{ function f() {{ {body} }} }}")
    }

    fn mutate(body: &str) -> Vec<String> {
        render_one(Mutator::AssignmentOperator, &function(body))
    }

    #[test]
    fn arithmetic_assignments_swap() {
        assert_eq!(mutate("a += 1"), [function("a -= 1")]);
        assert_eq!(mutate("a -= 1"), [function("a += 1")]);
        assert_eq!(mutate("a *= 2"), [function("a /= 2")]);
        assert_eq!(mutate("a /= 2"), [function("a *= 2")]);
        assert_eq!(mutate("a %= 2"), [function("a *= 2")]);
    }

    #[test]
    fn logical_assignments_swap() {
        assert_eq!(mutate("a &&= b"), [function("a ||= b")]);
        assert_eq!(mutate("a ||= b"), [function("a &&= b")]);
        assert_eq!(mutate("a ??= b"), [function("a &&= b")]);
    }

    #[test]
    fn appending_text_only_mutates_logical_assignments() {
        assert!(mutate(r#"a += "x""#).is_empty());
        assert_eq!(mutate(r#"a ||= "x""#), [function(r#"a &&= "x""#)]);
    }

    #[test]
    fn plain_assignment_is_left_alone() {
        assert!(mutate("a = 1").is_empty());
    }
}
