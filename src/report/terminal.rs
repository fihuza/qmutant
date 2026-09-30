use anstyle::{AnsiColor, Style};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};

use super::Report;
use crate::instrument::Plan;
use crate::mutant::{Mutant, Status};
use crate::score::{Band, Counts};

const SHOWN_LINES: usize = 4;
const REMOVED: Style = AnsiColor::Red.on_default();
const ADDED: Style = AnsiColor::Green.on_default();
const HEADING: Style = Style::new().bold();

#[must_use]
pub fn render(report: &Report) -> String {
    let mut out: String = report
        .plan
        .mutants
        .iter()
        .filter(|mutant| report.statuses[mutant.id] == Status::Survived)
        .map(|mutant| survivor(report, mutant))
        .collect();
    out.push_str(&table(report).to_string());
    out.push('\n');
    out.push_str(&score_line(report.counts(), report));
    out
}

#[must_use]
pub fn listing(plan: &Plan) -> String {
    plan.mutants
        .iter()
        .map(|mutant| {
            let file = &plan.files[mutant.file];
            let status = match plan.decided[mutant.id] {
                Some(Status::Ignored) => " (ignored)",
                Some(Status::Invalid) => " (invalid)",
                _ => "",
            };
            format!(
                "{}  {}:{}:{}  {}{status}  `{}` -> `{}`\n",
                mutant.id,
                file.path.display(),
                mutant.start.line,
                mutant.start.column,
                mutant.mutator,
                one_line(&file.source[mutant.range.clone()]),
                one_line(&mutant.replacement),
            )
        })
        .collect::<Vec<_>>()
        .concat()
}

fn one_line(text: &str) -> String {
    let mut words = text.split_whitespace();
    let first = words.next().unwrap_or_default().to_owned();
    words.fold(first, |line, word| line + " " + word)
}

fn survivor(report: &Report, mutant: &Mutant) -> String {
    let file = &report.plan.files[mutant.file];
    let source = &file.source;
    let line_start = source[..mutant.range.start]
        .rfind('\n')
        .map_or(0, |newline| newline + 1);
    let line_end = source[mutant.range.end..]
        .find('\n')
        .map_or(source.len(), |newline| mutant.range.end + newline);
    let original = &source[line_start..line_end];
    let mutated = format!(
        "{}{}{}",
        &source[line_start..mutant.range.start],
        mutant.replacement,
        &source[mutant.range.end..line_end]
    );
    format!(
        "{HEADING}Survived{HEADING:#}  {}:{}:{}  {}\n{}{}\n",
        file.path.display(),
        mutant.start.line,
        mutant.start.column,
        mutant.mutator,
        lines(original, '-', REMOVED),
        lines(&mutated, '+', ADDED),
    )
}

fn lines(text: &str, sign: char, style: Style) -> String {
    let all: Vec<&str> = text.lines().collect();
    let shown = all.iter().take(SHOWN_LINES).map(|line| line.trim_start());
    let elided = (all.len() > SHOWN_LINES).then_some("…");
    shown
        .chain(elided)
        .map(|line| format!("  {style}{sign} {line}{style:#}\n"))
        .collect::<Vec<_>>()
        .concat()
}

fn table(report: &Report) -> Table {
    let mut table = Table::new();
    table.load_style(UTF8_FULL_CONDENSED).set_header([
        "File", "Score", "Killed", "Timeout", "Survived", "Invalid", "Error", "Ignored",
    ]);
    for (index, file) in report.plan.files.iter().enumerate() {
        table.add_row(row(
            file.path.display().to_string(),
            report.counts_for(index),
        ));
    }
    table.add_row(row("All files".to_owned(), report.counts()));
    table
}

fn row(name: String, counts: Counts) -> [String; 8] {
    [
        name,
        percent(counts.score()),
        counts.killed.to_string(),
        counts.timeout.to_string(),
        counts.survived.to_string(),
        counts.invalid.to_string(),
        counts.error.to_string(),
        counts.ignored.to_string(),
    ]
}

fn percent(score: Option<f64>) -> String {
    score.map_or_else(|| "n/a".to_owned(), |score| format!("{score:.2}"))
}

