mod action;
mod configs;
mod ignores;
mod inits;
mod violation;

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::action::{check_file, contains_glob_meta, fix_file, resolve_paths};
use crate::configs::{discover_editorconfigs, file_policy};
use crate::inits::{init_editorconfig, init_ignore_revs};

use clap::Parser;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
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

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.init {
        match init_editorconfig() {
            true => return ExitCode::SUCCESS,
            false => return ExitCode::FAILURE,
        }
    }

    if cli.init_ignore_revs {
        match init_ignore_revs() {
            true => return ExitCode::SUCCESS,
            false => return ExitCode::FAILURE,
        };
    }

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
        return ExitCode::FAILURE;
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
                eprintln!(
                    "warning: using .editorconfig from parent directory: {}",
                    ec.display()
                );
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
        let mut policy = file_policy(path);
        policy.single_final_newline = cli.single_final_newline;
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
