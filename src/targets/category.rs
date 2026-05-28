use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    Xcode,
    Python,
    Rust,
    Nodejs,
    Brew,
    Docker,
    Mise,
}

impl Category {
    pub fn from_name(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "xcode" => Some(Category::Xcode),
            "python" => Some(Category::Python),
            "rust" => Some(Category::Rust),
            "nodejs" => Some(Category::Nodejs),
            "brew" => Some(Category::Brew),
            "docker" => Some(Category::Docker),
            "mise" => Some(Category::Mise),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Category::Xcode => "xcode",
            Category::Python => "python",
            Category::Rust => "rust",
            Category::Nodejs => "nodejs",
            Category::Brew => "brew",
            Category::Docker => "docker",
            Category::Mise => "mise",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Category::Xcode => "Xcode",
            Category::Python => "Python",
            Category::Rust => "Rust",
            Category::Nodejs => "Node.js",
            Category::Brew => "Homebrew",
            Category::Docker => "Docker",
            Category::Mise => "mise",
        }
    }

    pub fn supports_current_mode(&self, current: bool) -> bool {
        !current || !matches!(self, Category::Brew | Category::Docker | Category::Mise)
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
