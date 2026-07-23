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
        .stdout(predicate::str::contains("Found cleanup targets"));
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
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Reclaimable":"1GB (100%)"}'
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
        .stdout(predicate::str::contains("docker system prune --all --force --volumes"));
}

#[test]
fn scan_finds_uv_cache_beyond_the_previous_depth_limit() {
    let ctx = TestContext::new();
    let nested = (1..=12).map(|depth| format!("level-{depth}")).collect::<Vec<_>>().join("/");
    ctx.write_home_file(format!("{nested}/.uv-cache/archive.bin"), "cache");

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("python")
        .arg("--verbose")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains(".uv-cache"));
}
