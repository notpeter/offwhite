use std::fs;
use std::path::Path;
use tempfile::TempDir;

use crate::{
    action::{check_file_with, walk_paths},
    configs::{FilePolicy, LineEnding, PolicyCache},
    fix_file,
    violation::ViolationKind,
};

const ALL_CHECKS: FilePolicy = FilePolicy {
    trim_trailing_whitespace: true,
    insert_final_newline: true,
    single_final_newline: false,
    end_of_line: None,
};

const ALL_CHECKS_SINGLE: FilePolicy = FilePolicy {
    trim_trailing_whitespace: true,
    insert_final_newline: true,
    single_final_newline: true,
    end_of_line: None,
};

const TRIM_ONLY: FilePolicy = FilePolicy {
    trim_trailing_whitespace: true,
    insert_final_newline: false,
    single_final_newline: false,
    end_of_line: None,
};

const NEWLINE_ONLY: FilePolicy = FilePolicy {
    trim_trailing_whitespace: false,
    insert_final_newline: true,
    single_final_newline: false,
    end_of_line: None,
};

const NEWLINE_ONLY_SINGLE: FilePolicy = FilePolicy {
    trim_trailing_whitespace: false,
    insert_final_newline: true,
    single_final_newline: true,
    end_of_line: None,
};

const NO_CHECKS: FilePolicy = FilePolicy {
    trim_trailing_whitespace: false,
    insert_final_newline: false,
    single_final_newline: false,
    end_of_line: None,
};

const CRLF_ONLY: FilePolicy = FilePolicy {
    trim_trailing_whitespace: false,
    insert_final_newline: false,
    single_final_newline: false,
    end_of_line: Some(LineEnding::CrLf),
};

fn write_temp(dir: &Path, name: &str, contents: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn read_temp(path: &Path) -> String {
    fs::read_to_string(path).unwrap()
}

fn collect_walked_paths(paths: &[String], respect_ignore_files: bool) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    walk_paths(paths, respect_ignore_files, |path| files.push(path));
    files
}

fn check_file(
    path: &Path,
    policy: FilePolicy,
) -> Result<Vec<crate::violation::Violation>, Box<dyn std::error::Error>> {
    let mut violations = Vec::new();
    check_file_with(path, policy, |violation| violations.push(violation))?;
    Ok(violations)
}

#[test]
fn walk_paths_includes_files_from_directory_argument() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    let subdir = dir.path().join("src");
    fs::create_dir(&subdir).unwrap();
    let file = write_temp(&subdir, "main.rs", "fn main() {}\n");

    let files = collect_walked_paths(&[subdir.display().to_string()], true);

    assert_eq!(files, vec![file]);
}

#[test]
fn walk_paths_directory_argument_still_applies_default_ignores() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    let subdir = dir.path().join("assets");
    fs::create_dir(&subdir).unwrap();
    let png = write_temp(&subdir, "image.png", "not really a png");

    let files = collect_walked_paths(&[subdir.display().to_string()], true);

    assert!(!files.contains(&png));
}

#[test]
fn walk_paths_directory_argument_respects_ignore_files() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), ".ignore", "target/\n");
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();
    let ignored = write_temp(&target, "generated.txt", "ignored");
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    let kept = write_temp(&src, "main.rs", "fn main() {}\n");

    let files = collect_walked_paths(&[dir.path().display().to_string()], true);

    assert!(files.contains(&kept));
    assert!(!files.contains(&ignored));
}
// --- check_file tests ---

#[test]
fn check_clean_file() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "clean.rs", "fn main() {}\n");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn check_trailing_whitespace_multiple_lines() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "ws.rs", "a \nb\nc\t\nd \n");
    let violations: Vec<_> = check_file(&path, ALL_CHECKS)
        .unwrap()
        .into_iter()
        .filter(|v| matches!(v.kind, ViolationKind::TrailingWhitespace))
        .collect();
    assert_eq!(violations.len(), 3);
    assert_eq!(violations[0].line, 1);
    assert_eq!(violations[1].line, 3);
    assert_eq!(violations[2].line, 4);
}

#[test]
fn check_file_with_streams_violations_in_order() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "stream.rs", "a \nb");
    let mut seen = Vec::new();

    check_file_with(&path, ALL_CHECKS, |violation| {
        seen.push((violation.line, violation.kind));
    })
    .unwrap();

    assert_eq!(seen.len(), 2);
    assert!(matches!(seen[0], (1, ViolationKind::TrailingWhitespace)));
    assert!(matches!(seen[1], (2, ViolationKind::NoFinalNewline)));
}

#[test]
fn check_trailing_tabs() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "tabs.rs", "hello\t\n");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::TrailingWhitespace
    ));
    assert_eq!(violations[0].line, 1);
}

#[test]
fn check_no_final_newline() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "nonl.rs", "hello");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(violations[0].kind, ViolationKind::NoFinalNewline));
    assert_eq!(violations[0].line, 1);
}

