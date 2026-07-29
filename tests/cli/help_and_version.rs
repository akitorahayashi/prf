use crate::harness::TestContext;
use predicates::prelude::*;

#[test]
fn version_flag_works() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_primary_commands() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("scan"))
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("Safely clean development caches"))
        .stdout(predicate::str::contains("--version"));
}

#[test]
fn scan_help_explains_every_input() {
    let ctx = TestContext::new();

    ctx.cli()
        .args(["scan", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--type <TARGET>"))
        .stdout(predicate::str::contains(
            "possible values: xcode, python, rust, nodejs, brew, docker",
        ))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--list"))
        .stdout(predicate::str::contains("--current"))
        .stdout(predicate::str::contains("[PATH]..."))
        .stdout(predicate::str::contains("without measuring"))
        .stdout(predicate::str::contains("home discovery remains enabled"));
}

#[test]
fn run_help_explains_prompt_interactions() {
    let ctx = TestContext::new();

    ctx.cli()
        .args(["run", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--type <TARGET>"))
        .stdout(predicate::str::contains("--all"))
        .stdout(predicate::str::contains("--yes"))
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--current"))
        .stdout(predicate::str::contains("[PATH]..."))
        .stdout(predicate::str::contains("skip target selection"))
        .stdout(predicate::str::contains("still confirm deletion"))
        .stdout(predicate::str::contains("target selection still appears"));
}
