use tree_sitter::Node;

use super::Replacement;

pub(super) fn mutate(node: Node) -> Vec<Replacement> {
    let outcomes: &[&str] = match node.kind() {
        "if_statement" | "ternary_expression" => &["true", "false"],
        "while_statement" | "do_statement" | "for_statement" => &["false"],
        _ => return Vec::new(),
    };
    let Some(condition) = node.child_by_field_name("condition") else {
        return Vec::new();
    };
    outcomes
        .iter()
        .map(|outcome| Replacement::new(condition, shaped_like(condition, outcome)))
        .collect()
}

fn shaped_like(condition: Node, outcome: &str) -> String {
    match condition.kind() {
        "parenthesized_expression" => format!("({outcome})"),
        "expression_statement" | "empty_statement" => format!("{outcome};"),
        _ => outcome.to_owned(),
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
        render_one(Mutator::ConditionalExpression, &function(body))
    }

    #[test]
    fn an_if_is_forced_both_ways() {
        assert_eq!(
            mutate("if (a) b()"),
            [function("if (true) b()"), function("if (false) b()")]
        );
    }

    #[test]
    fn a_ternary_is_forced_both_ways() {
        assert_eq!(
            render_one(Mutator::ConditionalExpression, "Item { x: a ? 1 : 2 }"),
            ["Item { x: true ? 1 : 2 }", "Item { x: false ? 1 : 2 }"]
        );
    }

    #[test]
    fn loops_are_forced_never_to_run() {
        assert_eq!(mutate("while (a) b()"), [function("while (false) b()")]);
        assert_eq!(
            mutate("do { b() } while (a)"),
            [function("do { b() } while (false)")]
        );
        assert_eq!(
            mutate("for (let i = 0; i < n; i++) b()"),
            [function("for (let i = 0; false; i++) b()")]
        );
        assert_eq!(mutate("for (;;) b()"), [function("for (;false;) b()")]);
    }

    #[test]
    fn a_condition_that_is_already_a_literal_is_only_forced_the_other_way() {
        assert_eq!(mutate("if (true) b()"), [function("if (false) b()")]);
        assert_eq!(mutate("while (false) b()"), Vec::<String>::new());
        assert_eq!(
            render_one(Mutator::ConditionalExpression, "Item { x: false ? 1 : 2 }"),
            ["Item { x: true ? 1 : 2 }"]
        );
    }

    #[test]
    fn a_for_of_loop_has_no_condition() {
        assert!(mutate("for (const x of xs) b(x)").is_empty());
    }
}