#[test]
fn check_no_final_newline_multiline() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "nonl.rs", "a\nb\nc");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(violations[0].kind, ViolationKind::NoFinalNewline));
    assert_eq!(violations[0].line, 3);
}

#[test]
fn check_extra_final_newlines_allowed_by_default() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "extra.rs", "hello\n\n\n");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn check_extra_final_newlines_with_single() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "extra.rs", "hello\n\n\n");
    let violations = check_file(&path, ALL_CHECKS_SINGLE).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::ExtraFinalNewlines
    ));
    assert_eq!(violations[0].line, 3);
}

#[test]
fn check_empty_file() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "empty.rs", "");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn check_multiple_violations() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "multi.rs", "hello   ");
    let violations = check_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(violations.len(), 2);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::TrailingWhitespace
    ));
    assert_eq!(violations[0].line, 1);
    assert!(matches!(violations[1].kind, ViolationKind::NoFinalNewline));
    assert_eq!(violations[1].line, 1);
}

// --- policy-selective check tests ---

#[test]
fn check_trim_only_ignores_newline_issues() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   ");
    let violations = check_file(&path, TRIM_ONLY).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::TrailingWhitespace
    ));
}

#[test]
fn check_newline_only_ignores_whitespace() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   ");
    let violations = check_file(&path, NEWLINE_ONLY).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(violations[0].kind, ViolationKind::NoFinalNewline));
}

#[test]
fn check_end_of_line_reports_mismatches() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\nworld\r\n");
    let violations = check_file(&path, CRLF_ONLY).unwrap();
    assert_eq!(violations.len(), 1);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::IncorrectLineEnding {
            expected: LineEnding::CrLf,
            found: LineEnding::Lf
        }
    ));
    assert_eq!(violations[0].line, 1);
}

#[test]
fn check_end_of_line_reports_all_mismatches() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\nworld\n");
    let violations = check_file(&path, CRLF_ONLY).unwrap();

    assert_eq!(violations.len(), 2);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::IncorrectLineEnding { .. }
    ));
    assert!(matches!(
        violations[1].kind,
        ViolationKind::IncorrectLineEnding { .. }
    ));
    assert_eq!(violations[0].line, 1);
    assert_eq!(violations[1].line, 2);
}

#[test]
fn check_end_of_line_stream_reports_all_mismatches() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\nworld\n");
    let mut seen = Vec::new();

    check_file_with(&path, CRLF_ONLY, |violation| seen.push(violation)).unwrap();

    assert_eq!(seen.len(), 2);
    assert!(matches!(
        seen[0].kind,
        ViolationKind::IncorrectLineEnding { .. }
    ));
    assert!(matches!(
        seen[1].kind,
        ViolationKind::IncorrectLineEnding { .. }
    ));
}

#[test]
fn check_reports_mixed_violation_kinds_in_order() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello \nworld");
    let policy = FilePolicy {
        trim_trailing_whitespace: true,
        insert_final_newline: true,
        single_final_newline: false,
        end_of_line: Some(LineEnding::CrLf),
    };

    let violations = check_file(&path, policy).unwrap();

    assert_eq!(violations.len(), 3);
    assert!(matches!(
        violations[0].kind,
        ViolationKind::TrailingWhitespace
    ));
    assert!(matches!(
        violations[1].kind,
        ViolationKind::IncorrectLineEnding { .. }
    ));
    assert!(matches!(violations[2].kind, ViolationKind::NoFinalNewline));
}

// --- fix_file tests ---

#[test]
fn fix_trailing_whitespace() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "ws.rs", "hello   \nworld\t\n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");
}

#[test]
fn fix_no_final_newline() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "nonl.rs", "hello");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
}

#[test]
fn fix_extra_final_newlines_preserved_by_default() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "extra.rs", "hello\n\n\n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n\n\n");
}

#[test]
fn fix_extra_final_newlines_with_single() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "extra.rs", "hello\n\n\n");
    fix_file(&path, ALL_CHECKS_SINGLE).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n");
}

#[test]
fn fix_empty_file_stays_empty() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "empty.rs", "");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn fix_all_issues_combined() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "combo.rs", "  a  \n  b\t\n\n\n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "  a\n  b\n\n\n");
}

#[test]
fn fix_all_issues_combined_with_single() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "combo.rs", "  a  \n  b\t\n\n\n");
    fix_file(&path, ALL_CHECKS_SINGLE).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "  a\n  b\n");
}

#[test]
fn fix_clean_file_unchanged() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "clean.rs", "hello\nworld\n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");
}

#[test]
fn fix_clean_readonly_file_skips_write() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "clean.rs", "hello\nworld\n");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions.clone()).unwrap();

    let result = fix_file(&path, ALL_CHECKS);

    permissions.set_readonly(false);
    fs::set_permissions(&path, permissions).unwrap();

    assert!(result.is_ok());
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\nworld\n");
}

// --- policy-selective fix tests ---

#[test]
fn fix_trim_only_preserves_missing_newline() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   ");
    fix_file(&path, TRIM_ONLY).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello");
}

