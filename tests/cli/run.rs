use crate::harness::TestContext;
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::symlink;

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
        .stdout(predicate::str::contains("Reclaimed"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}

#[test]
fn run_routes_docker_prune_to_system_prune_not_a_filesystem_path() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("docker_prune_marker");
    ctx.set_env("PRF_TEST_MARKER", &marker);
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
if [ "$1" = "system" ] && [ "$2" = "prune" ]; then
  echo "$@" > "$PRF_TEST_MARKER"
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reclaimed"));

    let recorded =
        std::fs::read_to_string(&marker).expect("docker system prune should have been invoked");
    assert!(recorded.contains("system prune"), "recorded docker args: {recorded}");
    assert!(
        !ctx.work_dir().join("docker:prune").exists(),
        "no synthetic docker:prune filesystem path should be created or touched"
    );
}

#[test]
fn run_does_not_prune_docker_without_a_scanned_candidate() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("docker_prune_marker");
    ctx.set_env("PRF_TEST_MARKER", &marker);
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Type":"Images","Reclaimable":"0B"}'
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "prune" ]; then
  echo "$@" > "$PRF_TEST_MARKER"
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("No cleanup actions were discovered"));

    assert!(!marker.exists(), "an action absent from the scan report must not run");
}

#[test]
fn run_reports_docker_process_failure() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Type":"Images","Reclaimable":"1GB"}'
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "prune" ]; then
  exit 7
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .assert()
        .failure()
        .stdout(predicate::str::contains("0 completed"))
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Failed: Docker reclaimable"))
        .stderr(predicate::str::contains("status"))
        .stderr(predicate::str::contains("Cleanup incomplete"));
}

#[cfg(unix)]
#[test]
fn run_removes_a_swiftpm_link_without_touching_its_target() {
    let ctx = TestContext::new();
    ctx.write_home_file("workspace/Package.swift", "// package");
    let outside = ctx.work_dir().join("outside");
    std::fs::create_dir_all(&outside).expect("outside directory exists");
    let sentinel = outside.join("sentinel.txt");
    std::fs::write(&sentinel, "preserve").expect("sentinel exists");
    let link = ctx.home().join("workspace/.build");
    symlink(&outside, &link).expect("cache-shaped link exists");

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("xcode")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("1 completed"));

    assert!(
        std::fs::symlink_metadata(&link).is_err(),
        "the confirmed link entry should be removed"
    );
    assert!(outside.is_dir(), "the link target directory must remain");
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "preserve");
}

#[test]
fn run_reports_successful_mutation_before_a_later_process_failure() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("workspace/node_modules/index.js", "cache");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Type":"Images","Reclaimable":"1GB"}'
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "prune" ]; then
  exit 7
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("nodejs")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .arg(ctx.home())
        .assert()
        .failure()
        .stdout(predicate::str::contains("1 completed"))
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Docker reclaimable"));

    assert!(!cache_dir.exists(), "the successful path action must remain reported and applied");
}

#[test]
fn run_reports_a_process_that_disappears_after_discovery() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo '{"Type":"Images","Reclaimable":"1GB"}'
  /bin/rm "$0"
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("docker")
        .arg("-y")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Cannot start Docker reclaimable"))
        .stderr(predicate::str::contains("'docker'"));
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
        .stdout(predicate::str::contains("Reclaimed"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}
