use std::ops::ControlFlow;
use std::time::{Duration, Instant};

use tree_sitter::{ParseOptions, ParseState, Parser, Tree};

const BUDGET: Duration = Duration::from_secs(5);

#[must_use]
pub fn parse(source: &str) -> Option<Tree> {
    parse_within(source, BUDGET)
}

#[must_use]
pub fn is_valid(source: &str) -> bool {
    parse(source).is_some_and(|tree| !tree.root_node().has_error())
}

fn parse_within(source: &str, budget: Duration) -> Option<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_qmljs::LANGUAGE.into())
        .expect("the bundled QML grammar matches the linked tree-sitter ABI");
    let started = Instant::now();
    let mut within_budget = |_: &ParseState| {
        if started.elapsed() < budget {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(())
        }
    };
    let bytes = source.as_bytes();
    parser.parse_with_options(
        &mut |offset, _| &bytes[offset.min(bytes.len())..],
        None,
        Some(ParseOptions::new().progress_callback(&mut within_budget)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_real_component_parses_without_errors() {
        assert!(is_valid(include_str!(
            "../tests/fixtures/components/Everything.qml"
        )));
    }

    #[test]
    fn broken_syntax_is_detected() {
        assert!(!is_valid("Item { x: a < }"));
        assert!(!is_valid("Item { function f() { if (true b() } }"));
    }

    #[test]
    fn a_source_that_sends_the_grammar_into_a_loop_is_given_up_on() {
        let started = Instant::now();
        assert!(parse_within("m{x:[a=>{a''}}{u}`a", Duration::from_millis(200)).is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
