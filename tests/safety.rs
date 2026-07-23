#[allow(dead_code, unused_imports)]
mod harness;

use harness::TestContext;
use predicates::prelude::*;

#[test]
fn current_mode_excludes_system_targets() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--current")
        .arg("--list")
        .assert()
        .success()
        .stdout(predicate::str::contains("Found cleanup targets"))
        .stdout(predicate::str::contains("Homebrew").not())
        .stdout(predicate::str::contains("Docker").not())
        .stdout(predicate::str::contains("Unused images").not())
        .stdout(predicate::str::contains("Stopped containers").not())
        .stdout(predicate::str::contains("Unused volumes").not())
        .stdout(predicate::str::contains("Unused networks").not())
        .stdout(predicate::str::contains("Build cache").not());
}

#[test]
fn run_without_confirmation_preserves_targets() {
    let ctx = TestContext::new();
    let cache = ctx.write_home_file("workspace/node_modules/index.js", "console.log('cache');");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("nodejs")
        .arg(ctx.home())
        .write_stdin("n\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Deletion plan"))
        .stdout(predicate::str::contains("Aborted. No files were deleted."));

    assert!(cache_dir.exists(), "cache directory should remain after rejected confirmation");
}

#[test]
fn current_mode_rejects_external_category_as_usage_error() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--current")
        .arg("--type")
        .arg("docker")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("remove --current or select a local category"));
}

#[test]
fn nonexistent_root_is_an_operational_error() {
    let ctx = TestContext::new();
    let missing = ctx.work_dir().join("missing");

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("python")
        .arg(&missing)
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Invalid scan root"));
}

#[test]
fn missing_default_desktop_has_no_fallback_root() {
    let ctx = TestContext::new();

    ctx.cli()
        .arg("scan")
        .arg("--type")
        .arg("python")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Desktop"))
        .stderr(predicate::str::contains("Invalid scan root"));
}

#[test]
fn yes_does_not_expand_category_scope() {
    let ctx = TestContext::new();
    let python = ctx.write_home_file("workspace/__pycache__/module.pyc", "cache");
    let node = ctx.write_home_file("workspace/node_modules/index.js", "cache");
    let python_dir = python.parent().expect("Python cache has a parent").to_path_buf();
    let node_dir = node.parent().expect("Node cache has a parent").to_path_buf();

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("python")
        .arg("--yes")
        .arg(ctx.home())
        .assert()
        .success();

    assert!(!python_dir.exists(), "selected Python cache is removed");
    assert!(node_dir.exists(), "unselected Node cache remains");
}

#[test]
fn nested_cross_category_candidates_execute_once() {
    let ctx = TestContext::new();
    let nested =
        ctx.write_home_file("workspace/DerivedData/node_modules/index.js", "generated content");
    let derived_data = nested
        .parent()
        .and_then(std::path::Path::parent)
        .expect("nested target has a DerivedData ancestor")
        .to_path_buf();

    ctx.cli()
        .arg("run")
        .arg("--type")
        .arg("nodejs")
        .arg("--type")
        .arg("xcode")
        .arg("--yes")
        .arg(ctx.home())
        .assert()
        .success()
        .stdout(predicate::str::contains("Cleanup summary: 1 removed, 0 skipped, 0 failed"))
        .stdout(predicate::str::contains("[xcode,nodejs]"));

    assert!(!derived_data.exists(), "normalized ancestor candidate is removed once");
}
