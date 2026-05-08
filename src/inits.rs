use std::{fs, path::Path};

const DEFAULT_EDITORCONFIG: &str = include_str!("../templates/default.editorconfig");
const DEFAULT_GIT_BLAME_IGNORE_REVS: &str =
    include_str!("../templates/default.git-blame-ignore-revs");

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

pub fn init_editorconfig() -> bool {
    let path = Path::new(".editorconfig");
    if path.exists() {
        eprintln!("error: .editorconfig already exists");
        return false;
    }
    if let Err(e) = fs::write(path, DEFAULT_EDITORCONFIG) {
        eprintln!("error: failed to write .editorconfig: {e}");
        return false;
    }
    println!("Created .editorconfig");
    true
}
