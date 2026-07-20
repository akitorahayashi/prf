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
        .stdout(predicate::str::contains("Attempted to delete"));

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
        .stdout(predicate::str::contains("Attempted to delete"));

    let recorded =
        std::fs::read_to_string(&marker).expect("docker system prune should have been invoked");
    assert!(recorded.contains("system prune"), "recorded docker args: {recorded}");
    assert!(
        !ctx.work_dir().join("docker:prune").exists(),
        "no synthetic docker:prune filesystem path should be created or touched"
    );
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
        .stdout(predicate::str::contains("Attempted to delete"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}
