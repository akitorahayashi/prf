use std::path::PathBuf;

use rayon::prelude::*;

use crate::error::AppError;
use crate::fs::size::path_size;

use super::action::{Action, EntryKind};
use super::target::TargetId;

#[derive(Debug, Clone)]
pub struct Candidate {
    pub target: TargetId,
    pub action: Action,
    estimated_size: Option<u64>,
}

impl Candidate {
    pub fn directory(target: TargetId, path: PathBuf) -> Self {
        Self {
            target,
            action: Action::RemovePath { path, kind: EntryKind::Directory },
            estimated_size: None,
        }
    }

    pub fn file(target: TargetId, path: PathBuf) -> Self {
        Self {
            target,
            action: Action::RemovePath { path, kind: EntryKind::File },
            estimated_size: None,
        }
    }

    pub fn process(
        target: TargetId,
        label: &'static str,
        program: &'static str,
        args: &'static [&'static str],
        estimated_size: u64,
    ) -> Self {
        Self {
            target,
            action: Action::RunProcess { label, program, args },
            estimated_size: Some(estimated_size),
        }
    }

    pub fn estimated_size(&self) -> u64 {
        self.estimated_size.unwrap_or_default()
    }

    fn measure(&mut self) -> Result<(), AppError> {
        if self.estimated_size.is_some() {
            return Ok(());
        }

        let path = self.action.path().ok_or_else(|| {
            AppError::Cleanup("process candidate is missing an estimated size".to_string())
        })?;
        self.estimated_size = Some(path_size(path)?);
        Ok(())
    }
}

pub fn measure_candidates<F>(candidates: &mut [Candidate], on_measured: F) -> Result<(), AppError>
where
    F: Fn() + Sync,
{
    candidates.par_iter_mut().try_for_each(|candidate| {
        candidate.measure()?;
        on_measured();
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use assert_fs::TempDir;
    use assert_fs::prelude::*;

    use super::*;

    const TARGET: TargetId = TargetId::new("test");

    #[test]
    fn measurement_assigns_path_sizes() {
        let temp = TempDir::new().expect("temp directory is created");
        let directory = temp.child("node_modules");
        directory.child("lib").create_dir_all().expect("nested directory exists");
        directory.child("lib/index.js").write_str("cache").expect("file exists");
        let file = temp.child("cache.log");
        file.write_str("hello").expect("file exists");
        let mut candidates = vec![
            Candidate::directory(TARGET, directory.path().to_path_buf()),
            Candidate::file(TARGET, file.path().to_path_buf()),
        ];

        measure_candidates(&mut candidates, || {}).expect("measurement succeeds");

        assert!(candidates.iter().all(|candidate| candidate.estimated_size() > 0));
    }

    #[test]
    fn measurement_tolerates_paths_removed_after_discovery() {
        let temp = TempDir::new().expect("temp directory is created");
        let mut candidates = vec![
            Candidate::directory(TARGET, temp.path().join("gone-directory")),
            Candidate::file(TARGET, temp.path().join("gone-file")),
        ];

        measure_candidates(&mut candidates, || {}).expect("measurement succeeds");

        assert!(candidates.iter().all(|candidate| candidate.estimated_size() == 0));
    }
}
