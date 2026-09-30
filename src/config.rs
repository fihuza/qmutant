use std::fmt;
use std::num::NonZeroUsize;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

use serde::Deserialize;

use crate::error::{self, Error};
use crate::mutator::Mutator;

pub const DEFAULT_FILE: &str = "qmutant.toml";

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip)]
    pub root: PathBuf,
    #[serde(default = "default_mutate")]
    pub mutate: Vec<String>,
    pub command: String,
    #[serde(default)]
    pub jobs: Jobs,
    #[serde(default)]
    pub timeout: Timeout,
    #[serde(default)]
    pub thresholds: Thresholds,
    #[serde(default = "default_reporters")]
    pub reporters: Vec<Reporter>,
    #[serde(default = "default_sandbox_dir")]
    pub sandbox_dir: PathBuf,
    #[serde(default)]
    pub ignore: Vec<String>,
    #[serde(default)]
    pub exclude_mutators: Vec<Mutator>,
}

#[derive(Clone, Debug, Default)]
pub struct Overrides {
    pub mutate: Option<Vec<String>>,
    pub jobs: Option<Jobs>,
    pub timeout_ms: Option<u64>,
    pub timeout_factor: Option<f64>,
    pub reporters: Option<Vec<Reporter>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Timeout {
    pub ms: u64,
    pub factor: f64,
}

impl Default for Timeout {
    fn default() -> Self {
        Self {
            ms: 5000,
            factor: 1.5,
        }
    }
}

impl Timeout {
    #[must_use]
    pub fn for_baseline(self, baseline: Duration) -> Duration {
        baseline.mul_f64(self.factor) + Duration::from_millis(self.ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Thresholds {
    pub high: u8,
    pub low: u8,
    #[serde(rename = "break")]
    pub minimum: Option<u8>,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            high: 80,
            low: 60,
            minimum: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawJobs")]
pub enum Jobs {
    Count(NonZeroUsize),
    Percent(u8),
}

impl Default for Jobs {
    fn default() -> Self {
        Self::Percent(50)
    }
}

impl Jobs {
    #[must_use]
    pub fn resolve(self, cpus: NonZeroUsize) -> NonZeroUsize {
        match self {
            Self::Count(count) => count,
            Self::Percent(percent) => NonZeroUsize::new(cpus.get() * usize::from(percent) / 100)
                .unwrap_or(NonZeroUsize::MIN),
        }
    }
}

impl FromStr for Jobs {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let invalid = || format!("`{text}` is neither a worker count nor a percentage like `50%`");
        if let Some(percent) = text.strip_suffix('%') {
            match percent.parse::<u8>() {
                Ok(percent @ 1..=100) => Ok(Self::Percent(percent)),
                _ => Err(invalid()),
            }
        } else {
            text.parse().map(Self::Count).map_err(|_| invalid())
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawJobs {
    Count(usize),
    Text(String),
}

impl TryFrom<RawJobs> for Jobs {
    type Error = String;

    fn try_from(raw: RawJobs) -> Result<Self, Self::Error> {
        match raw {
            RawJobs::Count(count) => count.to_string().parse(),
            RawJobs::Text(text) => text.parse(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Reporter {
    Terminal,
    Progress,
    Json,
    Toml,
    Html,
}

impl FromStr for Reporter {
    type Err = String;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "terminal" => Ok(Self::Terminal),
            "progress" => Ok(Self::Progress),
            "json" => Ok(Self::Json),
            "toml" => Ok(Self::Toml),
            "html" => Ok(Self::Html),
            _ => Err(format!(
                "unknown reporter `{name}`; expected terminal, progress, json, toml or html"
            )),
        }
    }
}

impl fmt::Display for Reporter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Terminal => "terminal",
            Self::Progress => "progress",
            Self::Json => "json",
            Self::Toml => "toml",
            Self::Html => "html",
        })
    }
}

fn default_mutate() -> Vec<String> {
    vec!["**/*.qml".to_owned()]
}

fn default_reporters() -> Vec<Reporter> {
    vec![Reporter::Terminal, Reporter::Progress, Reporter::Html]
}

fn default_sandbox_dir() -> PathBuf {
    PathBuf::from(".qmutant")
}

impl Config {
    pub fn load(path: &Path, overrides: Overrides) -> Result<Self, Error> {
        let text = std::fs::read_to_string(path).map_err(error::at(path))?;
        let directory = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let directory = directory.unwrap_or(Path::new("."));
        let root = directory.canonicalize().map_err(error::at(directory))?;
        Self::parse(&text, path, root, overrides)
    }

    pub fn parse(
        text: &str,
        path: &Path,
        root: PathBuf,
        overrides: Overrides,
    ) -> Result<Self, Error> {
        let mut config: Self = toml::from_str(text).map_err(|error| Error::Config {
            path: path.to_owned(),
            message: error.message().to_owned() + &location(text, error.span()),
        })?;
        config.root = root;
        config.apply(overrides);
        config.validate().map_err(|message| Error::Config {
            path: path.to_owned(),
            message,
        })?;
        Ok(config)
    }

    fn apply(&mut self, overrides: Overrides) {
        if let Some(mutate) = overrides.mutate {
            self.mutate = mutate;
        }
        if let Some(jobs) = overrides.jobs {
            self.jobs = jobs;
        }
        if let Some(ms) = overrides.timeout_ms {
            self.timeout.ms = ms;
        }
        if let Some(factor) = overrides.timeout_factor {
            self.timeout.factor = factor;
        }
        if let Some(reporters) = overrides.reporters {
            self.reporters = reporters;
        }
    }

    fn validate(&self) -> Result<(), String> {
        let Thresholds { high, low, minimum } = self.thresholds;
        if self.command.trim().is_empty() {
            Err("`command` is empty; set it to the command that runs your tests".to_owned())
        } else if self.mutate.is_empty() {
            Err("`mutate` is empty; list the QML files to mutate".to_owned())
        } else if !(low <= high && high <= 100) {
            Err(format!(
                "thresholds must satisfy low <= high <= 100, got low = {low}, high = {high}"
            ))
        } else if minimum.is_some_and(|minimum| minimum > 100) {
            Err("`thresholds.break` must be at most 100".to_owned())
        } else if !(self.timeout.factor.is_finite() && self.timeout.factor >= 1.0) {
            Err(format!(
                "`timeout.factor` must be at least 1, got {}",
                self.timeout.factor
            ))
        } else if !is_inside(&self.sandbox_dir) {
            Err(format!(
                "`sandbox_dir` must be a relative path inside the project, got `{}`",
                self.sandbox_dir.display()
            ))
        } else {
            Ok(())
        }
    }
}

fn is_inside(path: &Path) -> bool {
    path.components().next().is_some()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn location(text: &str, span: Option<std::ops::Range<usize>>) -> String {
    span.map(|span| {
        let position = crate::mutant::Position::at(text, span.start);
        format!(" (line {}, column {})", position.line, position.column)
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str, overrides: Overrides) -> Result<Config, String> {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(DEFAULT_FILE);
        std::fs::write(&path, text).unwrap();
        Config::load(&path, overrides).map_err(|error| {
            error
                .to_string()
                .replace(&path.display().to_string(), "qmutant.toml")
        })
    }

    #[test]
    fn a_minimal_file_takes_every_default() {
        let config = load(r#"command = "make test""#, Overrides::default()).unwrap();
        assert_eq!(config.mutate, ["**/*.qml"]);
        assert_eq!(config.jobs, Jobs::Percent(50));
        assert_eq!(config.timeout, Timeout::default());
        assert_eq!(config.thresholds, Thresholds::default());
        assert_eq!(
            config.reporters,
            [Reporter::Terminal, Reporter::Progress, Reporter::Html]
        );
        assert_eq!(config.sandbox_dir, Path::new(".qmutant"));
        assert!(config.ignore.is_empty() && config.exclude_mutators.is_empty());
        assert!(config.root.is_absolute());
    }

    #[test]
    fn every_key_is_read() {
        let config = load(
            r#"
            mutate = ["A.qml"]
            command = "t"
            jobs = 3
            timeout = { ms = 10, factor = 2.0 }
            thresholds = { high = 90, low = 70, break = 85 }
            reporters = ["json"]
            sandbox_dir = "tmp/sandboxes"
            ignore = ["build"]
            exclude_mutators = ["StringLiteral"]
            "#,
            Overrides::default(),
        )
        .unwrap();
        assert_eq!(config.jobs, Jobs::Count(NonZeroUsize::new(3).unwrap()));
        assert_eq!(
            config.timeout,
            Timeout {
                ms: 10,
                factor: 2.0
            }
        );
        assert_eq!(config.thresholds.minimum, Some(85));
        assert_eq!(config.reporters, [Reporter::Json]);
        assert_eq!(config.ignore, ["build"]);
        assert_eq!(config.exclude_mutators, [Mutator::StringLiteral]);
    }

    #[test]
    fn jobs_and_break_accept_their_boundaries() {
        let config = load(
            "command = \"t\"\njobs = \"100%\"\nthresholds = { low = 100, high = 100, break = 100 }",
            Overrides::default(),
        )
        .unwrap();
        assert_eq!(config.jobs, Jobs::Percent(100));
        assert_eq!(config.thresholds.minimum, Some(100));
        let config = load("command = \"t\"\njobs = \"1%\"", Overrides::default()).unwrap();
        assert_eq!(config.jobs, Jobs::Percent(1));
    }

    #[test]
    fn overrides_replace_what_the_file_says() {
        let config = load(
            r#"
            mutate = ["A.qml"]
            command = "t"
            timeout = { ms = 10, factor = 2.0 }
            "#,
            Overrides {
                mutate: Some(vec!["B.qml".to_owned()]),
                jobs: Some(Jobs::Percent(100)),
                timeout_ms: Some(20),
                timeout_factor: Some(3.0),
                reporters: Some(vec![Reporter::Json]),
            },
        )
        .unwrap();
        assert_eq!(config.mutate, ["B.qml"]);
        assert_eq!(config.jobs, Jobs::Percent(100));
        assert_eq!(
            config.timeout,
            Timeout {
                ms: 20,
                factor: 3.0
            }
        );
        assert_eq!(config.reporters, [Reporter::Json]);
    }

    #[test]
    fn an_unknown_key_is_refused_with_its_line() {
        let error = load("command = \"t\"\nmutators = []", Overrides::default()).unwrap_err();
        assert!(
            error.starts_with("qmutant.toml: unknown field `mutators`"),
            "{error}"
        );
        assert!(error.ends_with("(line 2, column 1)"), "{error}");
    }

    #[test]
    fn a_missing_command_is_refused() {
        let error = load("", Overrides::default()).unwrap_err();
        assert!(error.contains("missing field `command`"), "{error}");
    }

    #[test]
    fn invalid_values_are_refused() {
        let cases = [
            ("command = \" \"", "`command` is empty"),
            ("command = \"t\"\nmutate = []", "`mutate` is empty"),
            (
                "command = \"t\"\nthresholds = { low = 90, high = 80 }",
                "low <= high <= 100",
            ),
            (
                "command = \"t\"\nthresholds = { low = 90, high = 101 }",
                "low <= high <= 100",
            ),
            (
                "command = \"t\"\nthresholds = { break = 101 }",
                "at most 100",
            ),
            ("command = \"t\"\ntimeout = { factor = 0.5 }", "at least 1"),
            (
                "command = \"t\"\nsandbox_dir = \"../x\"",
                "inside the project",
            ),
            (
                "command = \"t\"\nsandbox_dir = \"/tmp\"",
                "inside the project",
            ),
            ("command = \"t\"\nsandbox_dir = \"\"", "inside the project"),
            ("command = \"t\"\njobs = 0", "neither a worker count"),
            ("command = \"t\"\njobs = \"0%\"", "neither a worker count"),
            ("command = \"t\"\njobs = \"101%\"", "neither a worker count"),
            (
                "command = \"t\"\nreporters = [\"xml\"]",
                "unknown variant `xml`",
            ),
            (
                "command = \"t\"\nexclude_mutators = [\"Nope\"]",
                "unknown variant `Nope`",
            ),
        ];
        for (text, expected) in cases {
            let error = load(text, Overrides::default()).unwrap_err();
            assert!(error.contains(expected), "{text:?} gave {error}");
        }
    }

    #[test]
    fn an_unreadable_file_names_its_path() {
        let error =
            Config::load(Path::new("/nonexistent/qmutant.toml"), Overrides::default()).unwrap_err();
        assert!(error.to_string().starts_with("/nonexistent/qmutant.toml: "));
    }

    #[test]
    fn jobs_resolve_against_the_cpus() {
        let eight = NonZeroUsize::new(8).unwrap();
        assert_eq!(Jobs::Percent(50).resolve(eight).get(), 4);
        assert_eq!(Jobs::Percent(1).resolve(eight).get(), 1);
        assert_eq!(Jobs::Percent(100).resolve(eight).get(), 8);
        assert_eq!("3".parse::<Jobs>().unwrap().resolve(eight).get(), 3);
        assert!("x".parse::<Jobs>().is_err());
    }

    #[test]
    fn the_timeout_scales_the_baseline_and_adds_the_margin() {
        let timeout = Timeout {
            ms: 100,
            factor: 2.0,
        };
        assert_eq!(
            timeout.for_baseline(Duration::from_millis(50)),
            Duration::from_millis(200)
        );
    }

    #[test]
    fn reporter_names_round_trip() {
        for reporter in [
            Reporter::Terminal,
            Reporter::Progress,
            Reporter::Json,
            Reporter::Toml,
            Reporter::Html,
        ] {
            assert_eq!(reporter.to_string().parse::<Reporter>(), Ok(reporter));
        }
        assert!("xml".parse::<Reporter>().is_err());
    }
}
