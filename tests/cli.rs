use std::path::Path;
use std::process::{Command as Process, Stdio};
use std::time::Duration;

use assert_cmd::Command;
use assert_fs::TempDir;
use assert_fs::prelude::*;
use predicates::prelude::*;

const COMPONENT: &str = r#"import QtQuick

Item {
    property string label: "x"
    function allowed(n) { return n < 3 }
}
"#;

const CHECKS_THE_LIMIT: &str = r#"command = "grep -q 'return n < 3' Main.qml""#;

fn project(component: &str, config: &str) -> TempDir {
    let root = TempDir::new().unwrap();
    root.child("Main.qml").write_str(component).unwrap();
    root.child("qmutant.toml").write_str(config).unwrap();
    root
}

fn qmutant(root: &Path) -> Command {
    let mut command = Command::cargo_bin("qmutant").unwrap();
    command
        .current_dir(root)
        .env("NO_COLOR", "1")
        .env_remove("RUST_LOG");
    command
}

#[test]
fn a_survivor_is_shown_and_scored() {
    let root = project(COMPONENT, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "terminal", "-j", "2"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Survived  Main.qml:4:28  StringLiteral\n  - property string label: \"x\"\n  + property string label: \"\"",
        ))
        .stdout(predicate::str::contains("Mutation score: 75.00%"));
    root.child(".qmutant").assert(predicate::path::missing());
}

#[test]
fn a_score_below_break_fails_the_run() {
    let root = project(
        COMPONENT,
        &format!("{CHECKS_THE_LIMIT}\nthresholds = {{ break = 80 }}"),
    );
    qmutant(&root)
        .args(["run", "--reporter", "terminal"])
        .assert()
        .code(1)
        .stdout(predicate::str::contains(
            "75.00%, below the break threshold of 80",
        ));
}

#[test]
fn a_mutant_that_hangs_the_tests_times_out() {
    let root = project(
        COMPONENT,
        r#"command = "grep -q 'label: \"\"' Main.qml && sleep 30; grep -q 'return n < 3' Main.qml""#,
    );
    qmutant(&root)
        .args([
            "run",
            "--reporter",
            "json",
            "--timeout",
            "300",
            "--timeout-factor",
            "1",
        ])
        .timeout(Duration::from_secs(20))
        .assert()
        .success();
    let report = std::fs::read_to_string(root.child("reports/mutation.json").path()).unwrap();
    let version = format!(r#""version": "{}""#, env!("CARGO_PKG_VERSION"));
    insta::assert_snapshot!(report.replace(&version, r#""version": "[version]""#));
}

#[test]
fn a_disabled_mutant_is_ignored_and_the_directive_is_consumed() {
    let component = COMPONENT.replace(
        "    property string label",
        "    // qmutant: disable next-line StringLiteral -- the label is decoration\n    property string label",
    );
    let root = project(&component, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "terminal"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score: 100.00%"))
        .stdout(predicate::str::contains(
            "│ Main.qml  ┆ 100.00 ┆ 3      ┆ 0       ┆ 0        ┆ 0       ┆ 0     ┆ 1       │",
        ))
        .stdout(predicate::str::contains(
            "│ All files ┆ 100.00 ┆ 3      ┆ 0       ┆ 0        ┆ 0       ┆ 0     ┆ 1       │",
        ));
}

#[test]
fn a_directive_that_disables_nothing_fails_the_run() {
    let component = COMPONENT.replace(
        "    function allowed",
        "    // qmutant: disable next-line ArrayDeclaration -- stale\n    function allowed",
    );
    let root = project(&component, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "terminal"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "error: Main.qml:5: `// qmutant: disable next-line ArrayDeclaration -- stale` disables nothing; remove it",
        ));
}

#[test]
fn a_directive_without_a_reason_is_refused() {
    let component = COMPONENT.replace(
        "    function allowed",
        "    // qmutant: disable next-line EqualityOperator\n    function allowed",
    );
    let root = project(&component, CHECKS_THE_LIMIT);
    qmutant(&root)
        .arg("run")
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "error: Main.qml:5: a disabled mutant needs a reason",
        ));
}

#[test]
fn failing_tests_stop_the_run_before_any_mutant() {
    let root = project(COMPONENT, r#"command = "echo broken suite; exit 1""#);
    qmutant(&root)
        .arg("run")
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "error: tests fail before anything is mutated, so every mutant would count as killed\nbroken suite",
        ));
    root.child(".qmutant").assert(predicate::path::missing());
}