#[test]
fn fix_trim_only_preserves_extra_newlines() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello  \n\n\n");
    fix_file(&path, TRIM_ONLY).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello\n\n\n");
}

#[test]
fn fix_newline_only_preserves_trailing_whitespace() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   \n\n\n");
    fix_file(&path, NEWLINE_ONLY).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello   \n\n\n");
}

#[test]
fn fix_newline_only_single_strips_extra() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   \n\n\n");
    fix_file(&path, NEWLINE_ONLY_SINGLE).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "hello   \n");
}

#[test]
fn fix_end_of_line_converts_existing_newlines() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\nworld\n");
    fix_file(&path, CRLF_ONLY).unwrap();
    assert_eq!(read_temp(&path), "hello\r\nworld\r\n");
}

#[test]
fn fix_end_of_line_uses_configured_newline_for_inserted_final_newline() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello");
    let policy = FilePolicy {
        trim_trailing_whitespace: false,
        insert_final_newline: true,
        single_final_newline: false,
        end_of_line: Some(LineEnding::CrLf),
    };
    fix_file(&path, policy).unwrap();
    assert_eq!(read_temp(&path), "hello\r\n");
}

#[test]
fn fix_without_end_of_line_preserves_existing_crlf() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello  \r\nworld\t\r\n");
    fix_file(&path, TRIM_ONLY).unwrap();
    assert_eq!(read_temp(&path), "hello\r\nworld\r\n");
}

// --- editorconfig integration test ---

#[test]
fn file_policy_reads_editorconfig() {
    let dir = TempDir::new().unwrap();
    let ec = "root = true\n\n[*]\ntrim_trailing_whitespace = true\ninsert_final_newline = true\nend_of_line = crlf\n";
    write_temp(dir.path(), ".editorconfig", ec);
    let path = write_temp(dir.path(), "test.rs", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);
    assert!(policy.trim_trailing_whitespace);
    assert!(policy.insert_final_newline);
    assert_eq!(policy.end_of_line, Some(LineEnding::CrLf));
}

#[test]
fn file_policy_respects_glob_sections() {
    let dir = TempDir::new().unwrap();
    let ec = "root = true\n\n[*.rs]\ntrim_trailing_whitespace = true\n\n[*.md]\ninsert_final_newline = true\n";
    write_temp(dir.path(), ".editorconfig", ec);

    let rs_path = write_temp(dir.path(), "test.rs", "");
    let md_path = write_temp(dir.path(), "test.md", "");
    let txt_path = write_temp(dir.path(), "test.txt", "");

    let mut cache = PolicyCache::new();

    let rs_policy = cache.file_policy(&rs_path);
    assert!(rs_policy.trim_trailing_whitespace);
    assert!(!rs_policy.insert_final_newline);

    let md_policy = cache.file_policy(&md_path);
    assert!(!md_policy.trim_trailing_whitespace);
    assert!(md_policy.insert_final_newline);

    let txt_policy = cache.file_policy(&txt_path);
    assert!(!txt_policy.trim_trailing_whitespace);
    assert!(!txt_policy.insert_final_newline);
    assert_eq!(txt_policy.end_of_line, None);
}

#[test]
fn file_policy_defaults_to_off() {
    let dir = TempDir::new().unwrap();
    // .editorconfig with root=true but no matching properties
    let ec = "root = true\n";
    write_temp(dir.path(), ".editorconfig", ec);
    let path = write_temp(dir.path(), "test.rs", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);
    assert!(!policy.trim_trailing_whitespace);
    assert!(!policy.insert_final_newline);
    assert_eq!(policy.end_of_line, None);
}

// --- no-op when both policies are off ---

#[test]
fn check_no_policy_returns_nothing() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   \n\n\n");
    let violations = check_file(&path, NO_CHECKS).unwrap();
    assert!(violations.is_empty());
}

// --- edge cases ---

#[test]
fn fix_whitespace_only_file() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "ws.rs", "   \n   \n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "\n\n");
}

#[test]
fn fix_whitespace_only_file_with_single() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "ws.rs", "   \n   \n");
    fix_file(&path, ALL_CHECKS_SINGLE).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn fix_single_newline_file() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "nl.rs", "\n");
    fix_file(&path, ALL_CHECKS).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
}

#[test]
fn fix_trim_only_then_check_passes() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello   \nworld\t\n");
    fix_file(&path, TRIM_ONLY).unwrap();
    let violations = check_file(&path, TRIM_ONLY).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn fix_newline_only_then_check_passes() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\n\n\n");
    fix_file(&path, NEWLINE_ONLY).unwrap();
    let violations = check_file(&path, NEWLINE_ONLY).unwrap();
    assert!(violations.is_empty());
}

#[test]
fn fix_newline_only_single_then_check_passes() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), "f.rs", "hello\n\n\n");
    fix_file(&path, NEWLINE_ONLY_SINGLE).unwrap();
    let violations = check_file(&path, NEWLINE_ONLY_SINGLE).unwrap();
    assert!(violations.is_empty());
}
