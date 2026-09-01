use crate::harness::TestContext;
use predicates::prelude::*;
#[cfg(unix)]
use std::os::unix::fs::symlink;

fn install_failing_docker_prune(ctx: &TestContext) {
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
}

#[test]
fn clean_nodejs_yes_deletes_directories() {
    let ctx = TestContext::new();
    let cache =
        ctx.write_home_file("Desktop/workspace/node_modules/index.js", "console.log('cache');");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("clean")
        .arg("nodejs")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reclaimed"));

    assert!(!cache_dir.exists(), "cache directory should be deleted");
}

#[test]
fn clean_mise_yes_deletes_the_global_cache() {
    let ctx = TestContext::new();
    let cache_file = ctx.write_home_file("Library/Caches/mise/node/versions.msgpack", "cache");
    let cache = cache_file.ancestors().nth(2).expect("mise cache ancestor").to_path_buf();

    ctx.cli().arg("clean").arg("mise").arg("-y").assert().success();

    assert!(!cache.exists(), "mise cache should be deleted");
}

#[cfg(unix)]
#[test]
fn clean_bun_removes_a_cache_link_without_touching_its_target() {
    let ctx = TestContext::new();
    let install = ctx.create_home_dir(".bun/install");
    let outside = ctx.create_home_dir("outside-bun-cache");
    let sentinel = outside.join("sentinel.txt");
    std::fs::write(&sentinel, "preserve").expect("sentinel exists");
    let cache = install.join("cache");
    symlink(&outside, &cache).expect("Bun cache link exists");

    ctx.cli().arg("clean").arg("bun").arg("-y").assert().success();

    assert!(std::fs::symlink_metadata(cache).is_err(), "cache link should be removed");
    assert_eq!(std::fs::read_to_string(sentinel).unwrap(), "preserve");
}

#[test]
fn clean_pnpm_prunes_the_store_resolved_by_the_scan() {
    let ctx = TestContext::new();
    let store = ctx.create_home_dir("Library/pnpm/store/v11");
    let marker = ctx.work_dir().join("pnpm_prune_marker");
    ctx.set_env("PRF_TEST_MARKER", &marker);
    ctx.create_mock_command(
        "pnpm",
        r#"#!/bin/sh
if [ "$1" = "store" ] && [ "$2" = "path" ]; then
  echo "$HOME/Library/pnpm/store/v11"
  exit 0
fi
if [ "$2" = "store" ] && [ "$3" = "prune" ]; then
  echo "$@" > "$PRF_TEST_MARKER"
  exit 0
fi
exit 9
"#,
    );

    ctx.cli()
        .arg("clean")
        .arg("pnpm")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("unestimated amount"))
        .stdout(predicate::str::contains("1 completed"));

    let recorded = std::fs::read_to_string(marker).expect("pnpm prune should have been invoked");
    assert_eq!(recorded.trim(), format!("--store-dir={} store prune", store.display()));
}

#[test]
fn clean_routes_docker_prune_to_system_prune_not_a_filesystem_path() {
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
        .arg("clean")
        .arg("docker")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reclaimed"))
        .stdout(predicate::str::contains("unused images, containers, networks, build cache"))
        .stdout(predicate::str::contains("volumes (-a --volumes)"));

    let recorded =
        std::fs::read_to_string(&marker).expect("docker system prune should have been invoked");
    assert!(recorded.contains("system prune"), "recorded docker args: {recorded}");
    assert!(
        !ctx.work_dir().join("docker:prune").exists(),
        "no synthetic docker:prune filesystem path should be created or touched"
    );
}

#[test]
fn clean_does_not_prune_docker_without_a_scanned_candidate() {
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
        .arg("clean")
        .arg("docker")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("No cleanup actions were discovered"));

    assert!(!marker.exists(), "an action absent from the scan report must not run");
}

#[test]
fn clean_reports_docker_process_failure() {
    let ctx = TestContext::new();
    install_failing_docker_prune(&ctx);

    ctx.cli()
        .arg("clean")
        .arg("docker")
        .arg("-y")
        .assert()
        .failure()
        .stdout(predicate::str::contains("0 completed"))
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Failed: Docker prune"))
        .stderr(predicate::str::contains("status"))
        .stderr(predicate::str::contains("Cleanup incomplete"));
}

#[cfg(unix)]
#[test]
fn clean_removes_a_swiftpm_link_without_touching_its_target() {
    let ctx = TestContext::new();
    ctx.write_home_file("Desktop/workspace/Package.swift", "// package");
    let outside = ctx.work_dir().join("outside");
    std::fs::create_dir_all(&outside).expect("outside directory exists");
    let sentinel = outside.join("sentinel.txt");
    std::fs::write(&sentinel, "preserve").expect("sentinel exists");
    let link = ctx.home().join("Desktop/workspace/.build");
    symlink(&outside, &link).expect("cache-shaped link exists");

    ctx.cli()
        .arg("clean")
        .arg("xcode")
        .arg("-y")
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
fn clean_accepts_multiple_targets_and_reports_a_later_process_failure() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("Desktop/workspace/node_modules/index.js", "cache");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();
    install_failing_docker_prune(&ctx);

    ctx.cli()
        .arg("clean")
        .arg("nodejs")
        .arg("docker")
        .arg("-y")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1 completed"))
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Docker prune"));

    assert!(!cache_dir.exists(), "the successful path action must remain reported and applied");
}

#[test]
fn clean_reports_a_process_that_disappears_after_discovery() {
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
        .arg("clean")
        .arg("docker")
        .arg("-y")
        .assert()
        .failure()
        .stdout(predicate::str::contains("1 failed"))
        .stderr(predicate::str::contains("Cannot start Docker prune"))
        .stderr(predicate::str::contains("'docker'"));
}

#[test]
fn clean_without_targets_requires_a_terminal_when_candidates_exist() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("Desktop/workspace/__pycache__/foo.pyc", "cache");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("clean")
        .arg("-y")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Target selection requires an interactive terminal"))
        .stderr(predicate::str::contains("TARGET... or --all"));

    assert!(cache_dir.exists(), "cache directory must remain without a target decision");
}

#[test]
fn clean_all_yes_skips_both_prompts() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("Desktop/workspace/__pycache__/foo.pyc", "cache");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("clean")
        .arg("--all")
        .arg("-y")
        .assert()
        .success()
        .stdout(predicate::str::contains("Reclaimed"));

    assert!(!cache_dir.exists(), "explicit all with yes should apply without terminal input");
}

#[test]
fn clean_rejects_named_targets_with_all() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("clean")
        .arg("python")
        .arg("--all")
        .arg("-y")
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot be used with '--all'"));
}

#[test]
fn clean_without_targets_or_candidates_succeeds_without_a_terminal() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("clean")
        .assert()
        .success()
        .stdout(predicate::str::contains("No cleanup actions were discovered"));
}
