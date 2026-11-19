use crate::special_comments_policy::SpecialCommentsPolicy;
use crate::supported_languages::SupportedLanguage;
use anyhow::{Context, Result};
use std::collections::BTreeSet;
use tree_sitter::{Node, Parser, Tree};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DisallowedItemKind {
    Comment,
    Docstring,
}

pub struct DisallowedItem {
    pub kind: DisallowedItemKind,
    pub start_byte_offset: usize,
    pub end_byte_offset: usize,
    pub line_number: usize,
    pub column_number: usize,
    pub source_text: String,
    pub replace_with_ellipsis_to_keep_body_nonempty: bool,
}

pub struct SuccessfullyParsedSource {
    pub language: SupportedLanguage,
    pub tree: Tree,
}

pub fn parse_source_text(source_text: &str, language: SupportedLanguage) -> Result<Tree> {
    let mut parser = Parser::new();
    parser
        .set_language(&language.tree_sitter_language())
        .context("Cannot load the tree-sitter grammar")?;
    parser
        .parse(source_text, None)
        .context("tree-sitter did not return a syntax tree")
}

pub fn parse_with_first_language_without_syntax_errors(
    source_text: &str,
    parsing_candidates: &[SupportedLanguage],
) -> Result<Option<SuccessfullyParsedSource>> {
    for &language in parsing_candidates {
        let tree = parse_source_text(source_text, language)?;
        if !tree.root_node().has_error() {
            return Ok(Some(SuccessfullyParsedSource { language, tree }));
        }
    }
    Ok(None)
}

pub fn find_disallowed_comments_and_docstrings(
    source_text: &str,
    parsed_source: &SuccessfullyParsedSource,
    project_relative_path: &str,
    policy: &SpecialCommentsPolicy,
) -> Vec<DisallowedItem> {
    let language = parsed_source.language;
    let mut comment_nodes = Vec::new();
    let mut docstring_nodes = Vec::new();
    let mut pending_nodes = vec![parsed_source.tree.root_node()];
    while let Some(node) = pending_nodes.pop() {
        if language.is_comment_node_kind(node.kind()) {
            comment_nodes.push(node);
            continue;
        }
        if language == SupportedLanguage::Python && is_python_docstring_statement(node, language) {
            docstring_nodes.push(node);
            continue;
        }
        let mut cursor = node.walk();
        let children: Vec<Node<'_>> = node.children(&mut cursor).collect();
        pending_nodes.extend(children.into_iter().rev());
    }

    let mut disallowed_items = Vec::new();
    for comment_node in comment_nodes {
        let (start_byte_offset, end_byte_offset) =
            byte_range_without_trailing_line_break(source_text, comment_node);
        let comment_text = source_text[start_byte_offset..end_byte_offset].replace("\r\n", "\n");
        if !policy.is_comment_allowed(project_relative_path, &comment_text) {
            disallowed_items.push(build_disallowed_item(
                source_text,
                DisallowedItemKind::Comment,
                start_byte_offset,
                end_byte_offset,
                false,
            ));
        }
    }
    if !policy.are_docstrings_allowed(project_relative_path) {
        let docstring_ids_to_replace_with_ellipsis =
            docstring_ids_whose_removal_would_leave_an_empty_body(&docstring_nodes, language);
        for docstring_node in &docstring_nodes {
            let (start_byte_offset, end_byte_offset) =
                byte_range_without_trailing_line_break(source_text, *docstring_node);
            disallowed_items.push(build_disallowed_item(
                source_text,
                DisallowedItemKind::Docstring,
                start_byte_offset,
                end_byte_offset,
                docstring_ids_to_replace_with_ellipsis.contains(&docstring_node.id()),
            ));
        }
    }
    disallowed_items.sort_by_key(|item| item.start_byte_offset);
    disallowed_items
}

fn is_python_docstring_statement(node: Node<'_>, language: SupportedLanguage) -> bool {
    if node.kind() != "expression_statement" {
        return false;
    }
    let mut cursor = node.walk();
    let non_comment_named_children: Vec<Node<'_>> = node
        .named_children(&mut cursor)
        .filter(|child| !language.is_comment_node_kind(child.kind()))
        .collect();
    matches!(
        non_comment_named_children.as_slice(),
        [only_child] if matches!(only_child.kind(), "string" | "concatenated_string")
    )
}

fn docstring_ids_whose_removal_would_leave_an_empty_body(
    docstring_nodes_in_source_order: &[Node<'_>],
    language: SupportedLanguage,
) -> BTreeSet<usize> {
    let removed_docstring_ids: BTreeSet<usize> = docstring_nodes_in_source_order
        .iter()
        .map(Node::id)
        .collect();
    let mut examined_body_ids = BTreeSet::new();
    let mut docstring_ids_to_replace = BTreeSet::new();
    for docstring_node in docstring_nodes_in_source_order {
        let Some(body) = docstring_node.parent() else {
            continue;
        };
        if body.kind() != "block" || !examined_body_ids.insert(body.id()) {
            continue;
        }
        let mut cursor = body.walk();
        let body_keeps_a_statement = body.named_children(&mut cursor).any(|statement| {
            !language.is_comment_node_kind(statement.kind())
                && !removed_docstring_ids.contains(&statement.id())
        });
        if !body_keeps_a_statement {
            docstring_ids_to_replace.insert(docstring_node.id());
        }
    }
    docstring_ids_to_replace
}

fn byte_range_without_trailing_line_break(source_text: &str, node: Node<'_>) -> (usize, usize) {
    let start_byte_offset = node.start_byte();
    let mut end_byte_offset = node.end_byte();
    let source_bytes = source_text.as_bytes();
    while end_byte_offset > start_byte_offset
        && matches!(source_bytes[end_byte_offset - 1], b'\n' | b'\r')
    {
        end_byte_offset -= 1;
    }
    (start_byte_offset, end_byte_offset)
}

fn build_disallowed_item(
    source_text: &str,
    kind: DisallowedItemKind,
    start_byte_offset: usize,
    end_byte_offset: usize,
    replace_with_ellipsis_to_keep_body_nonempty: bool,
) -> DisallowedItem {
    let text_before_item = &source_text[..start_byte_offset];
    let line_start_byte_offset = text_before_item.rfind('\n').map_or(0, |index| index + 1);
    DisallowedItem {
        kind,
        start_byte_offset,
        end_byte_offset,
        line_number: text_before_item.matches('\n').count() + 1,
        column_number: source_text[line_start_byte_offset..start_byte_offset]
            .chars()
            .count()
            + 1,
        source_text: source_text[start_byte_offset..end_byte_offset].to_owned(),
        replace_with_ellipsis_to_keep_body_nonempty,
    }
}
