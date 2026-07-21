use assert_cmd::Command;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub(crate) struct TestContext {
    _root: TempDir,
    home: PathBuf,
    work_dir: PathBuf,
    bin_dir: PathBuf,
    env_vars: RefCell<HashMap<String, OsString>>,
}

impl TestContext {
    pub(crate) fn new() -> Self {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        let test_tmp_dir = Path::new(manifest_dir).join("target").join("test_tmp");
        fs::create_dir_all(&test_tmp_dir).expect("test temp root is created");

        let root = TempDir::new_in(&test_tmp_dir).expect("temp directory is created");
        let home = root.path().join("home");
        let work_dir = root.path().join("work");
        let bin_dir = root.path().join("bin");

        fs::create_dir_all(&home).expect("home directory is created");
        fs::create_dir_all(&work_dir).expect("work directory is created");
        fs::create_dir_all(&bin_dir).expect("mock bin directory is created");

        let context =
            Self { _root: root, home, work_dir, bin_dir, env_vars: RefCell::new(HashMap::new()) };
        context.set_controlled_path();
        context
    }

    /// Pins PATH to the mock bin directory only, so external tools such as `docker` resolve
    /// solely when a mock has been installed. Including system dirs like `/usr/bin` would let
    /// a host-installed `docker` leak in, so they are deliberately excluded; the CLI shells
    /// out only to `docker`, and mock scripts rely on the kernel resolving their `#!/bin/sh`
    /// shebang directly rather than on PATH.
    fn set_controlled_path(&self) {
        self.set_env("PATH", &self.bin_dir);
    }

    pub(crate) fn home(&self) -> &Path {
        &self.home
    }

    pub(crate) fn work_dir(&self) -> &Path {
        &self.work_dir
    }

    pub(crate) fn cli(&self) -> Command {
        self.cli_in(&self.work_dir)
    }

    pub(crate) fn cli_in<P: AsRef<Path>>(&self, dir: P) -> Command {
        let mut cmd = Command::new(assert_cmd::cargo::cargo_bin!("prf"));
        cmd.current_dir(dir.as_ref()).env("HOME", &self.home);

        for (key, value) in self.env_vars.borrow().iter() {
            cmd.env(key, value);
        }

        cmd
    }

    pub(crate) fn write_home_file<P: AsRef<Path>>(&self, relative: P, contents: &str) -> PathBuf {
        let path = self.home.join(relative.as_ref());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("file parent is created");
        }
        fs::write(&path, contents).expect("file is written");
        path
    }

    pub(crate) fn create_home_dir<P: AsRef<Path>>(&self, relative: P) -> PathBuf {
        let path = self.home.join(relative.as_ref());
        fs::create_dir_all(&path).expect("directory is created");
        path
    }

    pub(crate) fn create_mock_command(&self, name: &str, script: &str) -> PathBuf {
        let path = self.bin_dir.join(name);
        fs::write(&path, script).expect("mock command is written");

        #[cfg(unix)]
        {
            let mut permissions =
                fs::metadata(&path).expect("mock command metadata exists").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&path, permissions).expect("mock command is executable");
        }

        // The bin directory is already first on the controlled PATH set in `new`, so writing
        // the script here is enough to make the tool resolvable.
        path
    }

    pub(crate) fn set_env<S: AsRef<OsStr>>(&self, key: &str, value: S) {
        self.env_vars.borrow_mut().insert(key.to_string(), value.as_ref().to_os_string());
    }
}
