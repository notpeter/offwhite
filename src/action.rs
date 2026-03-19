use std::fs;
use std::path::{Path, PathBuf};

use crate::configs::FilePolicy;
use crate::ignores::build_ignore_overrides;
use crate::violation::{Violation, ViolationKind};

use grep::matcher::Matcher;
use grep::searcher::Searcher;
use grep::searcher::sinks::UTF8;
use ignore::WalkBuilder;

fn walk_dir(
    dir: &Path,
    respect_gitignore: bool,
    include_glob: Option<&str>,
    out: &mut Vec<PathBuf>,
) {
    let walker = WalkBuilder::new(dir)
        .git_ignore(respect_gitignore)
        .git_global(respect_gitignore)
        .git_exclude(respect_gitignore)
        .hidden(false)
        .overrides(build_ignore_overrides(dir, include_glob))
        .build();

    for entry in walker {
        match entry {
            Ok(e) if e.file_type().is_some_and(|ft| ft.is_file()) => {
                out.push(e.into_path());
            }
            Err(e) => eprintln!("walk error: {e}"),
            _ => {}
        }
    }
}

pub(crate) fn contains_glob_meta(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[') || s.contains('{')
}

pub(crate) fn resolve_paths(patterns: &[String], respect_gitignore: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for pattern in patterns {
        if contains_glob_meta(pattern) {
            walk_dir(Path::new("."), respect_gitignore, Some(pattern), &mut files);
        } else {
            let path = PathBuf::from(pattern);
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                walk_dir(&path, respect_gitignore, None, &mut files);
            } else {
                eprintln!("{}: no such file or directory", path.display());
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

pub fn check_file(
    path: &Path,
    policy: FilePolicy,
) -> Result<Vec<Violation>, Box<dyn std::error::Error>> {
    let mut violations = Vec::new();

    if policy.trim_trailing_whitespace {
        let matcher = grep::regex::RegexMatcherBuilder::new()
            .multi_line(true)
            .build(r"[ \t]+$")?;

        let mut searcher = Searcher::new();
        searcher.search_path(
            &matcher,
            path,
            UTF8(|line_num, line| {
                if matcher.is_match(line.as_bytes())? {
                    violations.push(Violation {
                        path: path.to_owned(),
                        line: line_num,
                        kind: ViolationKind::TrailingWhitespace,
                    });
                }
                Ok(true)
            }),
        )?;
    }

    if policy.insert_final_newline {
        let contents = fs::read(path)?;
        if !contents.is_empty() {
            let total_lines = contents.iter().filter(|&&b| b == b'\n').count() as u64;

            if !contents.ends_with(b"\n") {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: total_lines + 1,
                    kind: ViolationKind::NoFinalNewline,
                });
            } else if policy.single_final_newline && contents.ends_with(b"\n\n") {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: total_lines,
                    kind: ViolationKind::ExtraFinalNewlines,
                });
            }
        }
    }

    Ok(violations)
}

pub fn fix_file(path: &Path, policy: FilePolicy) -> Result<(), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let mut lines: Vec<&str> = contents.lines().collect();

    if policy.trim_trailing_whitespace {
        lines = lines.into_iter().map(|line| line.trim_end()).collect();
    }

    if policy.single_final_newline {
        // Remove trailing empty lines to enforce exactly one final newline
        while lines.last() == Some(&"") {
            lines.pop();
        }
    }

    let mut output = lines.join("\n");

    // Ensure exactly one final newline (unless file is empty)
    if policy.insert_final_newline && !output.is_empty() {
        output.push('\n');
    } else if !policy.insert_final_newline && contents.ends_with('\n') {
        // Preserve the original final newline if we're not managing it
        output.push('\n');
    }

    fs::write(path, output)?;
    Ok(())
}
