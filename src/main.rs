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
use crate::configs::PolicyCache;
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

    let mut warned_root_configs = std::collections::HashSet::new();
    for path in cli.paths() {
        let target = PathBuf::from(path);
        if !target.exists() {
            continue;
        }

        let Some(root_config) = policy_cache.root_config(&target) else {
            if verbosity >= Verbosity::Normal {
                eprintln!(
                    "warning: {}: no root .editorconfig file found; nothing checked",
                    target.display()
                );
            }
            return Ok(ExitCode::FAILURE);
        };

        if !root_config.has_utf8_section
            && verbosity >= Verbosity::Normal
            && warned_root_configs.insert(root_config.path.clone())
        {
            eprintln!(
                "warning: {}: root .editorconfig does not declare `charset = utf-8` in any section",
                root_config.path.display()
            );
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

    if run_state.scanned_no_files() && verbosity >= Verbosity::Normal {
        eprintln!("warning: no files scanned; no matching .editorconfig sections enabled scanning");
    }

    Ok(if run_state.found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}
