use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::configs::{FilePolicy, LineEnding};
use crate::ignores::build_default_ignores;
use crate::violation::{Violation, ViolationKind};
use crate::{Action, PolicyCache, Verbosity};

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

pub(crate) struct RunState {
    pub found_violations: bool,
    warned_nested_roots: HashSet<PathBuf>,
}

impl RunState {
    pub(crate) fn new() -> Self {
        Self {
            found_violations: false,
            warned_nested_roots: HashSet::new(),
        }
    }

    fn mark_violation(&mut self) {
        self.found_violations = true;
    }
}

pub(crate) fn process_file(
    path: &Path,
    action: Action,
    verbosity: Verbosity,
    single_final_newline: bool,
    policy_cache: &mut PolicyCache,
    run_state: &mut RunState,
) {
    let decision = policy_cache.file_policy(path);
    if let Some(config_path) = &decision.nested_root_missing_utf8 {
        if verbosity >= Verbosity::Verbose
            && run_state.warned_nested_roots.insert(config_path.clone())
        {
            eprintln!(
                "warning: {}: nested .editorconfig with `root = true` lacks `charset = utf-8` in a `[*]` section. Skipping",
                config_path.display()
            );
        }
        run_state.mark_violation();
        return;
    }

    if decision.skipped_non_utf8_sections && verbosity >= Verbosity::Verbose {
        eprintln!(
            "warning: {}: skipped .editorconfig sections with non-utf-8 charset",
            path.display()
        );
    }

    let mut policy = decision.policy;
    policy.single_final_newline = single_final_newline;
    if !policy.trim_trailing_whitespace
        && !policy.insert_final_newline
        && policy.end_of_line.is_none()
    {
        return;
    }

    match action {
        Action::Fix => process_fix(path, policy, verbosity, run_state),
        Action::Check => process_check(path, policy, verbosity, run_state),
        Action::Init | Action::InitIgnoreRevs => unreachable!(),
    }
}

fn process_fix(path: &Path, policy: FilePolicy, verbosity: Verbosity, run_state: &mut RunState) {
    match fix_file(path, policy) {
        Ok(FileStatus::Processed) => {}
        Ok(FileStatus::InvalidUtf8) => {
            if verbosity >= Verbosity::Normal {
                eprintln!("warning: {}: invalid UTF-8; skipped", path.display());
            }
            run_state.mark_violation();
        }
        Err(e) => {
            eprintln!("{}: error fixing: {e}", path.display());
            run_state.mark_violation();
        }
    }
}

fn process_check(path: &Path, policy: FilePolicy, verbosity: Verbosity, run_state: &mut RunState) {
    let mut saw_line_ending_mismatch = false;
    match check_file_with(path, policy, |violation| {
        run_state.mark_violation();
        match violation.kind {
            ViolationKind::IncorrectLineEnding { .. } if verbosity < Verbosity::Verbose => {
                if !saw_line_ending_mismatch {
                    saw_line_ending_mismatch = true;
                    println!("{violation}");
                }
            }
            _ => println!("{violation}"),
        }
    }) {
        Ok(FileStatus::Processed) => {}
        Ok(FileStatus::InvalidUtf8) => {
            if verbosity >= Verbosity::Normal {
                eprintln!("warning: {}: invalid UTF-8; skipped", path.display());
            }
            run_state.mark_violation();
        }
        Err(e) => {
            eprintln!("{}: error: {e}", path.display());
            run_state.mark_violation();
        }
    }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileStatus {
    Processed,
    InvalidUtf8,
}

#[derive(Clone, Copy)]
struct ValidUtf8<'a>(&'a str);

impl<'a> ValidUtf8<'a> {
    fn new(bytes: &'a [u8]) -> Option<Self> {
        std::str::from_utf8(bytes).ok().map(Self)
    }

    fn from_str(contents: &'a str) -> Self {
        Self(contents)
    }

    fn as_bytes(self) -> &'a [u8] {
        self.0.as_bytes()
    }

    fn is_empty(self) -> bool {
        self.0.is_empty()
    }
}

fn emit_violation<'a>(
    path: &'a Path,
    line: u64,
    kind: ViolationKind,
    on_violation: &mut impl FnMut(Violation<'a>),
) {
    on_violation(Violation { path, line, kind });
}

fn scan_lines_bytes(
    bytes: &[u8],
    mut on_line: impl FnMut(u64, &[u8], Option<LineEnding>),
) -> ScanState {
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
            let text = &bytes[start..i];
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

    if start < bytes.len() {
        let text = &bytes[start..];
        on_line(line_number, text, None);
        state.previous_line_ended = state.last_line_ended;
        state.last_line_ended = false;
        state.last_line_empty = text.is_empty();
        state.total_lines += 1;
    }

    state
}

fn scan_utf8_lines(
    contents: ValidUtf8<'_>,
    mut on_line: impl FnMut(u64, &str, Option<LineEnding>),
) -> ScanState {
    scan_lines_bytes(contents.as_bytes(), |line_number, text, ending| {
        // `text` is a subslice of validated UTF-8 bytes, so it remains valid UTF-8.
        let text = unsafe { std::str::from_utf8_unchecked(text) };
        on_line(line_number, text, ending);
    })
}

fn line_ending_str(ending: LineEnding) -> &'static str {
    match ending {
        LineEnding::Lf => "\n",
        LineEnding::CrLf => "\r\n",
        LineEnding::Cr => "\r",
    }
}

