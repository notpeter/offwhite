use std::fs;
use std::path::{Path, PathBuf};

use crate::configs::{FilePolicy, LineEnding};
use crate::ignores::build_default_ignores;
use crate::violation::{Violation, ViolationKind};

use ignore::WalkBuilder;

fn walk_dir(dir: &Path, respect_ignore_files: bool, mut on_file: impl FnMut(PathBuf)) {
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

    let walker = builder.build();

    for entry in walker {
        match entry {
            Ok(e) if e.file_type().is_some_and(|ft| ft.is_file()) => {
                on_file(e.into_path());
            }
            Err(e) => eprintln!("walk error: {e}"),
            _ => {}
        }
    }
}

pub(crate) fn walk_paths(
    paths: &[String],
    respect_ignore_files: bool,
    mut on_file: impl FnMut(PathBuf),
) {
    for path in paths {
        let path = PathBuf::from(path);
        if path.is_file() {
            on_file(path);
        } else if path.is_dir() {
            walk_dir(&path, respect_ignore_files, &mut on_file);
        } else {
            eprintln!("{}: no such file or directory", path.display());
        }
    }
}

#[derive(Clone)]
struct ParsedLine {
    text: String,
    ending: Option<LineEnding>,
}

#[derive(Clone, Copy)]
struct ScanState {
    total_lines: u64,
    previous_line_ended: bool,
    last_line_ended: bool,
    last_line_empty: bool,
}

impl ScanState {
    const fn new() -> Self {
        Self {
            total_lines: 0,
            previous_line_ended: false,
            last_line_ended: false,
            last_line_empty: false,
        }
    }
}

fn scan_lines(contents: &str, mut on_line: impl FnMut(u64, &str, Option<LineEnding>)) -> ScanState {
    let bytes = contents.as_bytes();
    let mut start = 0;
    let mut i = 0;
    let mut line_number = 1_u64;
    let mut state = ScanState::new();

    while i < bytes.len() {
        let ending = match bytes[i] {
            b'\n' => Some((LineEnding::Lf, 1)),
            b'\r' if i + 1 < bytes.len() && bytes[i + 1] == b'\n' => Some((LineEnding::CrLf, 2)),
            b'\r' => Some((LineEnding::Cr, 1)),
            _ => None,
        };

        if let Some((line_ending, width)) = ending {
            let text = &contents[start..i];
            on_line(line_number, text, Some(line_ending));
            state.previous_line_ended = state.last_line_ended;
            state.last_line_ended = true;
            state.last_line_empty = text.is_empty();
            state.total_lines += 1;
            line_number += 1;
            i += width;
            start = i;
        } else {
            i += 1;
        }
    }

    if start < contents.len() {
        let text = &contents[start..];
        on_line(line_number, text, None);
        state.previous_line_ended = state.last_line_ended;
        state.last_line_ended = false;
        state.last_line_empty = text.is_empty();
        state.total_lines += 1;
    }

    state
}

fn parse_lines(contents: &str) -> Vec<ParsedLine> {
    let mut lines = Vec::new();
    scan_lines(contents, |_, text, ending| {
        lines.push(ParsedLine {
            text: text.to_string(),
            ending,
        });
    });

    lines
}

fn inferred_line_ending(lines: &[ParsedLine]) -> LineEnding {
    lines
        .iter()
        .find_map(|line| line.ending)
        .unwrap_or(LineEnding::Lf)
}

fn render_lines(lines: &[ParsedLine]) -> String {
    let capacity = lines.iter().fold(0, |total, line| {
        total
            + line.text.len()
            + match line.ending {
                Some(LineEnding::Lf | LineEnding::Cr) => 1,
                Some(LineEnding::CrLf) => 2,
                None => 0,
            }
    });
    let mut output = String::with_capacity(capacity);
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

fn emit_check_violations(
    path: &Path,
    line_number: u64,
    text: &str,
    ending: Option<LineEnding>,
    policy: FilePolicy,
    mut on_violation: impl FnMut(Violation),
) {
    if policy.trim_trailing_whitespace && text.ends_with([' ', '\t']) {
        on_violation(Violation {
            path: path.to_owned(),
            line: line_number,
            kind: ViolationKind::TrailingWhitespace,
        });
    }

    if let (Some(expected), Some(found)) = (policy.end_of_line, ending) {
        if found != expected {
            on_violation(Violation {
                path: path.to_owned(),
                line: line_number,
                kind: ViolationKind::IncorrectLineEnding { expected, found },
            });
        }
    }
}

pub fn check_file_with(
    path: &Path,
    policy: FilePolicy,
    mut on_violation: impl FnMut(Violation),
) -> Result<(), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let state = scan_lines(&contents, |line_number, text, ending| {
        emit_check_violations(path, line_number, text, ending, policy, &mut on_violation);
    });

    if policy.insert_final_newline {
        if !contents.is_empty() {
            if !state.last_line_ended {
                on_violation(Violation {
                    path: path.to_owned(),
                    line: state.total_lines,
                    kind: ViolationKind::NoFinalNewline,
                });
            } else if policy.single_final_newline
                && state.previous_line_ended
                && state.last_line_empty
            {
                on_violation(Violation {
                    path: path.to_owned(),
                    line: state.total_lines,
                    kind: ViolationKind::ExtraFinalNewlines,
                });
            }
        }
    }

    Ok(())
}

pub fn fix_file(path: &Path, policy: FilePolicy) -> Result<(), Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let mut lines = parse_lines(&contents);
    let mut changed = false;

    if policy.trim_trailing_whitespace {
        for line in &mut lines {
            let trimmed_len = line.text.trim_end_matches([' ', '\t']).len();
            if trimmed_len != line.text.len() {
                line.text.truncate(trimmed_len);
                changed = true;
            }
        }
    }

    if policy.single_final_newline {
        // Remove trailing empty lines to enforce exactly one final newline.
        while lines
            .last()
            .is_some_and(|line| line.ending.is_some() && line.text.is_empty())
        {
            lines.pop();
            changed = true;
        }
    }

    if matches!(lines.as_slice(), [ParsedLine { text, .. }] if text.is_empty()) {
        lines.clear();
        changed = true;
    }

    let inferred = inferred_line_ending(&lines);
    let target_ending = policy.end_of_line.unwrap_or(inferred);

    if policy.end_of_line.is_some() {
        for line in &mut lines {
            if line.ending.is_some() && line.ending != Some(target_ending) {
                line.ending = Some(target_ending);
                changed = true;
            }
        }
    }

    // Ensure exactly one final newline (unless file is empty).
    if policy.insert_final_newline && !lines.is_empty() {
        if let Some(last) = lines.last_mut() {
            if last.ending != Some(target_ending) {
                last.ending = Some(target_ending);
                changed = true;
            }
        }
    }

    if !changed {
        return Ok(());
    }

    let output = render_lines(&lines);
    fs::write(path, output)?;
    Ok(())
}
