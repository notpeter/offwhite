# offwhite

Check and fix trailing whitespace and final newlines, driven by `.editorconfig`.

> [!NOTE]
> Work in progress

## Usage

```
offwhite check [OPTIONS] [PATHS]...
offwhite fix [OPTIONS] [PATHS]...
offwhite init
offwhite init-ignore-revs
```

Paths default to `.` and support glob patterns (`*`, `**/*.rs`).
Directories are walked recursively, respecting `.ignore` and `.gitignore` (including nested).
`check` is the default action when none is provided.

## Help

```
Offwhite enforces .editconfig whitespace and newline settings

Usage:
  offwhite check [OPTIONS] [PATHS]...       Check for violations
  offwhite fix [OPTIONS] [PATHS]...         Fix files in place
  offwhite init                             Create an example .editorconfig
  offwhite init-ignore-revs                 Create an example .git-blame-ignore-revs

Options:
  -q, --quiet                 Suppress warnings
  -v, --verbose               Increase logging output
      --single-final-newline  Enforce exactly one trailing newline (disabled by default)
      --no-ignore             Do not respect .ignore or git ignore files
  -h, --help                  Print help
```

## .editorconfig

offwhite reads `.editorconfig` files to decide which files to check and the rules to enforce.
Multiple `.editorconfig` files in nested directories are supported.

Offwhite only enforces the following properties:

| Property                          | Effect                                       |
| --------------------------------- | -------------------------------------------- |
| `trim_trailing_whitespace = true` | Check/fix trailing whitespace on lines       |
| `insert_final_newline = true`     | Check/fix missing or extra trailing newlines |

## Initialization

`offwhite` needs an `.editconfig` to operate.
Use `offwhite init` to create one in the current directory:

```editorconfig
root = true
[*]
end_of_line = lf
trim_trailing_whitespace = true
insert_final_newline = true
```

## Ignored / Exclusions

Like ripgrep, by default `offwhite` will not process paths excluded by `.ignore`, `.gitignore`, `.git/info/exclude`, or global Git ignore files (use `--no-ignore` to bypass).
Unlike `ripgrep`, directories with a leading `.` are not skipped to ensure things like `.github/**` are checked.

Additionally, `offwhite` bundles an ignore list which by-default excludes:
- Source control directories: `.git/`, `.svn/`, `.hg/`
- License files: `license*`, `COPYING`, etc
- Patch files: `*.patch`, `*.diff`, `*.rej`, `*.patchset`.
- extensions likely to contain binary content (`*.png`, `*.exe`, `*.jpg`, etc).
- See: [src/ignores.rs](src/ignores.rs) for a full list

To exclude certain files/directories add them to your `.editorconfig`:

```editorconfig
[*]
trim_trailing_whitespace = true
insert_final_newline = true

[**/tests/output/**]
trim_trailing_whitespace = none
insert_final_newline = none
```

Or add the files/directories to a `.ignore` file.

Q: Why don't you include a runtime option to `--exclude` or `--include` globs?
A: The goal of the project is to support automated enforcement of the same rules used by
editors that support the `editorconfig` standard editing files within a git repo,
with no any additional configuration.
If you don't wish to process directories (a) edit your `.editorconfig` (b) add `.ignore`
files or (c) specify paths/globs `offwhite 'src/*.{toml,md,rs}` at runtime.

> [!NOTE]
> - Unquoted globs: `offwhite *.md` will be expanded by your shell and passed a list of files to `offwhite` as arguments.
> There is a size limit to these arguments (`getconf ARG_MAX`), typically 1 or 2 MB, but sometimes smaller.
> - Quoted globs: `offwhite '*.md'` will be handled by `offwhite` and expanded as `.editorconfig` globs.
> Editconfig matches `*.md` recursively, so in practice `*.md` is like `**/*.md` in the shell.
> To match files non-recursively, use `[/*.md]` in your `.editorconfig`.
> - Globs in [src/ignores.rs](src/ignores.rs) are case insensitive.
> - All other non-shell globs ([`.editorconfig` wildcards](https://editorconfig.org/#wildcards), etc)
>  are case-sensitive and support complex globbing syntax like
> `[**/tests/{output/*,*.out.txt}]`,  `[*.[Pp][Yy]]`

## Dependencies

This project depends on the following crates:

| Crate                                     | Repo                                                                                  | Purpose             |
| ----------------------------------------- | ------------------------------------------------------------------------------------- | ------------------- |
| [ec4rs](https://crates.io/crates/ec4rs)   | [TheDaemoness/ec4rs](https://github.com/TheDaemoness/ec4rs)                           | Editorconfig parser |
| [grep](https://crates.io/crates/grep)     | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep/tree/master/crates/grep)   | Grep Engine         |
| [ignore](https://crates.io/crates/ignore) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) | Gitignore parser    |

## License

Copyright (c) Peter Tripp
Available under the [MIT License](LICENSE).
