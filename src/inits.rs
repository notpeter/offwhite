use std::{fs, path::Path};

use crate::templates::{find_template, template_names};

const DEFAULT_GIT_BLAME_IGNORE_REVS: &str = "\
# .git-blame-ignore-revs
#
# This file consists of a list of commits that should be ignored for
# `git blame` purposes. This is useful for ignoring commits that only
# changed whitespace / indentation / formatting, but did not change
# the underlying syntax tree.
#
# GitHub will pick this up automatically for blame views:
#   https://docs.github.com/en/repositories/working-with-files/using-files/viewing-a-file#ignore-commits-in-the-blame-view
#
# To use this file locally by default for `git blame` in this repo:
#   git config --local blame.ignoreRevsFile .git-blame-ignore-revs
# To undo/disable:
#   git config --local blame.ignoreRevsFile \"\"
# To use this file once:
#   git blame --ignore-revs-file .git-blame-ignore-revs
#
# Comments are optional, but may provide helpful context.
";

pub fn init_ignore_revs() -> bool {
    let path = Path::new(".git-blame-ignore-revs");
    if path.exists() {
        eprintln!("error: .git-blame-ignore-revs already exists");
        return false;
    }
    if let Err(e) = fs::write(path, DEFAULT_GIT_BLAME_IGNORE_REVS) {
        eprintln!("error: failed to write .git-blame-ignore-revs: {e}");
        return false;
    }
    println!("Created .git-blame-ignore-revs");
    true
}

pub fn init_editorconfig(template_name: &str) -> bool {
    let Some(template) = find_template(template_name) else {
        eprintln!(
            "error: unknown editorconfig template `{template_name}`; available templates: {}",
            template_names().collect::<Vec<_>>().join(", ")
        );
        return false;
    };

    let path = Path::new(".editorconfig");
    if path.exists() {
        eprintln!("error: .editorconfig already exists");
        return false;
    }
    if let Err(e) = fs::write(path, template.contents) {
        eprintln!("error: failed to write .editorconfig: {e}");
        return false;
    }
    println!("Created .editorconfig from {template_name} template");
    true
}
