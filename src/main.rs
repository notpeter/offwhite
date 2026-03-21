mod action;
mod args;
mod configs;
mod ignores;
mod inits;
mod violation;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::action::{RunState, process_file, walk_paths};
use crate::args::{Action, CliOutcome, Verbosity, parse_cli};
use crate::configs::{PolicyCache, RootConfigStatus};
use crate::inits::{init_editorconfig, init_ignore_revs};

fn main() -> ExitCode {
    let cli = match parse_cli() {
        CliOutcome::Run(cli) => cli,
        CliOutcome::Exit(code) => return code,
    };
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
    let mut policy_cache = PolicyCache::new();

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    match policy_cache.root_config_status(&cwd) {
        RootConfigStatus::Ready => {}
        RootConfigStatus::Missing => {
            if verbosity >= Verbosity::Normal {
                eprintln!("warning: no root .editorconfig file found; nothing checked");
            }
            return ExitCode::FAILURE;
        }
        RootConfigStatus::MissingUtf8 => {
            if verbosity >= Verbosity::Normal {
                eprintln!(
                    "warning: root .editorconfig must contain `charset = utf-8` in a `[*]` section; nothing checked"
                );
            }
            return ExitCode::FAILURE;
        }
    }

    let mut run_state = RunState::new();
    walk_paths(cli.paths(), !cli.no_ignore(), |path| {
        process_file(
            &path,
            action,
            verbosity,
            cli.single_final_newline(),
            &mut policy_cache,
            &mut run_state,
        );
    });

    if run_state.found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
