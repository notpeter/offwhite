mod action;
mod configs;
mod ignores;
mod inits;
mod violation;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::process::ExitCode;

use crate::action::{check_file_with, fix_file, walk_paths};
use crate::configs::PolicyCache;
use crate::inits::{init_editorconfig, init_ignore_revs};
use crate::violation::ViolationKind;

use clap::{Args, Parser, Subcommand};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Action {
    Init,
    InitIgnoreRevs,
    #[default]
    Check,
    Fix,
}

impl Action {
    fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::InitIgnoreRevs => "init-ignore-revs",
            Self::Check => "check",
            Self::Fix => "fix",
        }
    }
}

#[derive(Parser)]
#[command(
    name = "offwhite",
    about = "Offwhite enforces .editconfig whitespace and newline settings",
    override_usage = "offwhite check [OPTIONS] [PATHS]...\n       offwhite fix [OPTIONS] [PATHS]...\n       offwhite init\n       offwhite init-ignore-revs",
    help_template = "{about-section}\nUsage:\n  offwhite check [OPTIONS] [PATHS]...       Check for violations\n  offwhite fix [OPTIONS] [PATHS]...         Fix files in place\n  offwhite init                             Create an example .editorconfig\n  offwhite init-ignore-revs                 Create an example .git-blame-ignore-revs\n\nOptions:\n  -q, --quiet                 Suppress warnings\n  -v, --verbose               Increase logging output\n      --single-final-newline  Enforce exactly one trailing newline (disabled by default)\n      --no-ignore             Do not respect .ignore or .gitignore files\n  -h, --help                  Print help\n\n",
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    #[command(flatten)]
    options: Options,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Args)]
struct Options {
    /// Suppress warnings
    #[arg(short, long, conflicts_with = "verbose", global = true)]
    quiet: bool,

    /// Increase logging output
    #[arg(short, long, conflicts_with = "quiet", global = true)]
    verbose: bool,

    /// Enforce exactly one trailing newline (disabled by default)
    #[arg(long = "single-final-newline", global = true)]
    single_final_newline: bool,

    /// Do not respect .ignore or .gitignore files
    #[arg(short = 'u', long = "no-ignore", global = true)]
    no_ignore: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Create an example .editorconfig
    Init,

    /// Create a example .git-blame-ignore-revs
    #[command(name = "init-ignore-revs")]
    InitIgnoreRevs,

    /// Check for violations
    Check(CheckArgs),

    /// Fix files in place
    Fix(FixArgs),
}

#[derive(Args)]
struct PathsArgs {
    /// Files or directories to process
    #[arg(default_value = ".")]
    paths: Vec<String>,
}

#[derive(Args)]
#[command(
    about = "Check for violations",
    help_template = "{about-section}\nUsage: offwhite check [OPTIONS] [PATHS]...\n\nArguments:\n  [PATHS]...  Files or directories to process [default: .]\n\nOptions:\n  -q, --quiet                 Suppress warnings\n  -v, --verbose               Increase logging output\n      --single-final-newline  Enforce exactly one trailing newline (disabled by default)\n      --no-ignore             Do not respect .ignore or git ignore files\n  -h, --help                  Print help\n"
)]
struct CheckArgs {
    #[command(flatten)]
    paths: PathsArgs,
}

#[derive(Args)]
#[command(
    about = "Fix files in place",
    help_template = "{about-section}\nUsage: offwhite fix [OPTIONS] [PATHS]...\n\nArguments:\n  [PATHS]...  Files or directories to process [default: .]\n\nOptions:\n  -q, --quiet                 Suppress warnings\n  -v, --verbose               Increase logging output\n      --single-final-newline  Enforce exactly one trailing newline (disabled by default)\n      --no-ignore             Do not respect .ignore or git ignore files\n  -h, --help                  Print help\n"
)]
struct FixArgs {
    #[command(flatten)]
    paths: PathsArgs,
}

