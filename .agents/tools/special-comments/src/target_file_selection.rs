use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

pub enum TargetFileSelection {
    StagedFiles,
    AllTrackedFiles,
    ExplicitFilePaths(Vec<PathBuf>),
}

pub struct TargetFile {
    pub project_relative_path: String,
    pub absolute_path: PathBuf,
    pub was_listed_explicitly: bool,
}

pub fn list_target_files(
    project_root: &Path,
    selection: &TargetFileSelection,
) -> Result<Vec<TargetFile>> {
    match selection {
        TargetFileSelection::StagedFiles => list_regular_files_reported_by_git(
            project_root,
            &[
                "diff",
                "--cached",
                "--name-only",
                "-z",
                "--relative",
                "--diff-filter=ACMR",
            ],
        ),
        TargetFileSelection::AllTrackedFiles => {
            list_regular_files_reported_by_git(project_root, &["ls-files", "-z"])
        }
        TargetFileSelection::ExplicitFilePaths(file_paths) => file_paths
            .iter()
            .map(|file_path| resolve_explicit_file_path(project_root, file_path))
            .collect(),
    }
}

fn list_regular_files_reported_by_git(
    project_root: &Path,
    git_arguments: &[&str],
) -> Result<Vec<TargetFile>> {
    let git_output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .args(git_arguments)
        .output()
        .context("Cannot run git")?;
    if !git_output.status.success() {
        bail!(
            "git {} failed: {}",
            git_arguments.join(" "),
            String::from_utf8_lossy(&git_output.stderr).trim()
        );
    }
    let listed_paths = String::from_utf8(git_output.stdout)
        .context("git listed a path that is not valid UTF-8")?;
    Ok(listed_paths
        .split('\0')
        .filter(|listed_path| !listed_path.is_empty())
        .map(|listed_path| TargetFile {
            project_relative_path: listed_path.to_owned(),
            absolute_path: project_root.join(listed_path),
            was_listed_explicitly: false,
        })
        .filter(|target_file| {
            target_file
                .absolute_path
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.is_file())
        })
        .collect())
}

fn resolve_explicit_file_path(project_root: &Path, file_path: &Path) -> Result<TargetFile> {
    let absolute_path = file_path
        .canonicalize()
        .with_context(|| format!("Cannot find {}", file_path.display()))?;
    if !absolute_path.is_file() {
        bail!(
            "Not a file: {}; use --all for every file tracked by Git",
            file_path.display()
        );
    }
    let project_relative_path = absolute_path
        .strip_prefix(project_root)
        .with_context(|| format!("Outside the project root: {}", file_path.display()))?
        .to_string_lossy()
        .replace('\\', "/");
    Ok(TargetFile {
        project_relative_path,
        absolute_path,
        was_listed_explicitly: true,
    })
}
