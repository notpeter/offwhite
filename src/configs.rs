use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    rc::Rc,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FilePolicy {
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub single_final_newline: bool,
    pub end_of_line: Option<LineEnding>,
}

#[derive(Clone)]
struct ParsedSection {
    section: Section<Glob>,
    charset: SectionCharset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SectionCharset {
    Unset,
    Utf8,
    Other,
}

#[derive(Clone)]
struct ParsedEditorConfig {
    is_root: bool,
    has_utf8_root_section: bool,
    sections: Vec<ParsedSection>,
}

type CachedParsedConfig = Result<Option<Rc<ParsedEditorConfig>>, ()>;

#[derive(Clone)]
struct ConfigStack {
    parent: Option<Rc<ConfigStack>>,
    config_path: PathBuf,
}

type CachedConfigStack = Result<Option<Rc<ConfigStack>>, ()>;

#[derive(Clone, Default)]
pub struct PolicyDecision {
    pub policy: FilePolicy,
    pub skipped_non_utf8_sections: bool,
    pub nested_root_missing_utf8: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RootConfigStatus {
    Missing,
    MissingUtf8,
    Ready,
}

pub struct PolicyCache {
    cwd: PathBuf,
    parsed_configs: HashMap<PathBuf, CachedParsedConfig>,
    directory_configs: HashMap<PathBuf, CachedConfigStack>,
    policies: HashMap<PathBuf, PolicyDecision>,
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

    pub fn file_policy(&mut self, path: &Path) -> PolicyDecision {
        let normalized = self.normalize_target_path(path);
        if let Some(policy) = self.policies.get(&normalized) {
            return policy.clone();
        }

        let decision = self.properties_for(&normalized).map_or_else(
            || PolicyDecision::default(),
            |(props, skipped_non_utf8_sections, nested_root_missing_utf8)| PolicyDecision {
                policy: policy_from_properties(&props),
                skipped_non_utf8_sections,
                nested_root_missing_utf8,
            },
        );
        self.policies.insert(normalized, decision.clone());
        decision
    }

    pub fn root_config_status(&mut self, start: &Path) -> RootConfigStatus {
        let normalized = self.normalize_target_path(start);
        let dir = if normalized.is_dir() {
            normalized
        } else if let Some(parent) = normalized.parent() {
            parent.to_path_buf()
        } else {
            return RootConfigStatus::Missing;
        };

        let Ok(Some(stack)) = self.config_stack_for_dir(&dir) else {
            return RootConfigStatus::Missing;
        };

        let mut current = Some(stack);
        while let Some(node) = current {
            let Ok(Some(parsed)) = self.parsed_config(&node.config_path) else {
                current = node.parent.clone();
                continue;
            };
            if parsed.is_root {
                return if parsed.has_utf8_root_section {
                    RootConfigStatus::Ready
                } else {
                    RootConfigStatus::MissingUtf8
                };
            }
            current = node.parent.clone();
        }

        RootConfigStatus::Missing
    }

    fn normalize_target_path(&self, path: &Path) -> PathBuf {
        if path.is_relative() {
            self.cwd.join(path)
        } else {
            path.to_path_buf()
        }
    }

    fn properties_for(&mut self, path: &Path) -> Option<(Properties, bool, Option<PathBuf>)> {
        let mut props = Properties::new();
        let mut skipped_non_utf8_sections = false;
        let mut nested_root_missing_utf8 = None;
        let dir = if path.is_dir() { path } else { path.parent()? };
        let stack = self.config_stack_for_dir(dir).ok()??;

        self.apply_config_stack(
            &stack,
            path,
            &mut props,
            &mut skipped_non_utf8_sections,
            &mut nested_root_missing_utf8,
        );

        Some((props, skipped_non_utf8_sections, nested_root_missing_utf8))
    }

    fn apply_config_stack(
        &mut self,
        stack: &ConfigStack,
        path: &Path,
        props: &mut Properties,
        skipped_non_utf8_sections: &mut bool,
        nested_root_missing_utf8: &mut Option<PathBuf>,
    ) {
        if let Some(parent) = stack.parent.as_deref() {
            self.apply_config_stack(
                parent,
                path,
                props,
                skipped_non_utf8_sections,
                nested_root_missing_utf8,
            );
        }

        let Ok(Some(parsed)) = self.parsed_config(&stack.config_path) else {
            return;
        };
        if parsed.is_root && !parsed.has_utf8_root_section {
            nested_root_missing_utf8.get_or_insert_with(|| stack.config_path.clone());
        }

        let base = stack.config_path.parent().unwrap_or(Path::new(""));
        let relative = path.strip_prefix(base).unwrap_or(path);
        for parsed_section in &parsed.sections {
            if !parsed_section.section.applies_to(relative) {
                continue;
            }
            if parsed_section.charset == SectionCharset::Other {
                *skipped_non_utf8_sections = true;
                continue;
            }
            let _ = parsed_section.section.apply_to(props, relative);
        }
    }

    fn config_stack_for_dir(&mut self, dir: &Path) -> CachedConfigStack {
        if let Some(cached) = self.directory_configs.get(dir) {
            return cached.clone();
        }

        let parent_stack = dir
            .parent()
            .map(|parent| self.config_stack_for_dir(parent))
            .transpose()?
            .flatten();

        let config_path = dir.join(".editorconfig");
        let stack = if let Some(parsed) = self.parsed_config(&config_path)? {
            Some(Rc::new(ConfigStack {
                parent: if parsed.is_root { None } else { parent_stack },
                config_path,
            }))
        } else {
            parent_stack
        };

        let cached = Ok(stack);
        self.directory_configs
            .insert(dir.to_path_buf(), cached.clone());
        cached
    }

    fn parsed_config(&mut self, path: &Path) -> CachedParsedConfig {
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
                    let mut has_utf8_root_section = false;
                    for section in &mut file {
                        match section {
                            Ok(section) => {
                                let parsed_section = ParsedSection {
                                    charset: section_charset(&section),
                                    section,
                                };
                                has_utf8_root_section |= parsed_section.charset
                                    == SectionCharset::Utf8
                                    && is_global_section(&parsed_section.section);
                                sections.push(parsed_section);
                            }
                            Err(_) => return self.cache_parsed_config(path, Err(())),
                        }
                    }
                    Ok(Some(Rc::new(ParsedEditorConfig {
                        is_root,
                        has_utf8_root_section,
                        sections,
                    })))
                }
                Err(_) => Err(()),
            }
        };

        self.cache_parsed_config(path, parsed)
    }

    fn cache_parsed_config(
        &mut self,
        path: &Path,
        parsed: CachedParsedConfig,
    ) -> CachedParsedConfig {
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

fn section_charset(section: &Section<Glob>) -> SectionCharset {
    match section.props().get::<ec4rs::property::Charset>() {
        Ok(ec4rs::property::Charset::Utf8) => SectionCharset::Utf8,
        Ok(_) => SectionCharset::Other,
        Err(_) => SectionCharset::Unset,
    }
}

fn is_global_section(section: &Section<Glob>) -> bool {
    section
        .pattern()
        .as_ref()
        .is_ok_and(|pattern| pattern == &Glob::new("*"))
}