impl Cli {
    fn action(&self) -> Action {
        match self.command {
            Some(Command::Check(_)) => Action::Check,
            Some(Command::Fix(_)) => Action::Fix,
            Some(Command::Init) => Action::Init,
            Some(Command::InitIgnoreRevs) => Action::InitIgnoreRevs,
            None => Action::Check,
        }
    }

    fn paths(&self) -> &[String] {
        match &self.command {
            Some(Command::Check(args)) => &args.paths.paths,
            Some(Command::Fix(args)) => &args.paths.paths,
            Some(Command::Init) | Some(Command::InitIgnoreRevs) => &[],
            None => unreachable!("default check command should be normalized before parsing"),
        }
    }

    fn verbosity(&self) -> Verbosity {
        match (self.options.quiet, self.options.verbose) {
            (true, _) => Verbosity::Quiet,
            (_, true) => Verbosity::Verbose,
            _ => Verbosity::Normal,
        }
    }
}

fn parse_cli() -> Cli {
    Cli::parse_from(normalize_args(std::env::args_os()))
}

fn normalize_args<I>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let args: Vec<_> = args.into_iter().collect();
    if args.len() <= 1 {
        return vec![args[0].clone(), Action::Check.as_str().into()];
    }

    if args
        .iter()
        .skip(1)
        .any(|arg| matches!(arg.to_str(), Some("-h" | "--help")))
    {
        return args;
    }

    let action_names = [
        Action::Init,
        Action::InitIgnoreRevs,
        Action::Check,
        Action::Fix,
    ]
    .into_iter()
    .map(|action| action.as_str())
    .collect::<Vec<_>>();

    let first_positional = args
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, arg)| *arg == "--" || !arg.to_string_lossy().starts_with('-'))
        .map(|(idx, arg)| (idx, arg.to_string_lossy().into_owned()));

    match first_positional {
        Some((_, value)) if action_names.iter().any(|name| name == &value) => args,
        Some((idx, value)) if value == "--" => {
            let mut normalized = args[..idx].to_vec();
            normalized.push(Action::Check.as_str().into());
            normalized.extend_from_slice(&args[idx..]);
            normalized
        }
        Some((idx, _)) => {
            let mut normalized = args[..idx].to_vec();
            normalized.push(Action::Check.as_str().into());
            normalized.extend_from_slice(&args[idx..]);
            normalized
        }
        None => {
            let mut normalized = args;
            normalized.push(Action::Check.as_str().into());
            normalized
        }
    }
}

fn main() -> ExitCode {
    let cli = parse_cli();
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
    if !policy_cache.has_editorconfigs(&cwd) {
        if verbosity >= Verbosity::Normal {
            eprintln!("warning: no .editorconfig files found; nothing checked");
        }
        return ExitCode::FAILURE;
    }

    let mut found_violations = false;
    walk_paths(cli.paths(), !cli.options.no_ignore, |path| {
        let mut policy = policy_cache.file_policy(&path);
        policy.single_final_newline = cli.options.single_final_newline;
        if !policy.trim_trailing_whitespace
            && !policy.insert_final_newline
            && policy.end_of_line.is_none()
        {
            return;
        }

        match action {
            Action::Fix => {
                if let Err(e) = fix_file(&path, policy) {
                    eprintln!("{}: error fixing: {e}", path.display());
                    found_violations = true;
                }
            }
            Action::Check => {
                let mut saw_line_ending_mismatch = false;
                match check_file_with(&path, policy, |violation| {
                    found_violations = true;
                    match violation.kind {
                        ViolationKind::IncorrectLineEnding { .. }
                            if verbosity < Verbosity::Verbose =>
                        {
                            if !saw_line_ending_mismatch {
                                saw_line_ending_mismatch = true;
                                println!("{violation}");
                            }
                        }
                        _ => println!("{violation}"),
                    }
                }) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("{}: error: {e}", path.display());
                        found_violations = true;
                    }
                }
            }
            Action::Init | Action::InitIgnoreRevs => unreachable!(),
        }
    });

    if found_violations {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
