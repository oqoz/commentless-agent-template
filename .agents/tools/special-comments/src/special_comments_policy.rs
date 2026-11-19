use anyhow::{Context, Result, bail};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub const SPECIAL_COMMENTS_CONFIGURATION_FILE_NAME: &str = "special-comments.toml";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecialCommentsConfigurationFile {
    #[serde(default)]
    allowed_comments: Vec<AllowedCommentsEntry>,
    #[serde(default)]
    allowed_docstrings: Vec<AllowedDocstringsEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowedCommentsEntry {
    files: Vec<String>,
    patterns: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AllowedDocstringsEntry {
    files: Vec<String>,
}

#[derive(Clone, Copy)]
enum CommentPatternElement {
    LiteralCharacter(char),
    ZeroOrMoreNonWhitespaceCharacters,
    ExactlyOneNonWhitespaceCharacter,
}

struct CommentTextPattern {
    elements: Vec<CommentPatternElement>,
}

impl CommentTextPattern {
    fn parse(pattern_text: &str) -> Result<Self> {
        if pattern_text.is_empty() {
            bail!("Comment patterns must not be empty");
        }
        let elements = pattern_text
            .chars()
            .map(|pattern_character| match pattern_character {
                '*' => CommentPatternElement::ZeroOrMoreNonWhitespaceCharacters,
                '?' => CommentPatternElement::ExactlyOneNonWhitespaceCharacter,
                literal_character => CommentPatternElement::LiteralCharacter(literal_character),
            })
            .collect();
        Ok(Self { elements })
    }

    fn matches_entire_comment_text(&self, comment_text: &str) -> bool {
        let comment_characters: Vec<char> = comment_text.chars().collect();
        let is_non_whitespace_at = |index: usize| {
            comment_characters
                .get(index)
                .is_some_and(|character| !character.is_whitespace())
        };
        let mut is_prefix_length_matched = vec![false; comment_characters.len() + 1];
        is_prefix_length_matched[0] = true;
        for element in &self.elements {
            let mut is_next_prefix_length_matched = vec![false; comment_characters.len() + 1];
            for prefix_length in 0..=comment_characters.len() {
                if !is_prefix_length_matched[prefix_length] {
                    continue;
                }
                match element {
                    CommentPatternElement::LiteralCharacter(expected_character) => {
                        if comment_characters.get(prefix_length) == Some(expected_character) {
                            is_next_prefix_length_matched[prefix_length + 1] = true;
                        }
                    }
                    CommentPatternElement::ExactlyOneNonWhitespaceCharacter => {
                        if is_non_whitespace_at(prefix_length) {
                            is_next_prefix_length_matched[prefix_length + 1] = true;
                        }
                    }
                    CommentPatternElement::ZeroOrMoreNonWhitespaceCharacters => {
                        is_next_prefix_length_matched[prefix_length] = true;
                        let mut extended_prefix_length = prefix_length;
                        while is_non_whitespace_at(extended_prefix_length) {
                            extended_prefix_length += 1;
                            is_next_prefix_length_matched[extended_prefix_length] = true;
                        }
                    }
                }
            }
            is_prefix_length_matched = is_next_prefix_length_matched;
        }
        is_prefix_length_matched[comment_characters.len()]
    }
}

struct CompiledAllowedComments {
    file_path_globs: GlobSet,
    comment_text_patterns: Vec<CommentTextPattern>,
}

pub struct SpecialCommentsPolicy {
    allowed_comments: Vec<CompiledAllowedComments>,
    file_path_globs_allowing_docstrings: GlobSet,
}

impl SpecialCommentsPolicy {
    pub fn read_from_project_root(project_root: &Path) -> Result<Self> {
        let configuration_path = project_root.join(SPECIAL_COMMENTS_CONFIGURATION_FILE_NAME);
        let configuration_text = fs::read_to_string(&configuration_path)
            .with_context(|| format!("Cannot read {}", configuration_path.display()))?;
        Self::parse_configuration_text(&configuration_text)
            .with_context(|| format!("Invalid {}", configuration_path.display()))
    }

    fn parse_configuration_text(configuration_text: &str) -> Result<Self> {
        let configuration: SpecialCommentsConfigurationFile = toml::from_str(configuration_text)?;
        let mut allowed_comments = Vec::new();
        for entry in &configuration.allowed_comments {
            if entry.patterns.is_empty() {
                bail!("Each allowed_comments entry needs at least one pattern");
            }
            allowed_comments.push(CompiledAllowedComments {
                file_path_globs: compile_file_path_glob_set(&entry.files, "allowed_comments")?,
                comment_text_patterns: entry
                    .patterns
                    .iter()
                    .map(|pattern_text| CommentTextPattern::parse(pattern_text))
                    .collect::<Result<_>>()?,
            });
        }
        let mut docstring_file_path_globs = GlobSetBuilder::new();
        for entry in &configuration.allowed_docstrings {
            if entry.files.is_empty() {
                bail!("Each allowed_docstrings entry needs at least one file pattern");
            }
            for file_path_pattern in &entry.files {
                docstring_file_path_globs.add(compile_file_path_glob(file_path_pattern)?);
            }
        }
        Ok(Self {
            allowed_comments,
            file_path_globs_allowing_docstrings: docstring_file_path_globs.build()?,
        })
    }

    pub fn is_comment_allowed(&self, project_relative_path: &str, comment_text: &str) -> bool {
        self.allowed_comments.iter().any(|allowed| {
            allowed.file_path_globs.is_match(project_relative_path)
                && allowed
                    .comment_text_patterns
                    .iter()
                    .any(|pattern| pattern.matches_entire_comment_text(comment_text))
        })
    }

    pub fn are_docstrings_allowed(&self, project_relative_path: &str) -> bool {
        self.file_path_globs_allowing_docstrings
            .is_match(project_relative_path)
    }
}

fn compile_file_path_glob(file_path_pattern: &str) -> Result<Glob> {
    GlobBuilder::new(file_path_pattern)
        .literal_separator(true)
        .build()
        .with_context(|| format!("Invalid file pattern: {file_path_pattern}"))
}

fn compile_file_path_glob_set(
    file_path_patterns: &[String],
    section_name: &str,
) -> Result<GlobSet> {
    if file_path_patterns.is_empty() {
        bail!("Each {section_name} entry needs at least one file pattern");
    }
    let mut file_path_globs = GlobSetBuilder::new();
    for file_path_pattern in file_path_patterns {
        file_path_globs.add(compile_file_path_glob(file_path_pattern)?);
    }
    Ok(file_path_globs.build()?)
}
