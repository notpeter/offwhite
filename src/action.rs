use std::fs;
use std::path::{Path, PathBuf};

use crate::configs::{FilePolicy, LineEnding};
use crate::ignores::{build_default_ignores, build_ignore_overrides};
use crate::violation::{Violation, ViolationKind};

use ignore::WalkBuilder;

fn walk_dir(
    dir: &Path,
    respect_ignore_files: bool,
    include_glob: Option<&str>,
    out: &mut Vec<PathBuf>,
) {
    let mut builder = WalkBuilder::new(dir);
    let default_ignores = build_default_ignores(dir);
    builder
        .ignore(respect_ignore_files)
        .git_ignore(respect_ignore_files)
        .git_global(respect_ignore_files)
        .git_exclude(respect_ignore_files)
        .hidden(false);
    builder.filter_entry(move |entry| {
        !default_ignores
            .matched(
                entry.path(),
                entry.file_type().is_some_and(|ft| ft.is_dir()),
            )
            .is_ignore()
    });

    if let Some(glob) = include_glob {
        builder.overrides(build_ignore_overrides(dir, Some(glob)));
    }

    let walker = builder.build();

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

pub(crate) fn normalize_cli_pattern(s: &str) -> &str {
    s.strip_prefix("./").unwrap_or(s)
}

pub(crate) fn resolve_paths(patterns: &[String], respect_ignore_files: bool) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for pattern in patterns {
        let pattern = normalize_cli_pattern(pattern);
        if contains_glob_meta(pattern) {
            walk_dir(
                Path::new("."),
                respect_ignore_files,
                Some(pattern),
                &mut files,
            );
        } else {
            let path = PathBuf::from(pattern);
            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                walk_dir(&path, respect_ignore_files, None, &mut files);
            } else {
                eprintln!("{}: no such file or directory", path.display());
            }
        }
    }

    files.sort();
    files.dedup();
    files
}

#[derive(Clone)]
struct ParsedLine {
    text: String,
    ending: Option<LineEnding>,
}

fn parse_lines(contents: &str) -> Vec<ParsedLine> {
    let bytes = contents.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut i = 0;

    while i < bytes.len() {
        let ending = match bytes[i] {
            b'\n' => Some((LineEnding::Lf, 1)),
            b'\r' if i + 1 < bytes.len() && bytes[i + 1] == b'\n' => Some((LineEnding::CrLf, 2)),
            b'\r' => Some((LineEnding::Cr, 1)),
            _ => None,
        };

        if let Some((line_ending, width)) = ending {
            lines.push(ParsedLine {
                text: contents[start..i].to_string(),
                ending: Some(line_ending),
            });
            i += width;
            start = i;
        } else {
            i += 1;
        }
    }

    if start < contents.len() {
        lines.push(ParsedLine {
            text: contents[start..].to_string(),
            ending: None,
        });
    }

    lines
}

fn inferred_line_ending(lines: &[ParsedLine]) -> LineEnding {
    lines
        .iter()
        .find_map(|line| line.ending)
        .unwrap_or(LineEnding::Lf)
}

fn has_extra_final_newlines(lines: &[ParsedLine]) -> bool {
    matches!(
        lines,
        [.., ParsedLine { ending: Some(_), .. }, ParsedLine { text, ending: Some(_), .. }]
            if text.is_empty()
    )
}

fn render_lines(lines: &[ParsedLine]) -> String {
    let mut output = String::new();
    for line in lines {
        output.push_str(&line.text);
        if let Some(ending) = line.ending {
            output.push_str(match ending {
                LineEnding::Lf => "\n",
                LineEnding::CrLf => "\r\n",
                LineEnding::Cr => "\r",
            });
        }
    }
    output
}

pub fn check_file(
    path: &Path,
    policy: FilePolicy,
) -> Result<Vec<Violation>, Box<dyn std::error::Error>> {
    let mut violations = Vec::new();
    let contents = fs::read_to_string(path)?;
    let lines = parse_lines(&contents);

    if policy.trim_trailing_whitespace {
        for (idx, line) in lines.iter().enumerate() {
            if line.text.ends_with([' ', '\t']) {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: idx as u64 + 1,
                    kind: ViolationKind::TrailingWhitespace,
                });
            }
        }
    }

    if let Some(expected) = policy.end_of_line {
        for (idx, line) in lines.iter().enumerate() {
            if let Some(found) = line.ending.filter(|found| *found != expected) {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: idx as u64 + 1,
                    kind: ViolationKind::IncorrectLineEnding { expected, found },
                });
            }
        }
    }

    if policy.insert_final_newline {
        if !contents.is_empty() {
            if lines.last().is_some_and(|line| line.ending.is_none()) {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: lines.len() as u64,
                    kind: ViolationKind::NoFinalNewline,
                });
            } else if policy.single_final_newline && has_extra_final_newlines(&lines) {
                violations.push(Violation {
                    path: path.to_owned(),
                    line: lines.len() as u64,
                    kind: ViolationKind::ExtraFinalNewlines,
                });
            }
        }
    }

    Ok(violations)
}

pub fn fix_file(path: &Path, policy: FilePolicy) -> Result<(), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let mut lines = parse_lines(&contents);

    if policy.trim_trailing_whitespace {
        for line in &mut lines {
            line.text
                .truncate(line.text.trim_end_matches([' ', '\t']).len());
        }
    }

    if policy.single_final_newline {
        // Remove trailing empty lines to enforce exactly one final newline.
        while lines
            .last()
            .is_some_and(|line| line.ending.is_some() && line.text.is_empty())
        {
            lines.pop();
        }
    }

    if matches!(lines.as_slice(), [ParsedLine { text, .. }] if text.is_empty()) {
        lines.clear();
    }

    let inferred = inferred_line_ending(&lines);
    let target_ending = policy.end_of_line.unwrap_or(inferred);

    if policy.end_of_line.is_some() {
        for line in &mut lines {
            if line.ending.is_some() {
                line.ending = Some(target_ending);
            }
        }
    }

    // Ensure exactly one final newline (unless file is empty).
    if policy.insert_final_newline && !lines.is_empty() {
        if let Some(last) = lines.last_mut() {
            last.ending = Some(target_ending);
        }
    }

    let output = render_lines(&lines);
    fs::write(path, output)?;
    Ok(())
}