fn flush_pending_empty_lines(
    output: &mut String,
    pending_empty_lines: &mut Vec<LineEnding>,
    policy: FilePolicy,
) {
    for ending in pending_empty_lines.drain(..) {
        output.push_str(line_ending_str(policy.end_of_line.unwrap_or(ending)));
    }
}

pub fn check_file_with<'a>(
    path: &'a Path,
    policy: FilePolicy,
    mut on_violation: impl FnMut(Violation<'a>),
) -> Result<FileStatus, Box<dyn std::error::Error>> {
    let contents = fs::read(path)?;
    let Some(utf8_contents) = ValidUtf8::new(&contents) else {
        return Ok(FileStatus::InvalidUtf8);
    };

    let state = scan_lines_bytes(&contents, |line_number, text, ending| {
        if policy.trim_trailing_whitespace && (text.ends_with(b" ") || text.ends_with(b"\t")) {
            emit_violation(
                path,
                line_number,
                ViolationKind::TrailingWhitespace,
                &mut on_violation,
            );
        }

        if let (Some(expected), Some(found)) = (policy.end_of_line, ending) {
            if found != expected {
                emit_violation(
                    path,
                    line_number,
                    ViolationKind::IncorrectLineEnding { expected, found },
                    &mut on_violation,
                );
            }
        }
    });

    if policy.insert_final_newline {
        if !utf8_contents.is_empty() {
            if !state.last_line_ended {
                emit_violation(
                    path,
                    state.total_lines,
                    ViolationKind::NoFinalNewline,
                    &mut on_violation,
                );
            } else if policy.single_final_newline
                && state.previous_line_ended
                && state.last_line_empty
            {
                emit_violation(
                    path,
                    state.total_lines,
                    ViolationKind::ExtraFinalNewlines,
                    &mut on_violation,
                );
            }
        }
    }

    Ok(FileStatus::Processed)
}

fn fix_would_change(contents: &[u8], policy: FilePolicy) -> bool {
    let mut pending_empty_lines = 0_usize;
    let mut output_is_empty = true;
    let mut output_ends_with_newline = false;
    let mut changed = false;

    scan_lines_bytes(contents, |_, text, ending| {
        if policy.trim_trailing_whitespace && (text.ends_with(b" ") || text.ends_with(b"\t")) {
            changed = true;
        }

        if let (Some(expected), Some(found)) = (policy.end_of_line, ending) {
            if found != expected {
                changed = true;
            }
        }

        let trimmed_is_empty = if policy.trim_trailing_whitespace {
            text.iter().all(|byte| matches!(byte, b' ' | b'\t'))
        } else {
            text.is_empty()
        };

        if ending.is_some() && trimmed_is_empty {
            pending_empty_lines += 1;
            return;
        }

        if pending_empty_lines > 0 {
            output_is_empty = false;
            output_ends_with_newline = true;
            pending_empty_lines = 0;
        }

        if !text.is_empty() || ending.is_some() {
            output_is_empty = false;
        }
        output_ends_with_newline = ending.is_some();
    });

    if pending_empty_lines > 0 {
        if policy.single_final_newline {
            changed = true;
        } else if output_is_empty && pending_empty_lines == 1 {
            changed = true;
        } else {
            output_is_empty = false;
            output_ends_with_newline = true;
        }
    }

    if policy.insert_final_newline && !output_is_empty && !output_ends_with_newline {
        changed = true;
    }

    changed
}

pub fn fix_file(path: &Path, policy: FilePolicy) -> Result<FileStatus, Box<dyn std::error::Error>> {
    let contents = fs::read(path)?;
    let Ok(contents) = String::from_utf8(contents) else {
        return Ok(FileStatus::InvalidUtf8);
    };
    if !fix_would_change(contents.as_bytes(), policy) {
        return Ok(FileStatus::Processed);
    }

    let mut output = String::with_capacity(contents.len());
    let mut pending_empty_lines = Vec::new();
    let mut inferred_ending = None;

    scan_utf8_lines(ValidUtf8::from_str(&contents), |_, text, ending| {
        let trimmed_text = if policy.trim_trailing_whitespace {
            text.trim_end_matches([' ', '\t'])
        } else {
            text
        };

        if let Some(found) = ending {
            inferred_ending.get_or_insert(found);
        }

        if let Some(found) = ending {
            if trimmed_text.is_empty() {
                pending_empty_lines.push(found);
                return;
            }
        }

        flush_pending_empty_lines(&mut output, &mut pending_empty_lines, policy);
        output.push_str(trimmed_text);
        if let Some(found) = ending {
            output.push_str(line_ending_str(policy.end_of_line.unwrap_or(found)));
        }
    });

    if !pending_empty_lines.is_empty() && !policy.single_final_newline {
        if !(output.is_empty() && pending_empty_lines.len() == 1) {
            flush_pending_empty_lines(&mut output, &mut pending_empty_lines, policy);
        }
    }

    if policy.insert_final_newline
        && !output.is_empty()
        && !output.ends_with('\n')
        && !output.ends_with('\r')
    {
        output.push_str(line_ending_str(
            policy
                .end_of_line
                .unwrap_or(inferred_ending.unwrap_or(LineEnding::Lf)),
        ));
    }
    fs::write(path, output)?;
    Ok(FileStatus::Processed)
}
