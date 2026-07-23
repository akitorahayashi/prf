use crate::cleanup::{Discovery, Rule, ScopeSupport, Target, TargetId};

const RULES: &[Rule] = &[
    Rule::DirectoryNames { names: &["DerivedData"], parent_marker: None },
    Rule::MarkerChildren {
        marker: "Package.swift",
        children: &[".build", ".swiftpm"],
        listing: "SwiftPM Projects (.build, .swiftpm)",
    },
    Rule::HomePaths {
        paths: &[
            "Library/Developer/Xcode/DerivedData",
            "Library/Caches/com.apple.dt.Xcode",
            "Library/Developer/Xcode/DocumentationCache",
            "Library/Developer/Xcode/DocumentationIndex",
            "Library/Developer/Xcode/UserData/Previews",
            "Library/Caches/org.swift.swiftpm",
            "Library/org.swift.swiftpm",
            "Library/Developer/CoreSimulator/Caches",
        ],
    },
];

pub(super) static TARGET: Target =
    Target::new(TargetId::new("xcode"), "Xcode", ScopeSupport::AllModes, Discovery::Rules(RULES));
