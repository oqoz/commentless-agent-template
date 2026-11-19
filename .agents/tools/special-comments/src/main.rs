mod comment_and_docstring_extraction;
mod comment_and_docstring_removal;
mod special_comments_policy;
mod supported_languages;
mod target_file_selection;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use comment_and_docstring_extraction::{
    DisallowedItem, DisallowedItemKind, find_disallowed_comments_and_docstrings, parse_source_text,
    parse_with_first_language_without_syntax_errors,
};
use comment_and_docstring_removal::remove_disallowed_items_from_source_text;
use special_comments_policy::SpecialCommentsPolicy;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use supported_languages::SupportedLanguage;
use target_file_selection::{TargetFile, TargetFileSelection, list_target_files};

#[derive(Parser)]
#[command(
    version,
    about = "Check or remove comments and Python docstrings that special-comments.toml does not allow"
)]
struct CommandLineArguments {
    #[arg(
        long,
        global = true,
        default_value = ".",
        help = "Project root that contains special-comments.toml"
    )]
    project_root: PathBuf,
    #[command(subcommand)]
    requested_action: RequestedAction,
}

#[derive(Subcommand)]
enum RequestedAction {
    #[command(about = "Report disallowed comments and docstrings without changing files")]
    Check(TargetFileArguments),
    #[command(about = "Remove disallowed comments and docstrings from files")]
    Remove(TargetFileArguments),
}

#[derive(Args)]
#[group(required = true, multiple = false)]
struct TargetFileArguments {
    #[arg(long, help = "Process the files staged in the Git index")]
    staged: bool,
    #[arg(long, help = "Process every file tracked by Git")]
    all: bool,
    #[arg(help = "Process these files")]
    file_paths: Vec<PathBuf>,
}

impl TargetFileArguments {
    fn into_target_file_selection(self) -> TargetFileSelection {
        if self.staged {
            TargetFileSelection::StagedFiles
        } else if self.all {
            TargetFileSelection::AllTrackedFiles
        } else {
            TargetFileSelection::ExplicitFilePaths(self.file_paths)
        }
    }
}

enum FileProcessingOutcome {
    UnsupportedLanguage,
    Skipped {
        reason: String,
    },
    Processed {
        disallowed_items: Vec<DisallowedItem>,
    },
}

#[derive(Default)]
struct RunSummary {
    processed_file_count: usize,
    file_count_with_disallowed_items: usize,
    disallowed_item_count: usize,
    skipped_file_count: usize,
}

fn check_or_remove_disallowed_items_in_file(
    target_file: &TargetFile,
    policy: &SpecialCommentsPolicy,
    should_remove_disallowed_items: bool,
) -> Result<FileProcessingOutcome> {
    let parsing_candidates =
        SupportedLanguage::parsing_candidates_for_file_path(&target_file.absolute_path);
    if parsing_candidates.is_empty() {
        return Ok(FileProcessingOutcome::UnsupportedLanguage);
    }
    let source_bytes = fs::read(&target_file.absolute_path)
        .with_context(|| format!("Cannot read {}", target_file.project_relative_path))?;
    let Ok(source_text) = String::from_utf8(source_bytes) else {
        return Ok(FileProcessingOutcome::Skipped {
            reason: "not valid UTF-8".to_owned(),
        });
    };
    let Some(parsed_source) =
        parse_with_first_language_without_syntax_errors(&source_text, parsing_candidates)?
    else {
        return Ok(FileProcessingOutcome::Skipped {
            reason: "syntax error before removal".to_owned(),
        });
    };
    let disallowed_items = find_disallowed_comments_and_docstrings(
        &source_text,
        &parsed_source,
        &target_file.project_relative_path,
        policy,
    );
    if should_remove_disallowed_items && !disallowed_items.is_empty() {
        let edited_source_text =
            remove_disallowed_items_from_source_text(&source_text, &disallowed_items);
        if parse_source_text(&edited_source_text, parsed_source.language)?
            .root_node()
            .has_error()
        {
            return Ok(FileProcessingOutcome::Skipped {
                reason: "removal would introduce a syntax error; file left unchanged".to_owned(),
            });
        }
        fs::write(&target_file.absolute_path, edited_source_text)
            .with_context(|| format!("Cannot write {}", target_file.project_relative_path))?;
    }
    Ok(FileProcessingOutcome::Processed { disallowed_items })
}

