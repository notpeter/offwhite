pub(crate) struct Template {
    pub(crate) name: &'static str,
    pub(crate) contents: &'static str,
}

pub(crate) const TEMPLATES: &[Template] = &[
    Template {
        name: "default",
        contents: include_str!("../templates/default.editorconfig"),
    },
    Template {
        name: "gnu",
        contents: include_str!("../templates/gnu.editorconfig"),
    },
    Template {
        name: "java",
        contents: include_str!("../templates/java.editorconfig"),
    },
    Template {
        name: "lua",
        contents: include_str!("../templates/lua.editorconfig"),
    },
    Template {
        name: "rust",
        contents: include_str!("../templates/rust.editorconfig"),
    },
    Template {
        name: "typescript",
        contents: include_str!("../templates/typescript.editorconfig"),
    },
];

pub(crate) fn template_names() -> impl Iterator<Item = &'static str> {
    TEMPLATES.iter().map(|template| template.name)
}

pub(crate) fn find_template(name: &str) -> Option<&'static Template> {
    TEMPLATES.iter().find(|template| template.name == name)
}
