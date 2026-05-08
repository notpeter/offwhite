use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};

use ec4rs::glob::Glob;

use crate::action::{WalkOptions, walk_paths_with};
use crate::args::Verbosity;
use crate::output::write_stdout;
use crate::templates::template_names;

const NO_EXTENSION_LABEL: &str = "[no extension]";

pub(crate) struct ExtensionSummary {
    counts: Vec<(String, u64)>,
    no_extension_names: Vec<(String, u64)>,
}

pub(crate) fn list_editorconfigs(paths: &[String], respect_ignore_files: bool) -> io::Result<bool> {
    let (configs, mut found_failures) = discover_editorconfigs(paths, respect_ignore_files)?;

    for path in configs {
        match validate_editorconfig(&path) {
            Ok(()) => write_stdout(format_args!("{}\n", path.display()))?,
            Err(err) => {
                eprintln!("{err}");
                found_failures = true;
            }
        }
    }

    Ok(found_failures)
}

pub(crate) fn list_extensions(
    paths: &[String],
    respect_ignore_files: bool,
    verbosity: Verbosity,
) -> io::Result<bool> {
    let (summary, found_failures) = summarize_extensions(paths, respect_ignore_files)?;

    for line in render_extension_summary(&summary, verbosity) {
        write_stdout(format_args!("{line}\n"))?;
    }

    Ok(found_failures)
}

pub(crate) fn list_templates() -> io::Result<()> {
    for name in template_names() {
        write_stdout(format_args!("{name}\n"))?;
    }

    Ok(())
}

pub(crate) fn discover_editorconfigs(
    paths: &[String],
    respect_ignore_files: bool,
) -> io::Result<(Vec<PathBuf>, bool)> {
    let (paths, found_failures) = collect_existing_paths(paths);
    let mut configs = BTreeSet::new();

    walk_paths_with(&paths, WalkOptions::listing(respect_ignore_files), |path| {
        if is_editorconfig_path(&path) {
            configs.insert(path);
        }
        Ok(())
    })?;

    Ok((configs.into_iter().collect(), found_failures))
}

pub(crate) fn summarize_extensions(
    paths: &[String],
    respect_ignore_files: bool,
) -> io::Result<(ExtensionSummary, bool)> {
    let (paths, found_failures) = collect_existing_paths(paths);
    let mut counts = BTreeMap::new();
    let mut no_extension_names = BTreeMap::new();

    walk_paths_with(
        &paths,
        WalkOptions::extension_listing(respect_ignore_files),
        |path| {
            match extension_key(&path) {
                Some(ExtensionKey::Extension(label)) => {
                    *counts.entry(label).or_insert(0_u64) += 1;
                }
                Some(ExtensionKey::NoExtension(name)) => {
                    *counts
                        .entry(NO_EXTENSION_LABEL.to_string())
                        .or_insert(0_u64) += 1;
                    *no_extension_names.entry(name).or_insert(0_u64) += 1;
                }
                None => {}
            }
            Ok(())
        },
    )?;

    Ok((
        ExtensionSummary {
            counts: sort_counts(counts),
            no_extension_names: sort_counts(no_extension_names),
        },
        found_failures,
    ))
}

pub(crate) fn render_extension_summary(
    summary: &ExtensionSummary,
    verbosity: Verbosity,
) -> Vec<String> {
    let mut lines = Vec::new();

    for (label, count) in &summary.counts {
        if verbosity >= Verbosity::Verbose && label == NO_EXTENSION_LABEL {
            for (name, count) in &summary.no_extension_names {
                lines.push(format!("{count}\t{name}"));
            }
        } else {
            lines.push(format!("{count}\t{label}"));
        }
    }

    lines
}

fn sort_counts(counts: BTreeMap<String, u64>) -> Vec<(String, u64)> {
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|(left_label, left_count), (right_label, right_count)| {
        right_count
            .cmp(left_count)
            .then_with(|| left_label.cmp(right_label))
    });
    counts
}

fn collect_existing_paths(paths: &[String]) -> (Vec<String>, bool) {
    let mut existing = Vec::new();
    let mut found_failures = false;

    for path in paths {
        let target = PathBuf::from(path);
        if target.exists() {
            existing.push(path.clone());
        } else {
            eprintln!("{}: no such file or directory", target.display());
            found_failures = true;
        }
    }

    (existing, found_failures)
}

fn is_editorconfig_path(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name == OsStr::new(".editorconfig"))
}

enum ExtensionKey {
    Extension(String),
    NoExtension(String),
}

fn extension_key(path: &Path) -> Option<ExtensionKey> {
    if is_editorconfig_path(path) {
        return None;
    }

    match path.extension().filter(|ext| !ext.is_empty()) {
        Some(ext) => Some(ExtensionKey::Extension(format!(
            ".{}",
            ext.to_string_lossy().to_lowercase()
        ))),
        None => Some(ExtensionKey::NoExtension(
            path.file_name()
                .unwrap_or_else(|| OsStr::new(""))
                .to_string_lossy()
                .into_owned(),
        )),
    }
}

pub(crate) fn validate_editorconfig(path: &Path) -> Result<(), String> {
    let mut file = ec4rs::ConfigFile::<Glob>::open(path)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    while let Some(section) = file.next() {
        if let Err(err) = section {
            return Err(file.add_error_context(err).to_string());
        }
    }

    Ok(())
}