fn describe_item_kind(kind: DisallowedItemKind) -> &'static str {
    match kind {
        DisallowedItemKind::Comment => "comment",
        DisallowedItemKind::Docstring => "docstring",
    }
}

fn print_disallowed_items(project_relative_path: &str, disallowed_items: &[DisallowedItem]) {
    for item in disallowed_items {
        println!(
            "{}:{}:{}: {} not allowed: {}",
            project_relative_path,
            item.line_number,
            item.column_number,
            describe_item_kind(item.kind),
            item.source_text.lines().next().unwrap_or_default()
        );
    }
}

fn print_removed_item_counts(project_relative_path: &str, disallowed_items: &[DisallowedItem]) {
    let removed_comment_count = disallowed_items
        .iter()
        .filter(|item| item.kind == DisallowedItemKind::Comment)
        .count();
    let removed_docstring_count = disallowed_items.len() - removed_comment_count;
    println!(
        "{project_relative_path}: removed {removed_comment_count} comment(s) and {removed_docstring_count} docstring(s)"
    );
}

fn run_and_report_whether_every_file_is_clean(arguments: CommandLineArguments) -> Result<bool> {
    let project_root = arguments.project_root.canonicalize().with_context(|| {
        format!(
            "Project root not found: {}",
            arguments.project_root.display()
        )
    })?;
    let policy = SpecialCommentsPolicy::read_from_project_root(&project_root)?;
    let (target_file_arguments, should_remove_disallowed_items) = match arguments.requested_action {
        RequestedAction::Check(target_file_arguments) => (target_file_arguments, false),
        RequestedAction::Remove(target_file_arguments) => (target_file_arguments, true),
    };
    let target_files = list_target_files(
        &project_root,
        &target_file_arguments.into_target_file_selection(),
    )?;

    let mut summary = RunSummary::default();
    for target_file in &target_files {
        match check_or_remove_disallowed_items_in_file(
            target_file,
            &policy,
            should_remove_disallowed_items,
        )? {
            FileProcessingOutcome::UnsupportedLanguage => {
                if target_file.was_listed_explicitly {
                    eprintln!(
                        "{}: skipped: unsupported language",
                        target_file.project_relative_path
                    );
                    summary.skipped_file_count += 1;
                }
            }
            FileProcessingOutcome::Skipped { reason } => {
                eprintln!("{}: skipped: {reason}", target_file.project_relative_path);
                summary.skipped_file_count += 1;
            }
            FileProcessingOutcome::Processed { disallowed_items } => {
                summary.processed_file_count += 1;
                if disallowed_items.is_empty() {
                    continue;
                }
                summary.file_count_with_disallowed_items += 1;
                summary.disallowed_item_count += disallowed_items.len();
                if should_remove_disallowed_items {
                    print_removed_item_counts(
                        &target_file.project_relative_path,
                        &disallowed_items,
                    );
                } else {
                    print_disallowed_items(&target_file.project_relative_path, &disallowed_items);
                }
            }
        }
    }

    if should_remove_disallowed_items {
        println!(
            "Removed {} comment(s) or docstring(s) from {} of {} file(s)",
            summary.disallowed_item_count,
            summary.file_count_with_disallowed_items,
            summary.processed_file_count
        );
    } else if summary.disallowed_item_count > 0 {
        println!(
            "Found {} disallowed comment(s) or docstring(s) in {} of {} file(s)",
            summary.disallowed_item_count,
            summary.file_count_with_disallowed_items,
            summary.processed_file_count
        );
    } else {
        println!(
            "No disallowed comments or docstrings in {} file(s)",
            summary.processed_file_count
        );
    }
    if summary.skipped_file_count > 0 {
        eprintln!(
            "{} file(s) could not be processed",
            summary.skipped_file_count
        );
    }

    let has_unremoved_disallowed_items =
        !should_remove_disallowed_items && summary.disallowed_item_count > 0;
    Ok(summary.skipped_file_count == 0 && !has_unremoved_disallowed_items)
}

fn main() -> ExitCode {
    match run_and_report_whether_every_file_is_clean(CommandLineArguments::parse()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(error) => {
            eprintln!("ERROR: {error:#}");
            ExitCode::from(2)
        }
    }
}
