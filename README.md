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
      --no-ignore             Do not respect .ignore or .gitignore files
  -h, --help                  Print help
```

## .editorconfig

offwhite reads `.editorconfig` files to decide which files to check and the rules to enforce.
Multiple `.editorconfig` files in nested directories are supported.

Offwhite only enforces the following properties:

| Property                          | Effect                                       |
| --------------------------------- | -------------------------------------------- |
| `end_of_line = lf|crlf|cr`        | Check/fix line endings                       |
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

## Ignore / Exclusions

Like ripgrep, by default `offwhite` will not process paths excluded by `.ignore`, `.gitignore`, `.git/info/exclude`, or global Git Ignore files (use `--no-ignore` to bypass).
Unlike `ripgrep`, directories with a leading `.` are not skipped to ensure things like `.github/**` are checked.

Additionally, `offwhite` bundles an ignore list which by-default excludes:
- Source control directories: `.git/`, `.svn/`, `.hg/`
- License files: `license*`, `COPYING`, etc
- Patch files: `*.patch`, `*.diff`, `*.rej`, `*.patchset`.
- extensions likely to contain binary content (`*.png`, `*.exe`, `*.jpg`, etc).
- See: [src/ignores.rs](src/ignores.rs) for a full list

### Skipping files

To exclude/ignore certain files/directories you have a few options:

1. Add path(s) to a `.ignore` file:

    Offwhite supports the same [`.ignore`](https://github.com/BurntSushi/ripgrep/blob/master/GUIDE.md#automatic-filtering)
    files used by ripgrep to ignore specific files.  These are structured like a `.gitignore` file
    but will cause `offwhite` and ripgrep to not scan certain files or directories.

    As with `.gitignore` files, `.ignore` files may be placed at the root of your project
    or within specific subdirectories.  To ignore an entire directory just create a
    `.ignore` file with `.` as its contents:

    ```sh
    echo . > whatever/directory/.ignore
    ```

    This is appropriate for binary files or other files you would never want ripgrep
    or other text-processing tools to attempt to scan.

2. Specify the correct values `.editorconfig`:

    If you have files in your repository have different whitespace / linebreak needs,
    you can specify the correct settings (e.g. CRLF for certain files) or you can
    direct `.editorconfig` to apply a more specific configuration for certain paths.

    ```editorconfig
    root = true
    [*]
    end_of_line = lf
    trim_trailing_whitespace = true
    insert_final_newline = true

    [/vendor/windos_project/*.c]
    end_of_line = crlf
    trim_trailing_whitespace = unset
    insert_final_newline = unset

    [**/test_output/**]
    trim_trailing_whitespace = false
    insert_final_newline = false
    ```

    Just as with `.gitignore` / `.ignore` you can also put these in a subdirectory.
    If you `.editconfig` does not contain `root = true` it will inherit any settings
    from `.editorconfig` in parent directories. If a subdirectory `.editorconfig`
    includes `root = true` then any parent `.editorconfig` files will be ignored.

    To make `.editorconfig` not apply to a given `root = true` is all you need:

    ```sh
    echo 'root = true' > vendor/.editorconfig
    ```

### No --include or --exclude

Or add the files/directories to a `.ignore` file.

Q: Why don't you include a runtime option to `--exclude` or `--include` globs?
A: The goal of the project is to support automated enforcement of the same rules used by
editors that support the `editorconfig` standard editing files within a git repo,
with no any additional configuration.
If you don't wish to process directories (a) edit your `.editorconfig` (b) add `.ignore`
files or (c) specify paths/globs `offwhite 'src/*.{toml,md,rs}` at runtime.
### Glob notes

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
