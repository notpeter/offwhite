use std::process::ExitCode;

use pico_args::Arguments;

const ROOT_HELP_TEMPLATE: &str = concat!(
    "Offwhite enforces .editconfig whitespace and newline settings\n",
    "\n",
    "Usage:\n",
    "  offwhite check [OPTIONS] [PATHS]...       Check for violations\n",
    "  offwhite fix [OPTIONS] [PATHS]...         Fix files in place\n",
    "  offwhite init                             Create an example .editorconfig\n",
    "  offwhite init-ignore-revs                 Create an example .git-blame-ignore-revs\n",
    "\n",
    "Options:\n",
    "  -q, --quiet                 Suppress warnings\n",
    "  -v, --verbose               Increase logging output\n",
    "      --single-final-newline  Enforce exactly one trailing newline (disabled by default)\n",
    "      --no-ignore             Do not respect .ignore or .gitignore files\n",
    "  -h, --help                  Print help\n",
    "\n",
);
const ACTION_NAMES: &[&str] = &["init", "init-ignore-revs", "check", "fix"];
const PARSE_ERROR_FOOTER: &str = "\nFor more information, try '--help'.";

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

pub(crate) struct Cli {
    options: Options,
    action: Action,
    paths: Vec<String>,
}

#[derive(Default)]
struct Options {
    quiet: bool,
    verbose: bool,
    single_final_newline: bool,
    no_ignore: bool,
}

impl Cli {
    pub(crate) fn action(&self) -> Action {
        self.action
    }

    pub(crate) fn paths(&self) -> &[String] {
        &self.paths
    }

    pub(crate) fn verbosity(&self) -> Verbosity {
        match (self.options.quiet, self.options.verbose) {
            (true, _) => Verbosity::Quiet,
            (_, true) => Verbosity::Verbose,
            _ => Verbosity::Normal,
        }
    }

    pub(crate) fn single_final_newline(&self) -> bool {
        self.options.single_final_newline
    }

    pub(crate) fn no_ignore(&self) -> bool {
        self.options.no_ignore
    }
}

pub(crate) enum CliOutcome {
    Run(Cli),
    Exit(ExitCode),
}

pub(crate) fn parse_cli() -> CliOutcome {
    let args: Vec<_> = std::env::args_os().skip(1).collect();

    if let Some(outcome) = maybe_handle_help(&args) {
        return outcome;
    }

    match parse_cli_args(normalize_args(args)) {
        Ok(cli) => CliOutcome::Run(cli),
        Err(message) => {
            eprintln!("error: {message}{PARSE_ERROR_FOOTER}");
            CliOutcome::Exit(ExitCode::from(2))
        }
    }
}

fn parse_cli_args(args: Vec<std::ffi::OsString>) -> Result<Cli, String> {
    let mut input = Arguments::from_vec(args);
    let options = parse_options(&mut input)?;
    let action = parse_action(&mut input)?;
    let paths = parse_paths(input.finish(), action)?;

    Ok(Cli {
        options,
        action,
        paths,
    })
}

fn parse_options(input: &mut Arguments) -> Result<Options, String> {
    let options = Options {
        quiet: input.contains(["-q", "--quiet"]),
        verbose: input.contains(["-v", "--verbose"]),
        single_final_newline: input.contains("--single-final-newline"),
        no_ignore: input.contains(["-u", "--no-ignore"]),
    };

    if options.quiet && options.verbose {
        return Err("the '--quiet' and '--verbose' options cannot be used together".into());
    }

    Ok(options)
}

fn parse_action(input: &mut Arguments) -> Result<Action, String> {
    let Some(command) = input
        .opt_free_from_str::<String>()
        .map_err(|err| err.to_string())?
    else {
        return Err("internal error: missing default action".into());
    };

    parse_action_name(&command).ok_or_else(|| format!("unrecognized subcommand '{command}'"))
}

fn parse_paths(remaining: Vec<std::ffi::OsString>, action: Action) -> Result<Vec<String>, String> {
    match action {
        Action::Check | Action::Fix => parse_check_paths(remaining),
        Action::Init | Action::InitIgnoreRevs => parse_init_args(remaining),
    }
}

fn parse_check_paths(remaining: Vec<std::ffi::OsString>) -> Result<Vec<String>, String> {
    let mut paths = Vec::new();
    let mut literal_paths = false;

    for arg in remaining {
        if !literal_paths && arg == "--" {
            literal_paths = true;
            continue;
        }

        let value = arg
            .into_string()
            .map_err(|_| "argument is not a UTF-8 string".to_string())?;
        if !literal_paths && value.starts_with('-') {
            return Err(format!("unrecognized option '{value}'"));
        }
        paths.push(value);
    }

    if paths.is_empty() {
        paths.push(".".into());
    }

    Ok(paths)
}

fn parse_init_args(remaining: Vec<std::ffi::OsString>) -> Result<Vec<String>, String> {
    if let Some(arg) = remaining.into_iter().next() {
        let value = arg
            .into_string()
            .map_err(|_| "argument is not a UTF-8 string".to_string())?;
        if value.starts_with('-') {
            Err(format!("unrecognized option '{value}'"))
        } else {
            Err(format!("unexpected argument '{value}'"))
        }
    } else {
        Ok(Vec::new())
    }
}

fn maybe_handle_help(args: &[std::ffi::OsString]) -> Option<CliOutcome> {
    if args.iter().any(|arg| is_help_arg(arg)) {
        print!("{ROOT_HELP_TEMPLATE}");
        Some(CliOutcome::Exit(ExitCode::SUCCESS))
    } else {
        None
    }
}

fn normalize_args<I>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let args: Vec<_> = args.into_iter().collect();
    if args.is_empty() {
        return vec![Action::Check.as_str().into()];
    }

    if args.iter().any(|arg| is_help_arg(arg)) {
        return args;
    }

    match first_positional_arg(&args) {
        Some((_, value)) if is_action_name(value) => args,
        Some((idx, _)) => insert_default_check(args, idx),
        None => {
            let mut normalized = args;
            normalized.push(Action::Check.as_str().into());
            normalized
        }
    }
}

fn is_help_arg(arg: &std::ffi::OsStr) -> bool {
    matches!(arg.to_str(), Some("-h" | "--help"))
}

fn first_positional_arg(args: &[std::ffi::OsString]) -> Option<(usize, &std::ffi::OsStr)> {
    args.iter()
        .enumerate()
        .find(|(_, arg)| arg.as_os_str() == "--" || !arg.to_string_lossy().starts_with('-'))
        .map(|(idx, arg)| (idx, arg.as_os_str()))
}

fn is_action_name(arg: &std::ffi::OsStr) -> bool {
    arg.to_str()
        .is_some_and(|value| ACTION_NAMES.contains(&value))
}

fn parse_action_name(name: &str) -> Option<Action> {
    match name {
        "init" => Some(Action::Init),
        "init-ignore-revs" => Some(Action::InitIgnoreRevs),
        "check" => Some(Action::Check),
        "fix" => Some(Action::Fix),
        _ => None,
    }
}

fn insert_default_check(args: Vec<std::ffi::OsString>, idx: usize) -> Vec<std::ffi::OsString> {
    let mut normalized = args[..idx].to_vec();
    normalized.push(Action::Check.as_str().into());
    normalized.extend_from_slice(&args[idx..]);
    normalized
}
