use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::{Config, Overrides};
use crate::directive::Directives;
use crate::mutant::{Lines, Mutant, Position};
use crate::mutator::mutations;
use crate::parse;

fn mutate(source: &str) {
    let Some(tree) = parse::parse(source) else {
        return;
    };
    let lines = Lines::new(source);
    for mutation in mutations(&tree, source, &[]) {
        let range = mutation.range.clone();
        assert!(
            range.start <= range.end && range.end <= source.len(),
            "{mutation:?} is outside the source"
        );
        assert!(
            source.is_char_boundary(range.start) && source.is_char_boundary(range.end),
            "{mutation:?} splits a character"
        );
        assert_ne!(
            &source[range.clone()],
            mutation.replacement,
            "{mutation:?} changes nothing"
        );
        let start = Position::at(source, range.start);
        let end = Position::at(source, range.end);
        assert!(
            start.line >= 1
                && start.column >= 1
                && (start.line, start.column) <= (end.line, end.column)
        );
        assert_eq!(
            (lines.position(range.start), lines.position(range.end)),
            (start, end)
        );
        let mutant = Mutant {
            id: 0,
            file: 0,
            mutator: mutation.mutator,
            range,
            start,
            end,
            replacement: mutation.replacement,
        };
        let mutated = mutant.apply(source);
        let from_scratch = parse::parse(&mutated).is_some_and(|tree| !tree.root_node().has_error());
        assert_eq!(
            parse::is_valid_edit(&tree, &mutant.edit(&lines), &mutated),
            from_scratch,
            "{mutant:?}: the incremental re-parse disagrees with a full one"
        );
    }
}

fn directive(body: &str) {
    let source = format!("Item {{\n    // qmutant: {body}\n    x: a && b\n}}\n");
    let Some(tree) = parse::parse(&source) else {
        return;
    };
    if let Ok(mut directives) = Directives::parse(&tree, &source, Path::new("A.qml")) {
        for mutation in mutations(&tree, &source, &[]) {
            let line = Position::at(&source, mutation.range.start).line;
            directives.ignores(mutation.mutator, mutation.range.start, line);
        }
        directives.unused().count();
    }
}

fn config(text: &str) {
    if let Ok(config) = Config::parse(
        text,
        Path::new("qmutant.toml"),
        PathBuf::from("/"),
        Overrides::default(),
    ) {
        assert!(!config.command.trim().is_empty() && !config.mutate.is_empty());
        assert!(config.thresholds.low <= config.thresholds.high && config.thresholds.high <= 100);
        assert!(config.timeout.factor >= 1.0);
        for baseline in [Duration::ZERO, Duration::from_secs(4), Duration::MAX] {
            assert!(
                config.timeout.for_baseline(baseline) >= Duration::from_millis(config.timeout.ms)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::mutator::Mutator;

    const TOKENS: [&str; 40] = [
        "a",
        "b.c",
        "f(x)",
        "0",
        "1.5",
        "\"s\"",
        "''",
        "`t${u}`",
        "true",
        "false",
        "null",
        "undefined",
        "[",
        "]",
        "{",
        "}",
        "(",
        ")",
        "?.",
        "?",
        ":",
        ";",
        ",",
        "=>",
        "!",
        "-",
        "+",
        "*",
        "/",
        "%",
        "<",
        "<=",
        ">=",
        "===",
        "!==",
        "&&",
        "||",
        "??",
        "++",
        "+=",
    ];

    fn javascript() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::sample::select(&TOKENS[..]), 0..24)
            .prop_map(|tokens| tokens.join(" "))
    }

    fn component() -> impl Strategy<Value = String> {
        (javascript(), javascript(), javascript()).prop_map(|(binding, body, handler)| {
            format!(
                "import QtQuick\nItem {{\n    id: root\n    x: {binding}\n    function f(v) {{ {body} }}\n    onClicked: {{ {handler} }}\n}}\n"
            )
        })
    }

    fn directive_body() -> impl Strategy<Value = String> {
        let words: Vec<String> = [
            "disable",
            "enable",
            "next-line",
            "all",
            "--",
            "reason",
            ",",
            "Nope",
        ]
        .into_iter()
        .map(str::to_owned)
        .chain(Mutator::ALL.iter().map(ToString::to_string))
        .collect();
        prop::collection::vec(prop::sample::select(words), 0..8).prop_map(|words| words.join(" "))
    }

    const CONFIG_LINES: [&str; 17] = [
        "command = \"t\"",
        "command = \"\"",
        "mutate = []",
        "mutate = [\"*.qml\"]",
        "jobs = 0",
        "jobs = 4",
        "jobs = \"50%\"",
        "jobs = \"150%\"",
        "timeout = { ms = 1, factor = 0.5 }",
        "timeout = { ms = 1, factor = 2.0 }",
        "timeout = { ms = 18446744073709551615, factor = 1e300 }",
        "thresholds = { high = 10, low = 90 }",
        "thresholds = { high = 90, low = 10, break = 200 }",
        "reporters = [\"json\"]",
        "sandbox_dir = \"../x\"",
        "exclude_mutators = [\"StringLiteral\"]",
        "unknown = 1",
    ];

    fn config_text() -> impl Strategy<Value = String> {
        prop::collection::vec(prop::sample::select(&CONFIG_LINES[..]), 0..6)
            .prop_map(|lines| lines.join("\n"))
    }

    proptest! {
        #[test]
        fn any_text_yields_mutants_that_stay_inside_it(source in any::<String>()) {
            mutate(&source);
        }

        #[test]
        fn any_component_yields_mutants_that_stay_inside_it(source in component()) {
            mutate(&source);
        }

        #[test]
        fn any_directive_is_accepted_or_refused_without_panicking(body in directive_body()) {
            directive(&body);
        }

        #[test]
        fn any_directive_text_is_accepted_or_refused_without_panicking(body in any::<String>()) {
            directive(&body);
        }

        #[test]
        fn an_accepted_config_is_always_valid(text in config_text()) {
            config(&text);
        }

        #[test]
        fn any_config_text_is_accepted_or_refused_without_panicking(text in any::<String>()) {
            config(&text);
        }
    }
}
