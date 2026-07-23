use crate::harness::TestContext;
use predicates::prelude::*;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

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

#[test]
fn scan_reports_missing_docker_as_a_diagnostic() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("docker")
        .assert()
        .success()
        .stderr(predicate::str::contains("Docker CLI is unavailable"));
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
        .arg("--type")
        .arg("docker")
        .assert()
        .failure()
        .stderr(predicate::str::contains("Discovery failed"))
        .stderr(predicate::str::contains("not valid JSON"));
}

#[cfg(unix)]
#[test]
fn scan_list_does_not_measure_discovered_candidates() {
    let ctx = TestContext::new();
    let cache = ctx.create_home_dir("workspace/node_modules");
    let mut permissions = fs::metadata(&cache).expect("cache metadata exists").permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&cache, permissions).expect("cache becomes unreadable");

    if fs::read_dir(&cache).is_ok() {
        let mut permissions = fs::metadata(&cache).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&cache, permissions).unwrap();
        return;
    }

    let list = ctx
        .cli()
        .arg("scan")
        .arg("--list")
        .arg("--type")
        .arg("nodejs")
        .arg(ctx.home())
        .output()
        .expect("list command runs");
    let scan = ctx
        .cli()
        .arg("scan")
        .arg("--type")
        .arg("nodejs")
        .arg(ctx.home())
        .output()
        .expect("scan command runs");

    let mut permissions = fs::metadata(&cache).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&cache, permissions).expect("cache permissions are restored");

    assert!(list.status.success(), "list stderr: {}", String::from_utf8_lossy(&list.stderr));
    assert!(!scan.status.success(), "scan unexpectedly succeeded");
    assert!(
        String::from_utf8_lossy(&scan.stderr).contains("Footprint estimation failed"),
        "scan stderr: {}",
        String::from_utf8_lossy(&scan.stderr)
    );
}

#[cfg(unix)]
#[test]
fn scan_reports_allocated_footprint_for_sparse_files() {
    use std::fs::File;

    let ctx = TestContext::new();
    let cache = ctx.create_home_dir("workspace/node_modules");
    let sparse = cache.join("sparse.bin");
    File::create(&sparse)
        .expect("sparse file is created")
        .set_len(1024 * 1024 * 1024)
        .expect("logical length is set");
    let allocated = fs::metadata(&cache).unwrap().blocks() * 512
        + fs::metadata(&sparse).unwrap().blocks() * 512;
    let expected = prf::output::bytes::format_bytes(allocated);

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("nodejs")
        .arg("--verbose")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains(expected))
        .stdout(predicate::str::contains("1.07 GB").not());
}
