mod action;
mod configs;
mod ignores;
mod inits;
mod violation;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::action::{check_file, fix_file, resolve_paths};
use crate::configs::{discover_editorconfigs, file_policy};
use crate::inits::{init_editorconfig, init_ignore_revs};

use clap::Parser;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Action {
    Init,
    InitIgnoreRevs,
    #[default]
    Check,
    Fix,
}

#[derive(Parser)]
#[command(
    name = "offwhite",
    about = "Check and fix trailing whitespace and final newlines",
    after_help = "Offwhite enforces trim_trailing_whitespace and insert_final_newline as specified in .editorconfig.",
    group = clap::ArgGroup::new("action")
        .args(["init", "init_ignore_revs", "check", "fix"]),
)]
pub(crate) struct Cli {
    /// Create a default .editorconfig in the current directory
    #[arg(long)]
    init: bool,

    /// Create a default .git-blame-ignore-revs in the current directory
    #[arg(long = "init-ignore-revs")]
    init_ignore_revs: bool,

    /// Check for violations (default)
    #[arg(long)]
    check: bool,

    /// Fix files in place
    #[arg(long)]
    fix: bool,

    /// Suppress warnings
    #[arg(short, long, conflicts_with = "verbose")]
    quiet: bool,

    /// Increase logging output
    #[arg(short, long, conflicts_with = "quiet")]
    verbose: bool,

    /// Enforce exactly one trailing newline (by default, one or more is accepted)
    #[arg(long = "single-final-newline")]
    single_final_newline: bool,

    /// Do not respect .gitignore files (respected by default)
    #[arg(long = "no-gitignore")]
    no_gitignore: bool,

    /// Files or directories to process (supports quoted glob patterns)
    #[arg(default_value = ".")]
    paths: Vec<String>,
}

impl Cli {
    fn action(&self) -> Action {
        if self.init {
            Action::Init
        } else if self.init_ignore_revs {
            Action::InitIgnoreRevs
        } else if self.fix {
            Action::Fix
        } else {
            Action::Check
        }
    }

    fn verbosity(&self) -> Verbosity {
        match (self.quiet, self.verbose) {
            (true, _) => Verbosity::Quiet,
            (_, true) => Verbosity::Verbose,
            _ => Verbosity::Normal,
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let action = cli.action();

    match action {
        Action::Init => {
            return if init_editorconfig() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Action::InitIgnoreRevs => {
            return if init_ignore_revs() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            };
        }
        Action::Check | Action::Fix => {}
    }

    let verbosity = cli.verbosity();

    // Discover .editorconfig files from the current directory upward.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let editorconfigs = discover_editorconfigs(&cwd);

    if editorconfigs.is_empty() {
        if verbosity >= Verbosity::Normal {
            eprintln!("warning: no .editorconfig files found; nothing checked");
        }
        return ExitCode::FAILURE;
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
        let mut policy = file_policy(path);
        policy.single_final_newline = cli.single_final_newline;
        if !policy.trim_trailing_whitespace && !policy.insert_final_newline {
            continue;
        }

        match action {
            Action::Fix => {
                if let Err(e) = fix_file(path, policy) {
                    eprintln!("{}: error fixing: {e}", path.display());
                    found_violations = true;
                }
            }
            Action::Check => match check_file(path, policy) {
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
            },
            Action::Init | Action::InitIgnoreRevs => unreachable!(),
        }
    }

    if found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
