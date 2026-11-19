use std::path::Path;
use tree_sitter::Language;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SupportedLanguage {
    C,
    CPlusPlus,
    Python,
    Rust,
}

impl SupportedLanguage {
    pub fn parsing_candidates_for_file_path(file_path: &Path) -> &'static [SupportedLanguage] {
        let lowercase_extension = file_path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        match lowercase_extension.as_deref() {
            Some("c") => &[SupportedLanguage::C],
            Some("h") => &[SupportedLanguage::C, SupportedLanguage::CPlusPlus],
            Some("cc" | "cpp" | "cxx" | "c++" | "hh" | "hpp" | "hxx" | "h++") => {
                &[SupportedLanguage::CPlusPlus]
            }
            Some("py" | "pyi") => &[SupportedLanguage::Python],
            Some("rs") => &[SupportedLanguage::Rust],
            _ => &[],
        }
    }

    pub fn tree_sitter_language(self) -> Language {
        match self {
            Self::C => tree_sitter_c::LANGUAGE.into(),
            Self::CPlusPlus => tree_sitter_cpp::LANGUAGE.into(),
            Self::Python => tree_sitter_python::LANGUAGE.into(),
            Self::Rust => tree_sitter_rust::LANGUAGE.into(),
        }
    }

    pub fn is_comment_node_kind(self, node_kind: &str) -> bool {
        match self {
            Self::Rust => matches!(node_kind, "line_comment" | "block_comment"),
            Self::C | Self::CPlusPlus | Self::Python => node_kind == "comment",
        }
    }
}