#[test]
fn a_dry_run_only_runs_the_tests_once() {
    let root = project(
        COMPONENT,
        r#"command = "grep -q 'return n < 3' Main.qml && echo ran >> runs""#,
    );
    qmutant(&root)
        .args(["run", "--dry-run"])
        .assert()
        .success()
        .stdout(predicate::str::is_match(r"^The tests pass unmutated in .+\. 4 mutants would run, each stopped after .+\.\n$").unwrap());
    root.child("runs").assert(predicate::path::missing());
    root.child(".qmutant").assert(predicate::path::missing());
}

#[test]
fn an_unknown_key_is_refused() {
    let root = project(COMPONENT, &format!("{CHECKS_THE_LIMIT}\nworkers = 2"));
    qmutant(&root)
        .arg("run")
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "qmutant.toml: unknown field `workers`",
        ));
}

#[test]
fn a_missing_config_suggests_nothing_it_cannot_do() {
    let root = TempDir::new().unwrap();
    qmutant(&root)
        .arg("list")
        .assert()
        .code(2)
        .stderr(predicate::str::starts_with("error: qmutant.toml: "));
}

#[test]
fn mutate_on_the_command_line_replaces_the_config() {
    let root = project(
        COMPONENT,
        &format!("mutate = [\"Other.qml\"]\n{CHECKS_THE_LIMIT}"),
    );
    qmutant(&root)
        .args(["list", "--mutate", "Main.qml"])
        .assert()
        .success()
        .stdout(
            "0  Main.qml:4:28  StringLiteral  `\"x\"` -> `\"\"`\n\
             1  Main.qml:5:25  BlockStatement  `{ return n < 3 }` -> `{}`\n\
             2  Main.qml:5:36  EqualityOperator  `<` -> `<=`\n\
             3  Main.qml:5:36  EqualityOperator  `<` -> `>=`\n",
        );
}

#[test]
fn html_embeds_the_report() {
    let root = project(COMPONENT, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "html"])
        .assert()
        .success()
        .stdout(predicate::str::contains("reports/mutation.html"));
    root.child("reports/mutation.html")
        .assert(predicate::str::contains(
            r#""mutatorName": "StringLiteral""#,
        ));
}

#[test]
fn init_lists_the_projects_qml_files() {
    let root = TempDir::new().unwrap();
    root.child("ui/Main.qml").write_str(COMPONENT).unwrap();
    root.child("Model.js").write_str("").unwrap();
    qmutant(&root)
        .args(["init", "--command", "make test"])
        .assert()
        .success();
    root.child("qmutant.toml")
        .assert("mutate = [\"ui/Main.qml\"]\ncommand = \"make test\"\n");
    qmutant(&root)
        .args(["init", "--command", "make test"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("already exists"));
}

#[test]
fn init_leaves_out_the_files_that_hold_the_tests() {
    let root = TempDir::new().unwrap();
    root.child("ui/Main.qml").write_str(COMPONENT).unwrap();
    root.child("tests/tst_main.qml")
        .write_str(COMPONENT)
        .unwrap();
    root.child("tests/stubs/Fake.qml")
        .write_str(COMPONENT)
        .unwrap();
    root.child("ui/tst_inline.qml")
        .write_str(COMPONENT)
        .unwrap();
    qmutant(&root)
        .args(["init", "--command", "make test"])
        .assert()
        .success();
    root.child("qmutant.toml")
        .assert("mutate = [\"ui/Main.qml\"]\ncommand = \"make test\"\n");
}

#[test]
fn a_missing_config_is_reported_once() {
    let root = TempDir::new().unwrap();
    let output = qmutant(&root).arg("run").assert().code(2);
    let stderr = String::from_utf8(output.get_output().stderr.clone()).unwrap();
    assert_eq!(
        stderr.matches("No such file or directory").count(),
        1,
        "the reason was printed more than once: {stderr}"
    );
}

#[test]
fn init_refuses_a_project_that_is_only_tests() {
    let root = TempDir::new().unwrap();
    root.child("tests/tst_main.qml")
        .write_str(COMPONENT)
        .unwrap();
    qmutant(&root)
        .args(["init", "--command", "make test"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("only test files"));
    root.child("qmutant.toml")
        .assert(predicate::path::missing());
}

#[test]
fn an_interrupted_run_cleans_up_and_says_so() {
    let root = project(COMPONENT, r#"command = "sleep 30""#);
    let child = Process::new(assert_cmd::cargo::cargo_bin("qmutant"))
        .arg("run")
        .current_dir(&root)
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_millis(500));
    root.child(".qmutant").assert(predicate::path::exists());
    let pid = nix::unistd::Pid::from_raw(i32::try_from(child.id()).unwrap());
    nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGINT).unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(130));
    assert!(String::from_utf8_lossy(&output.stderr).contains("error: interrupted"));
    root.child(".qmutant").assert(predicate::path::missing());
}

