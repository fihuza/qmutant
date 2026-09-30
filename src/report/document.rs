use std::collections::BTreeMap;

use serde::Serialize;

use super::Report;
use crate::mutant::{Position, Status};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'a> {
    schema_version: &'static str,
    thresholds: Thresholds,
    files: BTreeMap<String, File<'a>>,
    framework: Framework,
}

#[derive(Serialize)]
struct Thresholds {
    high: u8,
    low: u8,
}

#[derive(Serialize)]
struct Framework {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct File<'a> {
    language: &'static str,
    source: &'a str,
    mutants: Vec<Mutant<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Mutant<'a> {
    id: String,
    mutator_name: &'static str,
    replacement: &'a str,
    location: Location,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_reason: Option<&'a str>,
}

#[derive(Serialize)]
struct Location {
    start: Position,
    end: Position,
}

fn status_name(status: Status) -> &'static str {
    match status {
        Status::Killed => "Killed",
        Status::Survived => "Survived",
        Status::Timeout => "Timeout",
        Status::Invalid => "CompileError",
        Status::Error => "RuntimeError",
        Status::Ignored => "Ignored",
    }
}

fn document<'a>(report: &'a Report) -> Document<'a> {
    let files = report
        .plan
        .files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let mutants = report
                .plan
                .mutants
                .iter()
                .filter(|mutant| mutant.file == index)
                .map(|mutant| Mutant {
                    id: mutant.id.to_string(),
                    mutator_name: mutant.mutator.name(),
                    replacement: &mutant.replacement,
                    location: Location {
                        start: mutant.start,
                        end: mutant.end,
                    },
                    status: status_name(report.statuses[mutant.id]),
                    status_reason: report.reasons[mutant.id].as_deref(),
                })
                .collect();
            (
                file.path.to_string_lossy().into_owned(),
                File {
                    language: "qml",
                    source: &file.source,
                    mutants,
                },
            )
        })
        .collect();
    Document {
        schema_version: "2",
        thresholds: Thresholds {
            high: report.thresholds.high,
            low: report.thresholds.low,
        },
        files,
        framework: Framework {
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
        },
    }
}

#[must_use]
pub fn json(report: &Report) -> String {
    serde_json::to_string_pretty(&document(report))
        .expect("the report holds only strings and numbers")
}

#[must_use]
pub fn toml(report: &Report) -> String {
    ::toml::to_string_pretty(&document(report))
        .expect("the report holds only strings, numbers and tables")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Thresholds;
    use crate::mutant::Verdict;
    use crate::mutator::Mutator;
    use crate::report::fixtures::plan;

    #[test]
    fn hostile_test_output_survives_both_formats_unchanged() {
        let plan = plan("Item { x: a && b; y: c && d }", Mutator::LogicalOperator);
        let output = "quotes \"\"\" and ''' and \\ and \t and \u{1} and é\nsecond line";
        let verdicts = vec![
            Verdict {
                mutant: 0,
                status: Status::Killed,
                reason: Some(output.to_owned()),
            },
            Verdict {
                mutant: 1,
                status: Status::Survived,
                reason: None,
            },
        ];
        let report = Report::new(&plan, verdicts, Thresholds::default());
        let from_json: serde_json::Value = serde_json::from_str(&json(&report)).unwrap();
        let from_toml: serde_json::Value = ::toml::from_str(&toml(&report)).unwrap();
        assert_eq!(from_toml, from_json);
        assert_eq!(
            from_toml["files"]["A.qml"]["mutants"][0]["statusReason"],
            output
        );
        assert!(
            from_toml["files"]["A.qml"]["mutants"][1]
                .get("statusReason")
                .is_none()
        );
    }
}
