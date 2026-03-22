# offwhite

Check and fix trailing whitespace and final newlines, driven by `.editorconfig`.

> [!NOTE]
> Work in progress

## Installation

Install from GitHub with Cargo:

```sh
cargo install --git https://github.com/notpeter/offwhite.git offwhite
```

## Usage

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

`check` is the default action when none is provided.

If not `PATHS` are provided, default to the current directory: `.`

Directories are walked recursively, respecting `.editorconfig`, `.ignore` and `.gitignore`.

## .editorconfig

offwhite reads `.editorconfig` files to determine what to check/fix and which rules to enforce.
Multiple `.editorconfig` files in nested directories are supported.

Offwhite enforces compliance with the follow `.editorconfig` keys (when set):

| Property                          | Effect                                       |
| --------------------------------- | -------------------------------------------- |
| `charset = utf-8`                 | Required to enable scanning through files    |
| `end_of_line = lf`                | Check/fix line endings (`lf` or `crlf`)      |
| `trim_trailing_whitespace = true` | Check/fix trailing whitespace on lines       |
| `insert_final_newline = true`     | Check/fix missing or extra trailing newlines |

## Initialization

To operate, `offwhite` requires a root `.editconfig`:

```editorconfig
root = true
[*]
charset = utf-8
```

Use `offwhite init` to create an example `.editorconfig` in the current directory:

```editorconfig
root = true
[*]
charset = utf-8
end_of_line = lf
trim_trailing_whitespace = true
insert_final_newline = true
```

`offwhite` requires each target path to resolve to a root `.editorconfig`.
If the root `.editorconfig` does not declare `charset = utf-8` in any section, `offwhite` warns.
Files are only scanned when matching `.editorconfig` sections declare `charset = utf-8`.
If no discovered files match scanable `.editorconfig` sections, `offwhite` warns.
When you pass explicit paths, `.editorconfig` lookup starts from each target path and walks upward from there, not from the shell's current working directory.
Sections with alternative charset policies are skipped; in verbose mode, `offwhite` warns when this happens.
Files that are not valid UTF-8 are skipped with a warning.

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
   files used by `ripgrep` to ignore specific files. These are structured like a `.gitignore` file
   but will cause `offwhite` (and ripgrep) to not scan certain files or directories.

   As with `.gitignore` files, `.ignore` files may be placed at the root of your project
   or within specific subdirectories. To ignore an specific directory you can create a
   `.ignore` file with `*` as its contents:

   ```sh
   echo '*' > whatever/directory/.ignore
   ```

   Or in a top-level .ignore file:

   ```sh
   whatever/directory/
   ```

   This is appropriate for binary files you'd like text-processing tools like `ripgrep`/`offwhite` to skip.

2. Specify the correct values `.editorconfig`:

   If files in your repository have different whitespace / linebreak needs,
   you can edit your `.editorconfig` specify correct settings
   (e.g. CRLF for certain files) for certain paths.

   ```editorconfig
   root = true
   [*]
   charset = utf-8
   end_of_line = lf
   trim_trailing_whitespace = true
   insert_final_newline = true

   [/vendor/windos_project/*.c]
   charset = latin1
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

   To prevent a `.editorconfig` in parent directory from applying, create a nested
   `.editorconfig` which just contains `root = true`. This directory will be skipped.

   ```sh
   echo 'root = true' > vendor/.editorconfig
   ```

3. Don't apply rules with a global `[*]` in your .editconfig

   If `offwhite` is triggering false positives on many files and you have a default `[*]` editconfig
   section, you might want to consider removing that and replace it with path specific and/or
   extension specific rules like `[/src/**/*.rs]` or `[*.{rs,py,ts,md,asciidoc}]`.

### No --include or --exclude

Q: Why don't you include a runtime option to `--exclude` or `--include` globs?
A: The goal of the project is to support automated enforcement of the same rules used by
editors that support the `editorconfig` standard editing files within a git repo,
with no any additional configuration.
If you don't wish to process directories (a) edit your `.editorconfig` (b) add `.ignore`
files or (c) pass explicit files or directories at runtime.

### Glob notes

> [!NOTE]
>
> - Globs like `offwhite **/*.md` will be expanded by your shell, passing a list of files to `offwhite` as arguments.
>   There is a size limit to these arguments (`getconf ARG_MAX`), typically 1 or 2 MB, but sometimes smaller.
> - Globs in [src/ignores.rs](src/ignores.rs) are case insensitive.
> - [`.editorconfig` wildcards](https://editorconfig.org/#wildcards) are case-sensitive globs
>   and support complex globbing syntax like `[**/tests/{output/*,*.out.txt}]`, `[*.[Pp][Yy]]`
> - To match files non-recursively, use a leading slash in the `.editorconfig` section like `[/*.md]`

## Git Blame Ignore Revs

When making whitespace-only corrections to existing repositories, afterwards you can optionally
create a `.git-blame-ignore-revs` file which contain git commit ids which should be considered
transparent for git blame purposes. This means if you convert a tree from CRLF it is possible
to preserve meaningful Git Blame information.

Running `offwhite init-ignore-revs` will create you an template `.git-blame-ignore-revs` file
which contains instructions on how to configure git repo locally to use that file. GitHub and
GitLab natively support this out of the box.

## Limitations

### Character Encoding Limtations

Only UTF-8 files are processed. Files with invalid UTF-8 is skipped.
Other character encodings are unsupported. (latin1 / ISO 8859-1, Windows-1252, utf-8, utf-8-bom, utf-16be, utf-16le, etc)

Unix filenames may be arbitrary bytes, but only UTF-8 filenames are supported.

### Editorconfig Limitations

Offwhite supports enforcing the following EditorConfig directives on UTF-8 files:

- `insert_final_newline = true`
- `trim_trailing_whitespace = true`
- `end_of_line = lf`
- `end_of_line = crlf`

It ignores and does not check [other EditorConfig Properties](https://github.com/editorconfig/editorconfig/wiki/EditorConfig-Properties) like:

```editorconfig
charset = latin1
indent_style = tab
indent_size = 4
tab_width = 4
max_line_length = 100
```

This is intentional.

## Dependencies

This project depends on the following crates:

| Crate                                     | Repo                                                                                  | Purpose             |
| ----------------------------------------- | ------------------------------------------------------------------------------------- | ------------------- |
| [ec4rs](https://crates.io/crates/ec4rs)   | [TheDaemoness/ec4rs](https://github.com/TheDaemoness/ec4rs)                           | Editorconfig parser |
| [ignore](https://crates.io/crates/ignore) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) | Gitignore parser    |

## License

Copyright (c) Peter Tripp
Available under the [MIT License](LICENSE).
