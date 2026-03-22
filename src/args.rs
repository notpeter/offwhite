use std::ffi::OsString;
use std::process::ExitCode;

use clap::{Arg, ArgAction, Command};

const ROOT_HELP_TEMPLATE: &str = concat!(
    "Offwhite enforces .editconfig whitespace and newline settings\n",
    "\n",
    "Usage:\n",
    "  offwhite check              Check for violations\n",
    "  offwhite fix                Fix files in place\n",
    "  offwhite list editorconfig  List and validate .editorconfig files\n",
    "  offwhite list extensions    Summarize file extensions in the tree\n",
    "  offwhite init editorconfig  Create an example .editorconfig\n",
    "  offwhite init ignore-revs   Create an example .git-blame-ignore-revs\n",
    "\n",
    "Options:\n",
    "  -q, --quiet                 Suppress warnings\n",
    "  -v, --verbose               Increase logging output\n",
    "      --single-final-newline  Enforce exactly one trailing newline (disabled by default)\n",
    "      --no-ignore             Do not respect .ignore or .gitignore files\n",
    "  -h, --help                  Print help\n",
    "\n",
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verbosity {
    Quiet,
    Normal,
    Verbose,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Action {
    InitEditorconfig,
    InitIgnoreRevs,
    ListEditorconfig,
    ListExtensions,
    #[default]
    Check,
    Fix,
}

impl Action {
    fn top_level_name(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Fix => "fix",
            Self::ListEditorconfig | Self::ListExtensions => "list",
            Self::InitEditorconfig | Self::InitIgnoreRevs => "init",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Cli {
    action: Action,
    paths: Vec<String>,
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
        match (self.quiet, self.verbose) {
            (true, _) => Verbosity::Quiet,
            (_, true) => Verbosity::Verbose,
            _ => Verbosity::Normal,
        }
    }

    pub(crate) fn single_final_newline(&self) -> bool {
        self.single_final_newline
    }

    pub(crate) fn no_ignore(&self) -> bool {
        self.no_ignore
    }
}

pub(crate) enum CliOutcome {
    Run(Cli),
    Exit(ExitCode),
}

pub(crate) fn parse_cli() -> CliOutcome {
    match parse_cli_args(normalize_args(std::env::args_os().skip(1))) {
        Ok(cli) => CliOutcome::Run(cli),
        Err(err) => {
            let _ = err.print();
            CliOutcome::Exit(ExitCode::from(err.exit_code() as u8))
        }
    }
}

fn parse_cli_args(args: Vec<OsString>) -> Result<Cli, clap::Error> {
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push("offwhite".into());
    argv.extend(args);

    let matches = build_cli().try_get_matches_from(argv)?;
    let (command, subcommand) = matches
        .subcommand()
        .expect("default action normalization should always provide a subcommand");
    let (action, action_matches) = match command {
        "check" => (Action::Check, subcommand),
        "fix" => (Action::Fix, subcommand),
        "list" => {
            let (subcommand_name, list_matches) = subcommand
                .subcommand()
                .expect("configured list command should require a subcommand");
            let action = match subcommand_name {
                "editorconfig" => Action::ListEditorconfig,
                "extensions" => Action::ListExtensions,
                _ => unreachable!("configured list subcommand should map to Action"),
            };
            (action, list_matches)
        }
        "init" => {
            let (subcommand_name, init_matches) = subcommand
                .subcommand()
                .expect("configured init command should require a subcommand");
            let action = match subcommand_name {
                "editorconfig" => Action::InitEditorconfig,
                "ignore-revs" => Action::InitIgnoreRevs,
                _ => unreachable!("configured init subcommand should map to Action"),
            };
            (action, init_matches)
        }
        _ => unreachable!("configured subcommand should map to Action"),
    };
    let paths = match action {
        Action::Check | Action::Fix | Action::ListEditorconfig | Action::ListExtensions => {
            action_matches
                .get_many::<String>("paths")
                .map(|paths| paths.cloned().collect())
                .unwrap_or_else(|| vec![".".into()])
        }
        Action::InitEditorconfig | Action::InitIgnoreRevs => Vec::new(),
    };

    Ok(Cli {
        action,
        paths,
        quiet: matches.get_flag("quiet"),
        verbose: matches.get_flag("verbose"),
        single_final_newline: matches.get_flag("single-final-newline"),
        no_ignore: matches.get_flag("no-ignore"),
    })
}

fn build_cli() -> Command {
    Command::new("offwhite")
        .disable_version_flag(true)
        .subcommand_required(true)
        .override_help(ROOT_HELP_TEMPLATE)
        .arg(
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Suppress warnings")
                .conflicts_with("verbose"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Increase logging output")
                .conflicts_with("quiet"),
        )
        .arg(
            Arg::new("single-final-newline")
                .long("single-final-newline")
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Enforce exactly one trailing newline (disabled by default)"),
        )
        .arg(
            Arg::new("no-ignore")
                .long("no-ignore")
                .short_alias('u')
                .action(ArgAction::SetTrue)
                .global(true)
                .help("Do not respect .ignore or .gitignore files"),
        )
        .subcommand(scan_command("check", "Check for violations"))
        .subcommand(scan_command("fix", "Fix files in place"))
        .subcommand(
            Command::new("list")
                .about("List repository metadata")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(list_command(
                    "editorconfig",
                    "List and validate .editorconfig files",
                ))
                .subcommand(list_command(
                    "extensions",
                    "Summarize file extensions in the tree",
                )),
        )
        .subcommand(
            Command::new("init")
                .about("Create example config files")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .subcommand(Command::new("editorconfig").about("Create an example .editorconfig"))
                .subcommand(
                    Command::new("ignore-revs").about("Create an example .git-blame-ignore-revs"),
                ),
        )
}

fn scan_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(Arg::new("paths").value_name("PATHS").num_args(0..))
}

fn list_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(Arg::new("paths").value_name("PATHS").num_args(0..))
}

fn normalize_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    let args: Vec<_> = args.into_iter().collect();
    if args.is_empty() {
        return vec![Action::Check.top_level_name().into()];
    }

    match args
        .iter()
        .enumerate()
        .find(|(_, arg)| arg.as_os_str() == "--" || !arg.to_string_lossy().starts_with('-'))
    {
        Some((_, value))
            if value
                .to_str()
                .is_some_and(|value| matches!(value, "check" | "fix" | "init" | "list")) =>
        {
            args
        }
        Some((idx, _)) => insert_default_check(args, idx),
        None => {
            let mut normalized = args;
            normalized.push(Action::Check.top_level_name().into());
            normalized
        }
    }
}

fn insert_default_check(args: Vec<OsString>, idx: usize) -> Vec<OsString> {
    let mut normalized = args[..idx].to_vec();
    normalized.push(Action::Check.top_level_name().into());
    normalized.extend_from_slice(&args[idx..]);
    normalized
}

#[cfg(test)]
mod tests {
    use super::{Action, Verbosity, normalize_args, parse_cli_args};
    use clap::error::ErrorKind;
    use std::ffi::OsString;

    fn parse(args: &[&str]) -> Result<super::Cli, clap::Error> {
        parse_cli_args(normalize_args(args.iter().map(OsString::from)))
    }

    #[test]
    fn defaults_to_check_and_current_directory() {
        let cli = parse(&[]).unwrap();
        assert_eq!(cli.action(), Action::Check);
        assert_eq!(cli.paths(), ["."]);
        assert_eq!(cli.verbosity(), Verbosity::Normal);
    }

    #[test]
    fn treats_non_action_first_positional_as_check_path() {
        let cli = parse(&["src"]).unwrap();
        assert_eq!(cli.action(), Action::Check);
        assert_eq!(cli.paths(), ["src"]);
    }

    #[test]
    fn accepts_options_after_paths() {
        let cli = parse(&["src", "-q"]).unwrap();
        assert_eq!(cli.action(), Action::Check);
        assert_eq!(cli.paths(), ["src"]);
        assert_eq!(cli.verbosity(), Verbosity::Quiet);
    }

    #[test]
    fn accepts_hidden_short_alias_for_no_ignore() {
        let cli = parse(&["fix", "-u"]).unwrap();
        assert_eq!(cli.action(), Action::Fix);
        assert!(cli.no_ignore());
    }

    #[test]
    fn parses_list_editorconfig_with_default_path() {
        let cli = parse(&["list", "editorconfig"]).unwrap();
        assert_eq!(cli.action(), Action::ListEditorconfig);
        assert_eq!(cli.paths(), ["."]);
    }

    #[test]
    fn parses_list_extensions_with_explicit_path() {
        let cli = parse(&["list", "extensions", "src"]).unwrap();
        assert_eq!(cli.action(), Action::ListExtensions);
        assert_eq!(cli.paths(), ["src"]);
    }

    #[test]
    fn supports_literal_hyphen_paths_after_double_dash() {
        let cli = parse(&["check", "--", "-stdin"]).unwrap();
        assert_eq!(cli.paths(), ["-stdin"]);
    }

    #[test]
    fn rejects_quiet_and_verbose_together() {
        let err = parse(&["check", "--quiet", "--verbose"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn rejects_unexpected_init_args() {
        let err = parse(&["init", "extra"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let err = parse_cli_args(normalize_args(vec![
            OsString::from("check"),
            OsString::from_vec(vec![0x66, 0x6f, 0x80]),
        ]))
        .unwrap_err();

        assert_eq!(err.kind(), ErrorKind::InvalidUtf8);
    }
}
