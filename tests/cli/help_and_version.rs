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
        .stdout(predicate::str::contains("run"));
}

#[test]
fn category_help_values_come_from_the_catalog() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("- xcode:"))
        .stdout(predicate::str::contains("- python:"))
        .stdout(predicate::str::contains("- rust:"))
        .stdout(predicate::str::contains("- nodejs:"))
        .stdout(predicate::str::contains("- brew:"))
        .stdout(predicate::str::contains("- docker:"));
}