#[test]
fn verbosity_raises_the_log_level_one_step_at_a_time() {
    let root = project(COMPONENT, CHECKS_THE_LIMIT);
    let quiet = qmutant(&root)
        .args(["run", "--reporter", "json"])
        .assert()
        .success();
    assert!(quiet.get_output().stderr.is_empty());
    qmutant(&root)
        .args(["run", "--reporter", "json", "-v"])
        .assert()
        .success()
        .stderr(predicate::str::contains("the unmutated tests pass"))
        .stderr(predicate::str::contains("judged").not());
    qmutant(&root)
        .args(["run", "--reporter", "json", "-vv"])
        .assert()
        .success()
        .stderr(predicate::str::contains("judged"));
}

#[test]
fn a_file_the_grammar_cannot_finish_is_refused_instead_of_hanging() {
    let root = project("m{x:[a=>{a''}}{u}`a", CHECKS_THE_LIMIT);
    let started = std::time::Instant::now();
    qmutant(&root)
        .arg("list")
        .timeout(Duration::from_secs(30))
        .assert()
        .code(2)
        .stderr("error: Main.qml: the QML grammar could not finish parsing this file\n");
    assert!(
        started.elapsed() < Duration::from_secs(3),
        "refused after {:?}",
        started.elapsed()
    );
}

#[test]
fn the_toml_report_holds_exactly_what_the_json_report_holds() {
    let root = project(COMPONENT, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "json", "--reporter", "toml"])
        .assert()
        .success()
        .stdout(predicate::str::contains("reports/mutation.toml"));
    let json = std::fs::read_to_string(root.child("reports/mutation.json").path()).unwrap();
    let toml = std::fs::read_to_string(root.child("reports/mutation.toml").path()).unwrap();
    let from_json: serde_json::Value = serde_json::from_str(&json).unwrap();
    let from_toml: serde_json::Value = toml::from_str(&toml).unwrap();
    assert_eq!(from_toml, from_json);
    assert_eq!(
        from_toml["files"]["Main.qml"]["mutants"][0]["status"],
        "Survived"
    );
}

#[test]
fn a_timeout_factor_too_large_to_represent_runs_without_a_limit() {
    let root = project(COMPONENT, CHECKS_THE_LIMIT);
    qmutant(&root)
        .args(["run", "--reporter", "terminal", "--timeout-factor", "1e300"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Mutation score: 75.00%"));
}

#[test]
fn init_writes_names_that_list_reads_back_literally() {
    let root = TempDir::new().unwrap();
    root.child("ui/[Main].qml").write_str(COMPONENT).unwrap();
    root.child("ui/M.qml").write_str(COMPONENT).unwrap();
    qmutant(&root)
        .args(["init", "--command", "make test"])
        .assert()
        .success();
    qmutant(&root)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("ui/[Main].qml:4:28"))
        .stdout(predicate::str::contains("ui/M.qml:4:28"));
}
