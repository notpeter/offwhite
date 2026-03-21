use std::path::Path;

use crate::configs::LineEnding;

#[derive(Clone)]
pub struct Violation<'a> {
    pub path: &'a Path,
    pub line: u64,
    pub kind: ViolationKind,
}

#[derive(Clone)]
pub enum ViolationKind {
    TrailingWhitespace,
    NoFinalNewline,
    ExtraFinalNewlines,
    IncorrectLineEnding {
        expected: LineEnding,
        found: LineEnding,
    },
}

impl std::fmt::Display for Violation<'_> {
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
            ViolationKind::IncorrectLineEnding { expected, found } => {
                write!(
                    f,
                    "{path}:{line}: incorrect line ending: expected {}, found {}",
                    expected.as_str(),
                    found.as_str()
                )
            }
        }
    }
}
