# offwhite

Check and fix trailing whitespace and final newlines, driven by `.editorconfig`.

## Usage

```
offwhite [OPTIONS] [PATHS]...
```

Paths default to `.` and support glob patterns (`*`, `**/*.rs`).
Directories are walked recursively, respecting `.gitignore` (including nested) by default.

## Options

| Flag             | Description                |
| ---------------- | -------------------------- |
| `--fix`          | Rewrite files in place     |
| `--no-gitignore` | Don't respect `.gitignore` |

## .editorconfig

offwhite reads `.editorconfig` files to decide what to check per file.
Files with neither property set are skipped entirely.

| Property                          | Effect                                       |
| --------------------------------- | -------------------------------------------- |
| `trim_trailing_whitespace = true` | Check/fix trailing whitespace on lines       |
| `insert_final_newline = true`     | Check/fix missing or extra trailing newlines |

Example `.editorconfig`:

```editorconfig
root = true

[*]
trim_trailing_whitespace = true
insert_final_newline = true
```

## Default ignores

Always skipped: `.git/`, `*.patch`, `*.diff`, `*.rej`, `*.patchset`.

Exits `0` if clean, `1` if violations found.

## Initialization

`offwhite` needs an `.editconfig` to operate.
Use `offwhite --init` to create one in the current directory with:

```editorconfig
root = true
[*]
end_of_line = lf
trim_trailing_whitespace = true
insert_final_newline = true
```

## Dependencies

This project depends on the following crates:

| Crate                                     | Repo                                                                                  | Purpose             |
| ----------------------------------------- | ------------------------------------------------------------------------------------- | ------------------- |
| [ec4rs](https://crates.io/crates/ec4rs)   | [TheDaemoness/ec4rs](https://github.com/TheDaemoness/ec4rs)                           | Editorconfig parser |
| [grep](https://crates.io/crates/grep)     | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep/tree/master/crates/grep)   | Grep Engine         |
| [ignore](https://crates.io/crates/ignore) | [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore) | Gitignore parser    |
