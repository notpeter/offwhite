use std::path::PathBuf;

pub struct Violation {
    pub path: PathBuf,
    pub line: u64,
    pub kind: ViolationKind,
}

pub enum ViolationKind {
    TrailingWhitespace,
    NoFinalNewline,
    ExtraFinalNewlines,
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path = self.path.display();
        let line = self.line;
        match &self.kind {
            ViolationKind::TrailingWhitespace => {
                write!(f, "{path}:{line}: trailing whitespace")
            }
            ViolationKind::NoFinalNewline => {
                write!(f, "{path}:{line}: no final newline")
            }
            ViolationKind::ExtraFinalNewlines => {
                write!(f, "{path}:{line}: multiple trailing newlines")
            }
        }
    }
}