fn score_line(counts: Counts, report: &Report) -> String {
    let style = match counts.band(report.thresholds) {
        Band::High => AnsiColor::Green.on_default(),
        Band::Low => AnsiColor::Yellow.on_default(),
        Band::Failing => AnsiColor::Red.on_default(),
    }
    .bold();
    let verdict = match (counts.score(), report.thresholds.minimum) {
        (Some(_), Some(minimum)) if !counts.meets(report.thresholds) => {
            format!(", below the break threshold of {minimum}")
        }
        _ => String::new(),
    };
    format!(
        "{style}Mutation score: {}{}{verdict}{style:#}\n",
        percent(counts.score()),
        if counts.score().is_some() { "%" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Thresholds;
    use crate::mutant::Verdict;
    use crate::mutator::Mutator;
    use crate::report::fixtures::plan;

    fn survivors(plan: &Plan) -> Vec<Verdict> {
        plan.mutants
            .iter()
            .map(|mutant| Verdict {
                mutant: mutant.id,
                status: Status::Survived,
                reason: None,
            })
            .collect()
    }

    fn plain(text: &str) -> String {
        anstream::adapter::strip_str(text).to_string()
    }

    #[test]
    fn a_survivor_shows_its_whole_line_before_and_after() {
        let plan = plan("Item {\n    x: a < b && c\n}\n", Mutator::LogicalOperator);
        let report = Report::new(&plan, survivors(&plan), Thresholds::default());
        assert!(plain(&render(&report)).starts_with(
            "Survived  A.qml:2:14  LogicalOperator\n  - x: a < b && c\n  + x: a < b || c\n\n"
        ));
    }

    #[test]
    fn a_long_body_is_cut_after_four_lines() {
        let four = "Item {\n    function f() {\n        a()\n        b()\n    }\n}\n";
        let plan_four = plan(four, Mutator::BlockStatement);
        let shown = plain(&render(&Report::new(
            &plan_four,
            survivors(&plan_four),
            Thresholds::default(),
        )));
        assert!(shown.contains("  - }\n  + function f() {}\n"), "{shown}");
        assert!(!shown.contains('…'));
        let five = "Item {\n    function f() {\n        a()\n        b()\n        c()\n    }\n}\n";
        let plan_five = plan(five, Mutator::BlockStatement);
        let shown = plain(&render(&Report::new(
            &plan_five,
            survivors(&plan_five),
            Thresholds::default(),
        )));
        assert!(shown.contains("  - c()\n  - …\n"), "{shown}");
    }

    #[test]
    fn the_score_line_mentions_break_only_when_it_is_missed() {
        let plan = plan("Item { x: a && b; y: c && d }", Mutator::LogicalOperator);
        let verdicts = vec![
            Verdict {
                mutant: 0,
                status: Status::Killed,
                reason: None,
            },
            Verdict {
                mutant: 1,
                status: Status::Survived,
                reason: None,
            },
        ];
        let at = |minimum| {
            let thresholds = Thresholds {
                minimum: Some(minimum),
                ..Thresholds::default()
            };
            plain(&render(&Report::new(&plan, verdicts.clone(), thresholds)))
        };
        assert!(at(50).ends_with("Mutation score: 50.00%\n"));
        assert!(at(51).ends_with("Mutation score: 50.00%, below the break threshold of 51\n"));
    }

    #[test]
    fn nothing_to_score_says_so() {
        let plan = plan("Item { x: a }", Mutator::LogicalOperator);
        let report = Report::new(&plan, Vec::new(), Thresholds::default());
        assert!(plain(&render(&report)).ends_with("Mutation score: n/a\n"));
    }

    #[test]
    fn the_listing_marks_what_will_not_run() {
        let mut plan = plan(
            "Item { x: a && b; y: c && d; z: e && f }",
            Mutator::LogicalOperator,
        );
        plan.decided = vec![None, Some(Status::Ignored), Some(Status::Invalid)];
        assert_eq!(
            listing(&plan),
            "0  A.qml:1:13  LogicalOperator  `&&` -> `||`\n\
             1  A.qml:1:24  LogicalOperator (ignored)  `&&` -> `||`\n\
             2  A.qml:1:35  LogicalOperator (invalid)  `&&` -> `||`\n"
        );
    }
}
