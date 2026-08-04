use std::path::{Component, Path, PathBuf};

pub(crate) fn validate_removal_path(
    path: &Path,
    protected_paths: &[PathBuf],
) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!("cleanup path must be absolute: {}", path.display()));
    }

    if path
        .components()
        .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
    {
        return Err(format!("cleanup path must not contain '.' or '..': {}", path.display()));
    }

    let normal_components =
        path.components().filter(|component| matches!(component, Component::Normal(_))).count();
    if normal_components < 2 {
        return Err(format!("cleanup path is too broad: {}", path.display()));
    }

    if let Some(protected) = protected_paths
        .iter()
        .find(|protected| protected.as_path() == path || protected.starts_with(path))
    {
        return Err(format!(
            "cleanup path {} contains protected path {}",
            path.display(),
            protected.display()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_relative_shallow_and_protected_ancestor_paths() {
        let protected = vec![PathBuf::from("/Users/test/Desktop")];

        assert!(validate_removal_path(Path::new("cache"), &protected).is_err());
        assert!(validate_removal_path(Path::new("/tmp"), &protected).is_err());
        assert!(validate_removal_path(Path::new("/Users/test"), &protected).is_err());
        assert!(validate_removal_path(Path::new("/Users/test/cache"), &protected).is_ok());
    }

    #[test]
    fn rejects_parent_components_before_filesystem_normalization() {
        assert!(validate_removal_path(Path::new("/Users/test/cache/.."), &[]).is_err());
    }
}
