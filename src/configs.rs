use std::{
    fs,
    path::{Path, PathBuf},
};

/// What to check/fix for a given file, derived from .editorconfig.
#[derive(Clone, Copy)]
pub struct FilePolicy {
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub single_final_newline: bool,
}

/// Look up .editorconfig properties for a file path.
pub fn file_policy(path: &Path) -> FilePolicy {
    let props = ec4rs::properties_of::<ec4rs::glob::Glob>(path).unwrap_or_default();
    FilePolicy {
        trim_trailing_whitespace: matches!(
            props.get::<ec4rs::property::TrimTrailingWs>(),
            Ok(ec4rs::property::TrimTrailingWs::Value(true))
        ),
        insert_final_newline: matches!(
            props.get::<ec4rs::property::FinalNewline>(),
            Ok(ec4rs::property::FinalNewline::Value(true))
        ),
        single_final_newline: false,
    }
}

/// Discover .editorconfig files by walking up from `start` to the filesystem root.
/// Returns them in order from nearest to farthest. Stops if one contains `root = true`.
pub(crate) fn discover_editorconfigs(start: &Path) -> Vec<PathBuf> {
    let start = start.canonicalize().unwrap_or_else(|_| start.to_path_buf());
    let mut found = Vec::new();
    let mut dir = if start.is_file() {
        start.parent().map(Path::to_path_buf)
    } else {
        Some(start)
    };

    while let Some(d) = dir {
        let candidate = d.join(".editorconfig");
        if candidate.is_file() {
            let is_root = fs::read_to_string(&candidate)
                .map(|s| {
                    s.lines().any(|line| {
                        let line = line.trim();
                        line.eq_ignore_ascii_case("root = true")
                            || line.eq_ignore_ascii_case("root=true")
                    })
                })
                .unwrap_or(false);
            found.push(candidate);
            if is_root {
                break;
            }
        }
        dir = d.parent().map(Path::to_path_buf);
    }

    found
}
