use crate::harness::TestContext;
use predicates::prelude::*;

#[test]
fn scan_python_verbose_lists_targets() {
    let ctx = TestContext::new();
    ctx.write_home_file("project/__pycache__/foo.pyc", "cache");

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("python")
        .arg("--verbose")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan results"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("~/project/__pycache__"));
}

#[test]
fn scan_list_prints_target_listing() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--list")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Found cleanup targets"))
        // No docker mock installed, so the controlled PATH must keep the host daemon out.
        .stdout(predicate::str::contains("Docker").not());
}

#[test]
fn scan_reports_docker_reclaimable_size() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Type":"Images","Reclaimable":"1.5GB"}'
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("docker")
        .assert()
        .success()
        .stdout(predicate::str::contains("Docker"))
        .stdout(predicate::str::contains("GB"));
}

#[test]
fn scan_list_reports_docker_when_docker_is_available() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("scan")
        .arg("--list")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Docker"))
        .stdout(predicate::str::contains("Unused images"))
        .stdout(predicate::str::contains("Build cache"));
}
