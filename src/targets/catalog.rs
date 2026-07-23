use std::sync::OnceLock;

use super::brew::BrewTarget;
use super::category::Category;
use super::docker::DockerTarget;
use super::nodejs::NodejsTarget;
use super::python::PythonTarget;
use super::rust::RustTarget;
use super::target::CleanupTarget;
use super::xcode::XcodeTarget;

pub struct CategoryEntry {
    pub category: Category,
    pub id: &'static str,
    pub display_name: &'static str,
    pub description: &'static str,
    pub supports_current: bool,
    pub optional: bool,
    build: fn(bool) -> Box<dyn CleanupTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestOrigin {
    Implicit,
    Explicit,
    All,
}

#[derive(Debug, Clone)]
pub struct CategorySelection {
    pub categories: Vec<Category>,
    pub origin: RequestOrigin,
}

const CATALOG: [CategoryEntry; 6] = [
    CategoryEntry {
        category: Category::Xcode,
        id: "xcode",
        display_name: "Xcode",
        description: "Xcode and SwiftPM generated artifacts",
        supports_current: true,
        optional: false,
        build: build_xcode,
    },
    CategoryEntry {
        category: Category::Python,
        id: "python",
        display_name: "Python",
        description: "Python caches and virtual environments",
        supports_current: true,
        optional: false,
        build: build_python,
    },
    CategoryEntry {
        category: Category::Rust,
        id: "rust",
        display_name: "Rust",
        description: "Cargo target directories",
        supports_current: true,
        optional: false,
        build: build_rust,
    },
    CategoryEntry {
        category: Category::Nodejs,
        id: "nodejs",
        display_name: "Node.js",
        description: "Node.js dependencies and framework build output",
        supports_current: true,
        optional: false,
        build: build_nodejs,
    },
    CategoryEntry {
        category: Category::Brew,
        id: "brew",
        display_name: "Homebrew",
        description: "Homebrew caches and logs",
        supports_current: false,
        optional: false,
        build: build_brew,
    },
    CategoryEntry {
        category: Category::Docker,
        id: "docker",
        display_name: "Docker",
        description: "Unused Docker data",
        supports_current: false,
        optional: true,
        build: build_docker,
    },
];

pub fn category_order() -> &'static [Category] {
    static CATEGORIES: OnceLock<Vec<Category>> = OnceLock::new();
    CATEGORIES.get_or_init(|| CATALOG.iter().map(|entry| entry.category).collect())
}

pub fn entries() -> &'static [CategoryEntry] {
    &CATALOG
}

pub fn entry(category: Category) -> &'static CategoryEntry {
    CATALOG
        .iter()
        .find(|entry| entry.category == category)
        .expect("every Category variant has a catalog entry")
}

pub fn find(id: &str) -> Option<&'static CategoryEntry> {
    CATALOG.iter().find(|entry| entry.id.eq_ignore_ascii_case(id))
}

pub fn categories_for_mode(current: bool) -> Vec<Category> {
    entries()
        .iter()
        .filter(|entry| !current || entry.supports_current)
        .map(|entry| entry.category)
        .collect()
}

pub fn unsupported_for_current(requested: &[Category]) -> Vec<Category> {
    requested.iter().copied().filter(|category| !entry(*category).supports_current).collect()
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
) -> Result<CategorySelection, crate::error::AppError> {
    let (resolved, origin) = if all {
        (categories_for_mode(current), RequestOrigin::All)
    } else if categories.is_empty() {
        (categories_for_mode(current), RequestOrigin::Implicit)
    } else {
        (unique_categories(categories.to_vec()), RequestOrigin::Explicit)
    };

    if current {
        let unsupported = unsupported_for_current(&resolved);
        if !unsupported.is_empty() {
            let names =
                unsupported.iter().map(|category| category.as_str()).collect::<Vec<_>>().join(", ");
            return Err(crate::error::AppError::UnsupportedCurrentModeCategory(names));
        }
    }

    Ok(CategorySelection { categories: resolved, origin })
}

pub fn build_targets(categories: &[Category], current: bool) -> Vec<Box<dyn CleanupTarget>> {
    category_order()
        .iter()
        .filter(|category| categories.contains(category))
        .filter(|category| !current || entry(**category).supports_current)
        .map(|category| (entry(*category).build)(current))
        .collect()
}

fn build_xcode(current: bool) -> Box<dyn CleanupTarget> {
    Box::new(XcodeTarget::new(current))
}

fn build_python(_: bool) -> Box<dyn CleanupTarget> {
    Box::new(PythonTarget::new())
}

fn build_rust(_: bool) -> Box<dyn CleanupTarget> {
    Box::new(RustTarget::new())
}

fn build_nodejs(_: bool) -> Box<dyn CleanupTarget> {
    Box::new(NodejsTarget::new())
}

fn build_brew(_: bool) -> Box<dyn CleanupTarget> {
    Box::new(BrewTarget::new())
}

fn build_docker(_: bool) -> Box<dyn CleanupTarget> {
    Box::new(DockerTarget::new())
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
        assert_eq!(categories, category_order());
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
        let targets = build_targets(category_order(), false);
        let target_categories: Vec<Category> =
            targets.iter().map(|target| target.category()).collect();
        assert_eq!(target_categories, category_order());
    }

    #[test]
    fn category_metadata_is_available_from_one_catalog() {
        for category in category_order() {
            let entry = entry(*category);
            assert_eq!(find(entry.id).map(|value| value.category), Some(*category));
            assert!(!entry.display_name.is_empty());
            assert!(!entry.description.is_empty());
        }
    }
}
