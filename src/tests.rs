use std::fs;
use std::path::Path;
use tempfile::TempDir;

use crate::{
    action::{FileStatus, check_file_with, fix_file, walk_paths},
    args::Verbosity,
    collect_scan_paths,
    configs::{FilePolicy, LineEnding, PolicyCache, RootConfig},
    list::{
        discover_editorconfigs, render_extension_summary, summarize_extensions,
        validate_editorconfig,
    },
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
    walk_paths(paths, respect_ignore_files, |path| {
        files.push(path);
        Ok(())
    })
    .unwrap();
    files
}

fn check_file(
    path: &Path,
    policy: FilePolicy,
) -> std::io::Result<Vec<crate::violation::Violation<'_>>> {
    let mut violations = Vec::new();
    let status = check_file_with(path, policy, |violation| {
        violations.push(violation);
        Ok(())
    })?;
    assert_eq!(status, FileStatus::Processed);
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
fn walk_paths_directory_argument_still_applies_default_lockfile_ignores() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    let subdir = dir.path().join("project");
    fs::create_dir(&subdir).unwrap();
    let cargo_lock = write_temp(&subdir, "Cargo.lock", "");
    let package_lock = write_temp(&subdir, "package.lock.json", "");
    let go_sum = write_temp(&subdir, "go.sum", "");
    let package_resolved = write_temp(&subdir, "Package.resolved", "");
    let pnpm_lock = write_temp(&subdir, "pnpm.lock.yaml", "");
    let yarn_lock = write_temp(&subdir, "yarn.lock.yml", "");
    let source = write_temp(&subdir, "main.rs", "fn main() {}\n");

    let files = collect_walked_paths(&[subdir.display().to_string()], true);

    assert!(files.contains(&source));
    assert!(!files.contains(&cargo_lock));
    assert!(!files.contains(&package_lock));
    assert!(!files.contains(&go_sum));
    assert!(!files.contains(&package_resolved));
    assert!(!files.contains(&pnpm_lock));
    assert!(!files.contains(&yarn_lock));
}

#[test]
fn walk_paths_directory_argument_still_applies_default_license_ignores() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    let subdir = dir.path().join("project");
    fs::create_dir(&subdir).unwrap();
    let license = write_temp(&subdir, "LICENSE", "");
    let licence_md = write_temp(&subdir, "licence.md", "");
    let copying = write_temp(&subdir, "COPYING", "");
    let notice = write_temp(&subdir, "NOTICE", "");
    let source = write_temp(&subdir, "main.rs", "fn main() {}\n");

    let files = collect_walked_paths(&[subdir.display().to_string()], true);

    assert!(files.contains(&source));
    assert!(!files.contains(&license));
    assert!(!files.contains(&licence_md));
    assert!(!files.contains(&copying));
    assert!(!files.contains(&notice));
}

#[test]
fn walk_paths_explicit_file_argument_still_applies_default_license_ignores() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    let license = write_temp(dir.path(), "unlicense.md", "");

    let files = collect_walked_paths(&[license.display().to_string()], true);

    assert!(!files.contains(&license));
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

#[test]
fn walk_paths_explicit_file_argument_respects_ignore_files() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), ".ignore", "generated.txt\n");
    let ignored = write_temp(dir.path(), "generated.txt", "ignored");

    let files = collect_walked_paths(&[ignored.display().to_string()], true);

    assert!(!files.contains(&ignored));
}

#[test]
fn discover_editorconfigs_finds_nested_files_and_respects_ignore_files() {
    let dir = TempDir::new().unwrap();
    let root = write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), ".ignore", "vendor/\n");
    let src = dir.path().join("src");
    fs::create_dir(&src).unwrap();
    let nested = write_temp(&src, ".editorconfig", "root = false\n");
    let vendor = dir.path().join("vendor");
    fs::create_dir(&vendor).unwrap();
    let ignored = write_temp(&vendor, ".editorconfig", "root = false\n");

    let (configs, found_failures) =
        discover_editorconfigs(&[dir.path().display().to_string()], true).unwrap();

    assert!(!found_failures);
    assert_eq!(configs, vec![root, nested]);
    assert!(!configs.contains(&ignored));
}

#[test]
fn validate_editorconfig_reports_line_errors() {
    let dir = TempDir::new().unwrap();
    let path = write_temp(dir.path(), ".editorconfig", "root = true\n[\n");

    let err = validate_editorconfig(&path).unwrap_err();

    assert!(err.contains(".editorconfig:"));
    assert!(err.contains("invalid line"));
}

