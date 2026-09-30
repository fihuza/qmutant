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

#[must_use]
pub fn render(report: &Report) -> String {
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
    let document = Document {
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
    };
    serde_json::to_string_pretty(&document).expect("the report holds only strings and numbers")
}
