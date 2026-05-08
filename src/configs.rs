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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndentStyle {
    Tab,
    Space,
}

impl IndentStyle {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tab => "tab",
            Self::Space => "space",
        }
    }
}

/// What to check/fix for a given file, derived from .editorconfig.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FilePolicy {
    pub trim_trailing_whitespace: bool,
    pub insert_final_newline: bool,
    pub single_final_newline: bool,
    pub end_of_line: Option<LineEnding>,
    pub indent_style: Option<IndentStyle>,
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
    has_utf8_section: bool,
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
    pub has_matching_utf8_section: bool,
    pub skipped_non_utf8_sections: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RootConfig {
    pub path: PathBuf,
    pub has_utf8_section: bool,
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

        let decision =
            self.properties_for(&normalized)
                .map_or_else(PolicyDecision::default, |resolved| PolicyDecision {
                    policy: if resolved.has_matching_utf8_section {
                        policy_from_properties(&resolved.props)
                    } else {
                        FilePolicy::default()
                    },
                    has_matching_utf8_section: resolved.has_matching_utf8_section,
                    skipped_non_utf8_sections: resolved.skipped_non_utf8_sections,
                });
        self.policies.insert(normalized, decision.clone());
        decision
    }

    pub fn root_config(&mut self, start: &Path) -> Option<RootConfig> {
        let normalized = self.normalize_target_path(start);
        let dir = if normalized.is_dir() {
            normalized
        } else if let Some(parent) = normalized.parent() {
            parent.to_path_buf()
        } else {
            return None;
        };

        let Ok(Some(stack)) = self.config_stack_for_dir(&dir) else {
            return None;
        };

        let mut current = Some(stack);
        while let Some(node) = current {
            let Ok(Some(parsed)) = self.parsed_config(&node.config_path) else {
                current = node.parent.clone();
                continue;
            };
            if parsed.is_root {
                return Some(RootConfig {
                    path: node.config_path.clone(),
                    has_utf8_section: parsed.has_utf8_section,
                });
            }
            current = node.parent.clone();
        }

        None
    }

    fn normalize_target_path(&self, path: &Path) -> PathBuf {
        if path.is_relative() {
            self.cwd.join(path)
        } else {
            path.to_path_buf()
        }
    }

    fn properties_for(&mut self, path: &Path) -> Option<ResolvedProperties> {
        let mut resolved = ResolvedProperties::default();
        let dir = if path.is_dir() { path } else { path.parent()? };
        let stack = self.config_stack_for_dir(dir).ok()??;

        self.apply_config_stack(&stack, path, &mut resolved);

        Some(resolved)
    }

    fn apply_config_stack(
        &mut self,
        stack: &ConfigStack,
        path: &Path,
        resolved: &mut ResolvedProperties,
    ) {
        if let Some(parent) = stack.parent.as_deref() {
            self.apply_config_stack(parent, path, resolved);
        }

        let Ok(Some(parsed)) = self.parsed_config(&stack.config_path) else {
            return;
        };

        let base = stack.config_path.parent().unwrap_or(Path::new(""));
        let relative = path.strip_prefix(base).unwrap_or(path);
        for parsed_section in &parsed.sections {
            if !parsed_section.section.applies_to(relative) {
                continue;
            }
            if parsed_section.charset == SectionCharset::Other {
                resolved.skipped_non_utf8_sections = true;
                continue;
            }
            if parsed_section.charset == SectionCharset::Utf8 {
                resolved.has_matching_utf8_section = true;
            }
            let _ = parsed_section
                .section
                .apply_to(&mut resolved.props, relative);
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
                    let mut has_utf8_section = false;
                    for section in &mut file {
                        match section {
                            Ok(section) => {
                                let parsed_section = ParsedSection {
                                    charset: section_charset(&section),
                                    section,
                                };
                                has_utf8_section |= parsed_section.charset == SectionCharset::Utf8;
                                sections.push(parsed_section);
                            }
                            Err(_) => return self.cache_parsed_config(path, Err(())),
                        }
                    }
                    Ok(Some(Rc::new(ParsedEditorConfig {
                        is_root,
                        has_utf8_section,
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

#[derive(Default)]
struct ResolvedProperties {
    props: Properties,
    skipped_non_utf8_sections: bool,
    has_matching_utf8_section: bool,
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
        indent_style: match props.get::<ec4rs::property::IndentStyle>() {
            Ok(ec4rs::property::IndentStyle::Tabs) => Some(IndentStyle::Tab),
            Ok(ec4rs::property::IndentStyle::Spaces) => Some(IndentStyle::Space),
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
