use super::brew::BrewTarget;
use super::category::Category;
use super::docker::DockerTarget;
use super::name_matcher::NameMatcherTarget;
use super::rust::RustTarget;
use super::target::CleanupTarget;
use super::xcode::XcodeTarget;

const PYTHON_TARGETS: &[&str] =
    &["__pycache__", ".pytest_cache", ".ruff_cache", ".mypy_cache", ".venv"];
const NODEJS_TARGETS: &[&str] = &["node_modules", ".next", ".nuxt", ".svelte-kit"];

const CATEGORY_ORDER: [Category; 6] = [
    Category::Xcode,
    Category::Python,
    Category::Rust,
    Category::Nodejs,
    Category::Brew,
    Category::Docker,
];

pub fn category_order() -> &'static [Category] {
    &CATEGORY_ORDER
}

pub fn categories_for_mode(current: bool) -> Vec<Category> {
    category_order()
        .iter()
        .copied()
        .filter(|category| category.supports_current_mode(current))
        .collect()
}

pub fn unsupported_for_current(requested: &[Category]) -> Vec<Category> {
    requested.iter().copied().filter(|category| !category.supports_current_mode(true)).collect()
}

pub fn unique_categories(categories: Vec<Category>) -> Vec<Category> {
    let mut unique = Vec::new();
    for category in categories {
        if !unique.contains(&category) {
            unique.push(category);
        }
    }
    unique
}

pub fn resolve(
    categories: &[Category],
    all: bool,
    current: bool,
) -> Result<Vec<Category>, crate::error::AppError> {
    let resolved = if all || categories.is_empty() {
        categories_for_mode(current)
    } else {
        unique_categories(categories.to_vec())
    };

    if current {
        let unsupported = unsupported_for_current(&resolved);
        if !unsupported.is_empty() {
            let names =
                unsupported.iter().map(|category| category.as_str()).collect::<Vec<_>>().join(", ");
            return Err(crate::error::AppError::UnsupportedCurrentModeCategory(names));
        }
    }

    Ok(resolved)
}

pub fn build_targets(categories: &[Category], current: bool) -> Vec<Box<dyn CleanupTarget>> {
    let mut targets: Vec<Box<dyn CleanupTarget>> = Vec::new();

    for category in categories {
        if !category.supports_current_mode(current) {
            continue;
        }

        match category {
            Category::Xcode => targets.push(Box::new(XcodeTarget::new(current))),
            Category::Python => {
                targets.push(Box::new(NameMatcherTarget::new(Category::Python, PYTHON_TARGETS)))
            }
            Category::Rust => targets.push(Box::new(RustTarget::new())),
            Category::Nodejs => {
                targets.push(Box::new(NameMatcherTarget::new(Category::Nodejs, NODEJS_TARGETS)))
            }
            Category::Brew => targets.push(Box::new(BrewTarget::new())),
            Category::Docker => targets.push(Box::new(DockerTarget::new())),
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_for_current_mode_excludes_system_targets() {
        let categories = categories_for_mode(true);
        assert!(!categories.contains(&Category::Brew));
        assert!(!categories.contains(&Category::Docker));
    }

    #[test]
    fn category_order_is_authoritative_for_default_mode() {
        let categories = categories_for_mode(false);
        assert_eq!(categories, CATEGORY_ORDER);
    }

    #[test]
    fn build_targets_excludes_brew_and_docker_in_current_mode() {
        let requested = vec![Category::Xcode, Category::Brew, Category::Docker, Category::Python];

        let targets = build_targets(&requested, true);
        let target_categories: Vec<Category> =
            targets.iter().map(|target| target.category()).collect();

        assert!(!target_categories.contains(&Category::Brew));
        assert!(!target_categories.contains(&Category::Docker));
    }

    #[test]
    fn build_targets_include_requested_categories_when_not_current_mode() {
        let targets = build_targets(&CATEGORY_ORDER, false);
        let target_categories: Vec<Category> =
            targets.iter().map(|target| target.category()).collect();
        assert_eq!(target_categories, CATEGORY_ORDER);
    }
}
