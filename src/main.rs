mod action;
mod args;
mod configs;
mod ignores;
mod inits;
mod output;
mod violation;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::action::{RunState, process_file, walk_paths};
use crate::args::{Action, CliOutcome, Verbosity, parse_cli};
use crate::configs::{PolicyCache, RootConfigStatus};
use crate::inits::{init_editorconfig, init_ignore_revs};
use crate::output::is_broken_pipe;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) if is_broken_pipe(&err) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: failed writing to stdout: {err}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> std::io::Result<ExitCode> {
    let cli = match parse_cli() {
        CliOutcome::Run(cli) => cli,
        CliOutcome::Exit(code) => return Ok(code),
    };
    let action = cli.action();

    match action {
        Action::Init => {
            return if init_editorconfig() {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::FAILURE)
            };
        }
        Action::InitIgnoreRevs => {
            return if init_ignore_revs() {
                Ok(ExitCode::SUCCESS)
            } else {
                Ok(ExitCode::FAILURE)
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
            return Ok(ExitCode::FAILURE);
        }
        RootConfigStatus::MissingUtf8 => {
            if verbosity >= Verbosity::Normal {
                eprintln!(
                    "warning: root .editorconfig must contain `charset = utf-8` in a `[*]` section; nothing checked"
                );
            }
            return Ok(ExitCode::FAILURE);
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
        )
    })?;

    Ok(if run_state.found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
