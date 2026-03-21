use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use ec4rs::{Properties, PropertiesSource, Section, glob::Glob};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    CrLf,
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "lf",
            Self::CrLf => "crlf",
            Self::Cr => "cr",
        }
    }
}

/// What to check/fix for a given file, derived from .editorconfig.
#[derive(Clone, Copy)]
pub struct FilePolicy {
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub single_final_newline: bool,
    pub end_of_line: Option<LineEnding>,
}

#[derive(Clone)]
struct ParsedEditorConfig {
    is_root: bool,
    sections: Vec<Section<Glob>>,
}

pub struct PolicyCache {
    cwd: PathBuf,
    parsed_configs: HashMap<PathBuf, Result<Option<ParsedEditorConfig>, ()>>,
    directory_configs: HashMap<PathBuf, Result<Vec<PathBuf>, ()>>,
    policies: HashMap<PathBuf, FilePolicy>,
}

impl PolicyCache {
    pub fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            parsed_configs: HashMap::new(),
            directory_configs: HashMap::new(),
            policies: HashMap::new(),
        }
    }

    pub fn file_policy(&mut self, path: &Path) -> FilePolicy {
        let normalized = self.normalize_target_path(path);
        if let Some(policy) = self.policies.get(&normalized) {
            return *policy;
        }

        let policy = self
            .properties_for(&normalized)
            .map(|props| policy_from_properties(&props))
            .unwrap_or_default();
        self.policies.insert(normalized, policy);
        policy
    }

    pub fn has_editorconfigs(&mut self, start: &Path) -> bool {
        let normalized = self.normalize_target_path(start);
        let dir = if normalized.is_dir() {
            normalized
        } else if let Some(parent) = normalized.parent() {
            parent.to_path_buf()
        } else {
            return false;
        };

        self.config_stack_for_dir(&dir)
            .is_some_and(|stack| !stack.is_empty())
    }

    fn normalize_target_path(&self, path: &Path) -> PathBuf {
        if path.is_relative() {
            self.cwd.join(path)
        } else {
            path.to_path_buf()
        }
    }

    fn properties_for(&mut self, path: &Path) -> Option<Properties> {
        let mut props = Properties::new();
        let dir = if path.is_dir() { path } else { path.parent()? };

        for config_path in self.config_stack_for_dir(dir)? {
            let parsed = self.parsed_config(&config_path).ok()??;
            let base = config_path.parent().unwrap_or(Path::new(""));
            let relative = path.strip_prefix(base).unwrap_or(path);
            for section in &parsed.sections {
                let _ = section.apply_to(&mut props, relative);
            }
        }

        Some(props)
    }

    fn config_stack_for_dir(&mut self, dir: &Path) -> Option<Vec<PathBuf>> {
        if let Some(cached) = self.directory_configs.get(dir) {
            return cached.clone().ok();
        }

        let mut stack = dir
            .parent()
            .and_then(|parent| self.config_stack_for_dir(parent))
            .unwrap_or_default();

        let config_path = dir.join(".editorconfig");
        if let Some(parsed) = self.parsed_config(&config_path).ok()? {
            if parsed.is_root {
                stack.clear();
            }
            stack.push(config_path);
        }

        self.directory_configs
            .insert(dir.to_path_buf(), Ok(stack.clone()));
        Some(stack)
    }

    fn parsed_config(&mut self, path: &Path) -> Result<Option<ParsedEditorConfig>, ()> {
        if let Some(cached) = self.parsed_configs.get(path) {
            return cached.clone();
        }

        let parsed = if !path.is_file() {
            Ok(None)
        } else {
            match ec4rs::ConfigFile::open(path) {
                Ok(mut file) => {
                    let is_root = file.reader.is_root;
                    let mut sections = Vec::new();
                    for section in &mut file {
                        match section {
                            Ok(section) => sections.push(section),
                            Err(_) => return self.cache_parsed_config(path, Err(())),
                        }
                    }
                    Ok(Some(ParsedEditorConfig { is_root, sections }))
                }
                Err(_) => Err(()),
            }
        };

        self.cache_parsed_config(path, parsed)
    }

    fn cache_parsed_config(
        &mut self,
        path: &Path,
        parsed: Result<Option<ParsedEditorConfig>, ()>,
    ) -> Result<Option<ParsedEditorConfig>, ()> {
        self.parsed_configs
            .insert(path.to_path_buf(), parsed.clone());
        parsed
    }
}

impl Default for FilePolicy {
    fn default() -> Self {
        Self {
            trim_trailing_whitespace: false,
            insert_final_newline: false,
            single_final_newline: false,
            end_of_line: None,
        }
    }
}

/// Look up .editorconfig properties for a file path.
pub fn file_policy(path: &Path) -> FilePolicy {
    PolicyCache::new().file_policy(path)
}

fn policy_from_properties(props: &Properties) -> FilePolicy {
    FilePolicy {
        trim_trailing_whitespace: matches!(
            props.get::<ec4rs::property::TrimTrailingWs>(),
            Ok(ec4rs::property::TrimTrailingWs::Value(true))
        ),
        insert_final_newline: matches!(
            props.get::<ec4rs::property::FinalNewline>(),
            Ok(ec4rs::property::FinalNewline::Value(true))
        ),
        single_final_newline: false,
        end_of_line: match props.get::<ec4rs::property::EndOfLine>() {
            Ok(ec4rs::property::EndOfLine::Lf) => Some(LineEnding::Lf),
            Ok(ec4rs::property::EndOfLine::CrLf) => Some(LineEnding::CrLf),
            Ok(ec4rs::property::EndOfLine::Cr) => Some(LineEnding::Cr),
            _ => None,
        },
    }
}