#[test]
fn summarize_extensions_counts_all_extensions_except_editorconfig() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), ".ignore", "ignored/\n");
    write_temp(dir.path(), "main.rs", "");
    write_temp(dir.path(), "lib.RS", "");
    write_temp(dir.path(), "Cargo.toml", "");
    write_temp(dir.path(), "README", "");
    write_temp(dir.path(), "image.png", "");
    let ignored = dir.path().join("ignored");
    fs::create_dir(&ignored).unwrap();
    write_temp(&ignored, "skip.rs", "");

    let (summary, found_failures) =
        summarize_extensions(&[dir.path().display().to_string()], true).unwrap();

    assert!(!found_failures);
    assert_eq!(
        render_extension_summary(&summary, Verbosity::Normal),
        vec![
            "2\t.rs".to_string(),
            "2\t[no extension]".to_string(),
            "1\t.png".to_string(),
            "1\t.toml".to_string(),
        ]
    );
}

#[test]
fn summarize_extensions_ignores_vcs_directories() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), "main.rs", "");
    for vcs_dir in [".git", ".hg", ".svn"] {
        let path = dir.path().join(vcs_dir);
        fs::create_dir(&path).unwrap();
        write_temp(&path, "ignored.txt", "");
    }

    let (summary, found_failures) =
        summarize_extensions(&[dir.path().display().to_string()], true).unwrap();

    assert!(!found_failures);
    assert_eq!(
        render_extension_summary(&summary, Verbosity::Normal),
        vec!["1\t.rs".to_string()]
    );
}

#[test]
fn render_extension_summary_verbose_expands_no_extension_names() {
    let dir = TempDir::new().unwrap();
    write_temp(dir.path(), ".editorconfig", "root = true\n");
    write_temp(dir.path(), "README", "");
    write_temp(dir.path(), "LICENSE", "");
    write_temp(dir.path(), "README.copy", "");

    let (summary, found_failures) =
        summarize_extensions(&[dir.path().display().to_string()], true).unwrap();

    assert!(!found_failures);
    assert_eq!(
        render_extension_summary(&summary, Verbosity::Verbose),
        vec![
            "1\tLICENSE".to_string(),
            "1\tREADME".to_string(),
            "1\t.copy".to_string(),
        ]
    );
}

#[test]
fn collect_scan_paths_fails_for_missing_paths() {
    let mut policy_cache = PolicyCache::new();
    let (scan_paths, found_failures) = collect_scan_paths(
        &["does-not-exist".into()],
        &mut policy_cache,
        Verbosity::Normal,
    );

    assert!(scan_paths.is_empty());
    assert!(found_failures);
}

#[test]
fn collect_scan_paths_keeps_valid_paths_when_another_has_no_root_config() {
    let dir = TempDir::new().unwrap();

    let valid = dir.path().join("valid");
    fs::create_dir(&valid).unwrap();
    write_temp(
        &valid,
        ".editorconfig",
        "root = true\n\n[*]\ncharset = utf-8\n",
    );

    let invalid = dir.path().join("invalid");
    fs::create_dir(&invalid).unwrap();

    let paths = vec![valid.display().to_string(), invalid.display().to_string()];
    let mut policy_cache = PolicyCache::new();
    let (scan_paths, found_failures) =
        collect_scan_paths(&paths, &mut policy_cache, Verbosity::Normal);

    assert_eq!(scan_paths, vec![valid.display().to_string()]);
    assert!(found_failures);
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
        Ok(())
    })
    .unwrap();

    assert_eq!(seen.len(), 2);
    assert!(matches!(seen[0], (1, ViolationKind::TrailingWhitespace)));
    assert!(matches!(seen[1], (2, ViolationKind::NoFinalNewline)));
}

#[test]
fn check_file_with_invalid_utf8_returns_warning_status() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("invalid.txt");
    fs::write(&path, [0x66, 0x6f, 0x80, 0x0a]).unwrap();
    let mut seen = Vec::new();

    let status = check_file_with(&path, ALL_CHECKS, |violation| {
        seen.push(violation);
        Ok(())
    })
    .unwrap();

    assert_eq!(status, FileStatus::InvalidUtf8);
    assert!(seen.is_empty());
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

    check_file_with(&path, CRLF_ONLY, |violation| {
        seen.push(violation);
        Ok(())
    })
    .unwrap();

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

#[test]
fn fix_invalid_utf8_returns_warning_status() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("invalid.txt");
    let bytes = [0x66, 0x6f, 0x80, 0x0a];
    fs::write(&path, bytes).unwrap();

    let status = fix_file(&path, ALL_CHECKS).unwrap();

    assert_eq!(status, FileStatus::InvalidUtf8);
    assert_eq!(fs::read(&path).unwrap(), bytes);
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
    let ec = "root = true\n\n[*]\ncharset = utf-8\ntrim_trailing_whitespace = true\ninsert_final_newline = true\nend_of_line = crlf\n";
    write_temp(dir.path(), ".editorconfig", ec);
    let path = write_temp(dir.path(), "test.rs", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);
    assert!(policy.policy.trim_trailing_whitespace);
    assert!(policy.policy.insert_final_newline);
    assert_eq!(policy.policy.end_of_line, Some(LineEnding::CrLf));
}

