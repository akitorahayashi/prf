use crate::harness::TestContext;
use predicates::prelude::*;

#[test]
fn alias_sc_works_like_scan() {
    let ctx = TestContext::new();
    ctx.write_home_file("Desktop/project/__pycache__/foo.pyc", "cache");

    ctx.cli()
        .arg("sc")
        .arg("python")
        .arg("--verbose")
        .assert()
        .success()
        .stdout(predicate::str::contains("Scan results"))
        .stdout(predicate::str::contains("Python"))
        .stdout(predicate::str::contains("~/Desktop/project/__pycache__"));
}

#[test]
fn alias_cln_works_like_clean() {
    let ctx = TestContext::new();

    ctx.cli().arg("cln").arg("--help").assert().success().stdout(predicate::str::contains(
        "Scan, select, and delete development caches and generated artifacts",
    ));
}
