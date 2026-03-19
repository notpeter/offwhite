use ignore::overrides::OverrideBuilder;
use std::path::Path;

/// Default ignore patterns (applied case-insensitively).
pub const DEFAULT_GLOBS: &[&str] = &[
    // Version control
    "!.git/",
    "!.hg/",
    "!.svn/",
    // Patch / diff
    "!*.patch",
    "!*.diff",
    "!*.rej",
    "!*.patchset",
    // Images
    "!*.png",
    "!*.jpg",
    "!*.jpeg",
    "!*.gif",
    "!*.bmp",
    "!*.ico",
    "!*.icns",
    "!*.tif",
    "!*.tiff",
    "!*.webp",
    "!*.avif",
    "!*.heic",
    "!*.heif",
    "!*.svg", // often auto-generated, binary-ish
    "!*.psd",
    "!*.ai",
    "!*.eps",
    // Audio
    "!*.mp3",
    "!*.wav",
    "!*.ogg",
    "!*.flac",
    "!*.aac",
    "!*.m4a",
    "!*.wma",
    // Video
    "!*.mp4",
    "!*.mkv",
    "!*.avi",
    "!*.mov",
    "!*.wmv",
    "!*.flv",
    "!*.webm",
    // Fonts
    "!*.ttf",
    "!*.otf",
    "!*.woff",
    "!*.woff2",
    "!*.eot",
    // Compiled / executables
    "!*.exe",
    "!*.msi",
    "!*.dll",
    "!*.so",
    "!*.dylib",
    "!*.a",
    "!*.lib",
    "!*.o",
    "!*.obj",
    "!*.out",
    "!*.bin",
    "!*.class",
    "!*.pyc",
    "!*.pyo",
    "!*.elc",
    "!*.beam",
    // Archives
    "!*.zip",
    "!*.tar",
    "!*.gz",
    "!*.bz2",
    "!*.xz",
    "!*.zst",
    "!*.lz",
    "!*.lzma",
    "!*.7z",
    "!*.rar",
    "!*.cab",
    "!*.dmg",
    "!*.iso",
    "!*.jar",
    "!*.war",
    "!*.ear",
    "!*.deb",
    "!*.rpm",
    "!*.apk",
    "!*.ipa",
    // Documents (binary)
    "!*.pdf",
    "!*.doc",
    "!*.docx",
    "!*.xls",
    "!*.xlsx",
    "!*.ppt",
    "!*.pptx",
    "!*.odt",
    "!*.ods",
    "!*.odp",
    // Database
    "!*.sqlite",
    "!*.sqlite3",
    "!*.db",
    // Certificates (binary DER format)
    "!*.der",
    "!*.p12",
    "!*.pfx",
    "!*.jks",
    // WebAssembly (binary)
    "!*.wasm",
    // Misc binary
    "!*.swp",
    "!*.swo",
    "!*.DS_Store",
    "!*.snap",
    "!*.min.js",
    "!*.min.css",
    "!*.map",
    // License
    "!licen[cs]e*",
    "!copying*",
];

pub(crate) fn build_ignore_overrides(
    base: &Path,
    include_glob: Option<&str>,
) -> ignore::overrides::Override {
    let mut builder = OverrideBuilder::new(base);
    builder
        .case_insensitive(true)
        .expect("failed to set case insensitive");
    for pat in DEFAULT_GLOBS {
        builder.add(pat).expect("invalid default ignore glob");
    }
    builder
        .case_insensitive(false)
        .expect("failed to reset case insensitive");
    if let Some(glob) = include_glob {
        builder.add(glob).expect("invalid include glob");
    }
    builder.build().expect("failed to build overrides")
}
