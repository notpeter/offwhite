#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use ec4rs::glob::Glob;
use ec4rs::property::{FinalNewline, TrimTrailingWs};
use grep::matcher::Matcher;
use grep::searcher::Searcher;
use grep::searcher::sinks::UTF8;
use ignore::WalkBuilder;
use ignore::overrides::OverrideBuilder;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Parser)]
#[command(
    name = "offwhite",
    about = "Check and fix trailing whitespace and final newlines"
)]
struct Cli {
    /// Check for violations (default)
    #[arg(long, conflicts_with = "fix")]
    check: bool,

    /// Fix files in place
    #[arg(long, conflicts_with = "check")]
    fix: bool,

    /// Suppress warnings
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Increase logging output
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    /// Do not respect .gitignore files
    #[arg(long = "no-gitignore")]
    no_gitignore: bool,

    /// Files or directories to process (supports glob patterns, must be quoted)
    #[arg(default_value = ".")]
    paths: Vec<String>,
}

pub struct Violation {
    pub path: PathBuf,
    pub line: u64,
    pub kind: ViolationKind,
}

pub enum ViolationKind {
    TrailingWhitespace,
    NoFinalNewline,
    ExtraFinalNewlines,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.path.display();
        let line = self.line;
        match &self.kind {
            ViolationKind::TrailingWhitespace => {
                write!(f, "{path}:{line}: trailing whitespace")
            }
            ViolationKind::NoFinalNewline => {
                write!(f, "{path}:{line}: no final newline")
            }
            ViolationKind::ExtraFinalNewlines => {
                write!(f, "{path}:{line}: multiple trailing newlines")
            }
        }
    }
}

/// What to check/fix for a given file, derived from .editorconfig.
#[derive(Clone, Copy)]
pub struct FilePolicy {
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
}

/// Look up .editorconfig properties for a file path.
pub fn file_policy(path: &Path) -> FilePolicy {
    let props = ec4rs::properties_of::<Glob>(path).unwrap_or_default();

    let trim = matches!(
        props.get::<TrimTrailingWs>(),
        Ok(TrimTrailingWs::Value(true))
    );
    let newline = matches!(props.get::<FinalNewline>(), Ok(FinalNewline::Value(true)));

    FilePolicy {
        trim_trailing_whitespace: trim,
        insert_final_newline: newline,
    }
}

/// Discover .editorconfig files by walking up from `start` to the filesystem root.
/// Returns them in order from nearest to farthest. Stops if one contains `root = true`.
fn discover_editorconfigs(start: &Path) -> Vec<PathBuf> {
    let start = start
        .canonicalize()
        .unwrap_or_else(|_| start.to_path_buf());
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
            } else if contents.ends_with(b"\n\n") {
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

    if policy.insert_final_newline {
        // Remove trailing empty lines
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

/// Default ignore patterns applied to all walks and glob results.
const DEFAULT_IGNORE_GLOBS: &[&str] = &["!.git/", "!*.patch", "!*.diff", "!*.rej", "!*.patchset"];

fn build_overrides(base: &Path, include_glob: Option<&str>) -> ignore::overrides::Override {
    let mut builder = OverrideBuilder::new(base);
    for pat in DEFAULT_IGNORE_GLOBS {
        builder.add(pat).expect("invalid default ignore glob");
    }
    if let Some(glob) = include_glob {
        builder.add(glob).expect("invalid include glob");
    }
    builder.build().expect("failed to build overrides")
}

fn resolve_paths(patterns: &[String], respect_gitignore: bool) -> Vec<PathBuf> {
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

fn contains_glob_meta(s: &str) -> bool {
    s.contains('*') || s.contains('?') || s.contains('[')
}

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
        .overrides(build_overrides(dir, include_glob))
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

fn main() -> ExitCode {
    let cli = Cli::parse();

    let verbosity = if cli.quiet {
        Verbosity::Quiet
    } else if cli.verbose {
        Verbosity::Verbose
    } else {
        Verbosity::Normal
    };

    // Discover .editorconfig files from the current directory upward.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let editorconfigs = discover_editorconfigs(&cwd);

    if editorconfigs.is_empty() {
        if verbosity >= Verbosity::Normal {
            eprintln!("warning: no .editorconfig files found; nothing checked");
        }
        return ExitCode::SUCCESS;
    }

    // Log discovered .editorconfig files.
    if verbosity >= Verbosity::Verbose {
        for ec in &editorconfigs {
            eprintln!("info: using {}", ec.display());
        }
    }

    // Warn about .editorconfig files in parent directories.
    if verbosity >= Verbosity::Normal {
        // Collect unique parent dirs from CLI paths to determine what counts as "parent".
        let mut roots = BTreeSet::new();
        for pattern in &cli.paths {
            if contains_glob_meta(pattern) {
                roots.insert(cwd.clone());
            } else {
                let p = PathBuf::from(pattern);
                let dir = if p.is_file() {
                    p.parent().map(Path::to_path_buf).unwrap_or(cwd.clone())
                } else {
                    p.clone()
                };
                roots.insert(dir.canonicalize().unwrap_or(dir));
            }
        }

        for ec in &editorconfigs {
            let ec_dir = ec.parent().unwrap_or(Path::new("."));
            let ec_dir = ec_dir.canonicalize().unwrap_or(ec_dir.to_path_buf());
            let is_parent = roots.iter().all(|root| ec_dir != *root);
            if is_parent {
                eprintln!("warning: using .editorconfig from parent directory: {}", ec.display());
            }
        }
    }

    let files = resolve_paths(&cli.paths, !cli.no_gitignore);

    if files.is_empty() {
        if verbosity >= Verbosity::Normal {
            eprintln!("warning: no files found");
        }
        return ExitCode::SUCCESS;
    }

    let mut found_violations = false;

    for path in &files {
        let policy = file_policy(path);
        if !policy.trim_trailing_whitespace && !policy.insert_final_newline {
            continue;
        }

        if cli.fix {
            if let Err(e) = fix_file(path, policy) {
                eprintln!("{}: error fixing: {e}", path.display());
                found_violations = true;
            }
        } else {
            match check_file(path, policy) {
                Ok(violations) => {
                    for v in &violations {
                        println!("{v}");
                    }
                    if !violations.is_empty() {
                        found_violations = true;
                    }
                }
                Err(e) => {
                    eprintln!("{}: error: {e}", path.display());
                    found_violations = true;
                }
            }
        }
    }

    if found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
