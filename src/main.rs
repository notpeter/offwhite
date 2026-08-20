mod action;
mod args;
mod configs;
mod ignores;
mod inits;
mod list;
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
use crate::list::{list_editorconfigs, list_extensions};
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
        Action::InitEditorconfig => {
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
        Action::ListEditorconfig => {
            return Ok(if list_editorconfigs(cli.paths(), !cli.no_ignore())? {
                ExitCode::FAILURE
            } else {
                ExitCode::SUCCESS
            });
        }
        Action::ListExtensions => {
            return Ok(
                if list_extensions(cli.paths(), !cli.no_ignore(), cli.verbosity())? {
                    ExitCode::FAILURE
                } else {
                    ExitCode::SUCCESS
                },
            );
        }
        Action::Check | Action::Fix => {}
    }

    let verbosity = cli.verbosity();
    let mut policy_cache = PolicyCache::new();
    let (scan_paths, mut found_failures) =
        collect_scan_paths(cli.paths(), &mut policy_cache, verbosity);

    let mut run_state = RunState::new();
    walk_paths(&scan_paths, !cli.no_ignore(), |path| {
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

    found_failures |= run_state.found_violations;

    Ok(if found_failures {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    })
}

fn collect_scan_paths(
    paths: &[String],
    policy_cache: &mut PolicyCache,
    verbosity: Verbosity,
) -> (Vec<String>, bool) {
    let mut scan_paths = Vec::new();
    let mut found_failures = false;
    let mut warned_editorconfigs = std::collections::HashSet::new();

    for path in paths {
        let target = PathBuf::from(path);
        if !target.exists() {
            eprintln!("{}: no such file or directory", target.display());
            found_failures = true;
            continue;
        }

        let Some(editorconfig) = policy_cache.editorconfig_for(&target) else {
            if verbosity >= Verbosity::Normal {
                eprintln!(
                    "warning: {}: no .editorconfig file found; nothing checked",
                    target.display()
                );
            }
            found_failures = true;
            continue;
        };

        if !editorconfig.has_utf8_section
            && verbosity >= Verbosity::Normal
            && warned_editorconfigs.insert(editorconfig.path.clone())
        {
            eprintln!(
                "warning: {}: no resolved .editorconfig declares `charset = utf-8` in any section",
                editorconfig.path.display()
            );
        }

        scan_paths.push(path.clone());
    }

    (scan_paths, found_failures)
}
