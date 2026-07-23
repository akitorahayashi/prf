use std::fmt;

use clap::{ValueEnum, builder::PossibleValue};

use super::catalog;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Xcode,
    Python,
    Rust,
    Nodejs,
    Brew,
    Docker,
}

impl Category {
    pub fn from_name(value: &str) -> Option<Self> {
        catalog::find(value).map(|entry| entry.category)
    }

    pub fn as_str(&self) -> &'static str {
        catalog::entry(*self).id
    }

    pub fn display_name(&self) -> &'static str {
        catalog::entry(*self).display_name
    }

    pub fn supports_current_mode(&self, current: bool) -> bool {
        !current || catalog::entry(*self).supports_current
    }
}

impl std::str::FromStr for Category {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Category::from_name(s).ok_or_else(|| format!("Unknown category '{s}'"))
    }
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl ValueEnum for Category {
    fn value_variants<'a>() -> &'a [Self] {
        catalog::category_order()
    }

    fn to_possible_value(&self) -> Option<PossibleValue> {
        let entry = catalog::entry(*self);
        Some(PossibleValue::new(entry.id).help(entry.description))
    }
}
