use crate::harness::TestContext;
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