#[test]
fn file_policy_respects_glob_sections() {
    let dir = TempDir::new().unwrap();
    let ec = "root = true\n\n[*]\ncharset = utf-8\n\n[*.rs]\ntrim_trailing_whitespace = true\n\n[*.md]\ninsert_final_newline = true\n";
    write_temp(dir.path(), ".editorconfig", ec);

    let rs_path = write_temp(dir.path(), "test.rs", "");
    let md_path = write_temp(dir.path(), "test.md", "");
    let txt_path = write_temp(dir.path(), "test.txt", "");

    let mut cache = PolicyCache::new();

    let rs_policy = cache.file_policy(&rs_path);
    assert!(rs_policy.policy.trim_trailing_whitespace);
    assert!(!rs_policy.policy.insert_final_newline);

    let md_policy = cache.file_policy(&md_path);
    assert!(!md_policy.policy.trim_trailing_whitespace);
    assert!(md_policy.policy.insert_final_newline);

    let txt_policy = cache.file_policy(&txt_path);
    assert!(!txt_policy.policy.trim_trailing_whitespace);
    assert!(!txt_policy.policy.insert_final_newline);
    assert_eq!(txt_policy.policy.end_of_line, None);
}

#[test]
fn file_policy_defaults_to_off() {
    let dir = TempDir::new().unwrap();
    // .editorconfig with root=true and utf-8, but no matching policy properties
    let ec = "root = true\n\n[*]\ncharset = utf-8\n";
    write_temp(dir.path(), ".editorconfig", ec);
    let path = write_temp(dir.path(), "test.rs", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);
    assert!(!policy.policy.trim_trailing_whitespace);
    assert!(!policy.policy.insert_final_newline);
    assert_eq!(policy.policy.end_of_line, None);
}

#[test]
fn file_policy_skips_non_utf8_sections() {
    let dir = TempDir::new().unwrap();
    let ec = "root = true\n\n[*]\ncharset = utf-8\ntrim_trailing_whitespace = true\n\n[*.bin]\ncharset = latin1\ninsert_final_newline = true\n";
    write_temp(dir.path(), ".editorconfig", ec);
    let path = write_temp(dir.path(), "test.bin", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);

    assert!(policy.policy.trim_trailing_whitespace);
    assert!(!policy.policy.insert_final_newline);
    assert!(policy.skipped_non_utf8_sections);
}

#[test]
fn file_policy_requires_matching_utf8_section() {
    let dir = TempDir::new().unwrap();
    write_temp(
        dir.path(),
        ".editorconfig",
        "root = true\n\n[*.txt]\ntrim_trailing_whitespace = true\n",
    );
    let path = write_temp(dir.path(), "test.txt", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);

    assert!(!policy.has_matching_utf8_section);
    assert!(!policy.policy.trim_trailing_whitespace);
}

#[test]
fn file_policy_uses_matching_utf8_section_from_same_stack() {
    let dir = TempDir::new().unwrap();
    write_temp(
        dir.path(),
        ".editorconfig",
        "root = true\n\n[*.txt]\ncharset = utf-8\n\n[*.txt]\ntrim_trailing_whitespace = true\n",
    );
    let path = write_temp(dir.path(), "test.txt", "hello   ");

    let mut cache = PolicyCache::new();
    let policy = cache.file_policy(&path);

    assert!(policy.has_matching_utf8_section);
    assert!(policy.policy.trim_trailing_whitespace);
}

#[test]
fn root_config_accepts_any_utf8_section() {
    let dir = TempDir::new().unwrap();
    write_temp(
        dir.path(),
        ".editorconfig",
        "root = true\n\n[*.rs]\ncharset = utf-8\n",
    );

    let mut cache = PolicyCache::new();
    let root_config = cache.root_config(dir.path()).unwrap();

    assert_eq!(
        root_config,
        RootConfig {
            path: dir.path().join(".editorconfig"),
            has_utf8_section: true,
        }
    );
}

#[test]
fn root_config_reports_missing_utf8_sections() {
    let dir = TempDir::new().unwrap();
    write_temp(
        dir.path(),
        ".editorconfig",
        "root = true\n\n[*.rs]\ntrim_trailing_whitespace = true\n",
    );

    let mut cache = PolicyCache::new();
    let root_config = cache.root_config(dir.path()).unwrap();

    assert_eq!(
        root_config,
        RootConfig {
            path: dir.path().join(".editorconfig"),
            has_utf8_section: false,
        }
    );
}

#[test]
fn root_config_returns_none_when_missing() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target");
    fs::create_dir(&target).unwrap();

    let mut cache = PolicyCache::new();
    let root_config = cache.root_config(&target);

    assert!(root_config.is_none());
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
