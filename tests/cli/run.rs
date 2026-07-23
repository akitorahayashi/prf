use crate::harness::TestContext;
use predicates::prelude::*;

#[test]
fn run_type_nodejs_yes_deletes_directories() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("workspace/node_modules/index.js", "console.log('cache');");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("nodejs")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleanup summary: 1 removed, 0 skipped, 0 failed"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}

#[test]
fn run_interactive_accepts_selection() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("workspace/__pycache__/foo.pyc", "cache");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("run")
        .arg(ctx.home())
        .write_stdin("python\ny\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deletion plan"))
        .stdout(predicate::str::contains("Cleanup summary: 1 removed, 0 skipped, 0 failed"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}

#[test]
fn explicit_docker_request_fails_when_docker_is_unavailable() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("pruned");
    ctx.create_mock_command(
        "docker",
        &format!(
            r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 1
fi
if [ "$2" = "prune" ]; then
  touch "{}"
fi
"#,
            marker.display()
        ),
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Docker is unavailable"));

    assert!(!marker.exists(), "Docker prune must not run when Docker is unavailable");
}

#[test]
fn failed_docker_scan_never_runs_prune() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("pruned");
    ctx.create_mock_command(
        "docker",
        &format!(
            r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$2" = "df" ]; then
  echo 'not-json'
  exit 0
fi
if [ "$2" = "prune" ]; then
  touch "{}"
  exit 0
fi
"#,
            marker.display()
        ),
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .failure()
        .stderr(predicate::str::contains("Scan is incomplete"));

    assert!(!marker.exists(), "Docker prune must not run after a failed scan");
}

#[test]
fn zero_reclaimable_docker_bytes_never_runs_prune() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("pruned");
    ctx.create_mock_command(
        "docker",
        &format!(
            r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$2" = "df" ]; then
  echo '{{"Reclaimable":"0B (0%)"}}'
  exit 0
fi
if [ "$2" = "prune" ]; then
  touch "{}"
  exit 0
fi
"#,
            marker.display()
        ),
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("No cleanup candidates were found"));

    assert!(!marker.exists(), "Docker prune must not run for a clean scan");
}

#[test]
fn implicit_run_reports_unavailable_categories_separately_from_clean_categories() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 1
fi
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Unavailable categories:"))
        .stdout(predicate::str::contains("- Docker:"))
        .stdout(predicate::str::contains("No cleanup candidates were found"));
}

#[test]
fn confirmed_docker_plan_runs_prune_once() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("pruned");
    ctx.create_mock_command(
        "docker",
        &format!(
            r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$2" = "df" ]; then
  echo '{{"Reclaimable":"1GB (100%)"}}'
  exit 0
fi
if [ "$2" = "prune" ]; then
  echo prune >> "{}"
  exit 0
fi
"#,
            marker.display()
        ),
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleanup summary: 1 removed, 0 skipped, 0 failed"));

    let invocations = std::fs::read_to_string(marker).expect("prune marker exists");
    assert_eq!(invocations.lines().count(), 1, "Docker prune runs exactly once");
}
