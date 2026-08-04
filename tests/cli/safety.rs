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
        .stdout(predicate::str::contains("mise").not())
        .stdout(predicate::str::contains("Bun").not())
        .stdout(predicate::str::contains("pnpm").not())
        .stdout(predicate::str::contains("Docker").not())
        .stdout(predicate::str::contains("Unused images").not())
        .stdout(predicate::str::contains("Stopped containers").not())
        .stdout(predicate::str::contains("Unused volumes").not())
        .stdout(predicate::str::contains("Unused networks").not())
        .stdout(predicate::str::contains("Build cache").not());
}

#[test]
fn clean_requires_a_terminal_or_yes_for_confirmation() {
    let ctx = TestContext::new();
    let cache =
        ctx.write_home_file("Desktop/workspace/node_modules/index.js", "console.log('cache');");
    let cache_dir = cache.parent().expect("cache file has parent").to_path_buf();

    ctx.cli()
        .arg("clean")
        .arg("nodejs")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Deletion plan"))
        .stderr(predicate::str::contains("Deletion confirmation requires an interactive terminal"))
        .stderr(predicate::str::contains("Pass --yes"));

    assert!(cache_dir.exists(), "cache directory must remain without confirmation");
}

#[test]
fn dynamic_cache_path_cannot_contain_the_home_directory() {
    let ctx = TestContext::new();
    ctx.set_env("MISE_CACHE_DIR", ctx.home());

    ctx.cli()
        .arg("scan")
        .arg("mise")
        .assert()
        .failure()
        .stderr(predicate::str::contains("contains protected path"));
}
