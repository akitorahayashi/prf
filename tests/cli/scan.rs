use crate::harness::TestContext;
use predicates::prelude::*;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

#[test]
fn scan_python_verbose_lists_targets() {
    let ctx = TestContext::new();
    ctx.write_home_file("Desktop/project/__pycache__/foo.pyc", "cache");

    ctx.cli()
        .arg("scan")
        .arg("PYTHON")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan results"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("~/Desktop/project/__pycache__"));
}

#[test]
fn scan_mise_reports_the_global_cache() {
    let ctx = TestContext::new();
    ctx.write_home_file("Library/Caches/mise/node/versions.msgpack", "cache");

    ctx.cli()
        .arg("scan")
        .arg("mise")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("mise"))
        .stdout(predicate::str::contains("~/Library/Caches/mise"));
}

#[test]
fn scan_bun_honors_the_global_cache_configuration() {
    let ctx = TestContext::new();
    ctx.write_home_file(".bunfig.toml", "[install.cache]\ndir = '~/Library/Caches/custom-bun'\n");
    ctx.write_home_file("Library/Caches/custom-bun/pkg@1.0.0/index.js", "cache");

    ctx.cli()
        .arg("scan")
        .arg("bun")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Bun"))
        .stdout(predicate::str::contains("~/Library/Caches/custom-bun"));
}

#[test]
fn scan_rejects_a_malformed_global_bun_config() {
    let ctx = TestContext::new();
    ctx.write_home_file(".bunfig.toml", "[install.cache\n");

    ctx.cli()
        .arg("scan")
        .arg("bun")
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid Bun config"));
}

#[test]
fn scan_reports_pnpm_prune_without_running_it_or_inventing_an_estimate() {
    let ctx = TestContext::new();
    let marker = ctx.work_dir().join("pnpm_prune_marker");
    ctx.set_env("PRF_TEST_MARKER", &marker);
    ctx.create_home_dir("Library/pnpm/store/v11");
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
        .arg("scan")
        .arg("pnpm")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("pnpm"))
        .stdout(predicate::str::contains("pnpm store prune"))
        .stdout(predicate::str::contains("unknown (1 unestimated action)"));

    assert!(!marker.exists(), "scan must not prune the pnpm store");
}

#[test]
fn scan_reports_missing_pnpm_as_a_diagnostic() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("pnpm")
        .assert()
        .success()
        .stderr(predicate::str::contains("pnpm CLI is unavailable"));
}

#[test]
fn scan_list_prints_target_listing() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--list")
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
        .assert()
        .success()
        .stdout(predicate::str::contains("Docker"))
        .stdout(predicate::str::contains("Unused images"))
        .stdout(predicate::str::contains("Build cache"));
}

#[test]
fn scan_reports_missing_docker_as_a_diagnostic() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("docker")
        .assert()
        .success()
        .stderr(predicate::str::contains("Docker CLI is unavailable"));
}

#[test]
fn scan_requires_home_only_for_the_default_root() {
    let ctx = TestContext::new();

    ctx.cli()
        .env_remove("HOME")
        .arg("scan")
        .arg("python")
        .assert()
        .failure()
        .stderr(predicate::str::contains("HOME is not set"));
}

#[test]
fn current_scope_works_without_home() {
    let ctx = TestContext::new();
    let root = ctx.work_dir().join("workspace");
    std::fs::create_dir_all(root.join("__pycache__")).expect("cache directory exists");

    ctx.cli_in(&root)
        .env_remove("HOME")
        .arg("scan")
        .arg("python")
        .arg("--current")
        .assert()
        .success()
        .stdout(predicate::str::contains("Python"))
        .stderr(predicate::str::contains("Home directory is unavailable").not());
}

#[test]
fn scan_rejects_a_path_argument() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg(ctx.home())
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid value"))
        .stderr(predicate::str::contains("possible values"));
}

#[test]
fn scan_rejects_malformed_docker_output() {
    let ctx = TestContext::new();
    ctx.create_mock_command(
        "docker",
        r#"#!/bin/sh
if [ "$1" = "info" ]; then
  exit 0
fi
if [ "$1" = "system" ] && [ "$2" = "df" ]; then
  echo 'not-json'
  exit 0
fi
exit 0
"#,
    );

    ctx.cli()
        .arg("scan")
        .arg("docker")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Discovery failed"))
        .stderr(predicate::str::contains("discovery complete").not())
        .stderr(predicate::str::contains("not valid JSON"));
}

#[cfg(unix)]
#[test]
fn scan_reports_allocated_footprint_for_sparse_files() {
    use std::fs::File;

    let ctx = TestContext::new();
    let cache = ctx.create_home_dir("Desktop/workspace/node_modules");
    let sparse = cache.join("sparse.bin");
    File::create(&sparse)
        .expect("sparse file is created")
        .set_len(1024 * 1024 * 1024)
        .expect("logical length is set");
    let allocated = fs::metadata(&cache).unwrap().blocks() * 512
        + fs::metadata(&sparse).unwrap().blocks() * 512;
    let expected = if allocated == 0 {
        "0 B".to_string()
    } else {
        assert!(allocated < 10_000, "fixture allocation exceeded the expected SI range");
        format!("{:.1} KB", allocated as f64 / 1_000.0)
    };

    ctx.cli()
        .arg("scan")
        .arg("nodejs")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains(expected))
        .stdout(predicate::str::contains("1.07 GB").not());
}
