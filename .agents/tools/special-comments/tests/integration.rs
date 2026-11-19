use std::fs;
use std::process::{Command, Output};
use tempfile::TempDir;

fn create_project_with_configuration(configuration_text: &str) -> TempDir {
    let project = tempfile::tempdir().unwrap();
    fs::write(
        project.path().join("special-comments.toml"),
        configuration_text,
    )
    .unwrap();
    project
}

fn write_project_file(project: &TempDir, project_relative_path: &str, contents: &str) {
    let file_path = project.path().join(project_relative_path);
    fs::create_dir_all(file_path.parent().unwrap()).unwrap();
    fs::write(file_path, contents).unwrap();
}

fn read_project_file(project: &TempDir, project_relative_path: &str) -> String {
    fs::read_to_string(project.path().join(project_relative_path)).unwrap()
}

fn run_special_comments(project: &TempDir, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_special-comments"))
        .current_dir(project.path())
        .args(arguments)
        .output()
        .unwrap()
}

fn run_git(project: &TempDir, arguments: &[&str]) {
    let status = Command::new("git")
        .current_dir(project.path())
        .args(arguments)
        .status()
        .unwrap();
    assert!(status.success());
}

fn standard_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn standard_error(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn removes_python_comments_and_docstrings_and_keeps_bodies_nonempty() {
    let project = create_project_with_configuration("");
    write_project_file(
        &project,
        "example.py",
        concat!(
            "\"\"\"Module docstring.\"\"\"\n",
            "import os  # trailing explanation\n",
            "\n",
            "\n",
            "# whole line explanation\n",
            "def documented_function() -> int:\n",
            "    \"\"\"Function docstring.\"\"\"\n",
            "    return 1\n",
            "\n",
            "\n",
            "class OnlyDocumented:\n",
            "    \"\"\"Only a docstring.\"\"\"\n",
            "\n",
            "\n",
            "def text_with_hash() -> str:\n",
            "    return \"# not a comment\"\n",
        ),
    );

    let output = run_special_comments(&project, &["remove", "example.py"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "example.py"),
        concat!(
            "import os\n",
            "\n",
            "\n",
            "def documented_function() -> int:\n",
            "    return 1\n",
            "\n",
            "\n",
            "class OnlyDocumented:\n",
            "    ...\n",
            "\n",
            "\n",
            "def text_with_hash() -> str:\n",
            "    return \"# not a comment\"\n",
        )
    );
}

#[test]
fn keeps_comments_matching_patterns_and_star_does_not_match_whitespace() {
    let project = create_project_with_configuration(concat!(
        "[[allowed_comments]]\n",
        "files = [\"**/*.py\"]\n",
        "patterns = [\"# type: ignore[*]\"]\n",
    ));
    write_project_file(
        &project,
        "values.py",
        concat!(
            "first_value: int = \"text\"  # type: ignore[assignment]\n",
            "second_value: int = \"text\"  # type: ignore[assignment] because of reasons\n",
        ),
    );

    let check_output = run_special_comments(&project, &["check", "values.py"]);

    assert_eq!(check_output.status.code(), Some(1));
    assert!(standard_output(&check_output).contains("values.py:2:29: comment not allowed"));
    assert!(!standard_output(&check_output).contains("values.py:1:"));

    let remove_output = run_special_comments(&project, &["remove", "values.py"]);

    assert!(
        remove_output.status.success(),
        "{}",
        standard_error(&remove_output)
    );
    assert_eq!(
        read_project_file(&project, "values.py"),
        concat!(
            "first_value: int = \"text\"  # type: ignore[assignment]\n",
            "second_value: int = \"text\"\n",
        )
    );
}

#[test]
fn file_patterns_do_not_let_a_single_star_cross_directories() {
    let project = create_project_with_configuration(concat!(
        "[[allowed_comments]]\n",
        "files = [\"src/*.py\"]\n",
        "patterns = [\"# keep\"]\n",
    ));
    write_project_file(&project, "src/direct.py", "value = 1  # keep\n");
    write_project_file(&project, "src/nested/deeper.py", "value = 1  # keep\n");

    let output = run_special_comments(
        &project,
        &["remove", "src/direct.py", "src/nested/deeper.py"],
    );

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "src/direct.py"),
        "value = 1  # keep\n"
    );
    assert_eq!(
        read_project_file(&project, "src/nested/deeper.py"),
        "value = 1\n"
    );
}

#[test]
fn removes_c_comments_without_touching_preprocessor_directives_or_strings() {
    let project = create_project_with_configuration("");
    write_project_file(
        &project,
        "example.c",
        concat!(
            "#include <stdio.h> /* trailing block */\n",
            "// whole line\n",
            "int/**/value = 1;\n",
            "const char *text = \"/* not a comment */\";\n",
        ),
    );

    let output = run_special_comments(&project, &["remove", "example.c"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "example.c"),
        concat!(
            "#include <stdio.h>\n",
            "int value = 1;\n",
            "const char *text = \"/* not a comment */\";\n",
        )
    );
}

#[test]
fn removes_cpp_comments() {
    let project = create_project_with_configuration("");
    write_project_file(
        &project,
        "example.hpp",
        concat!(
            "#pragma once\n",
            "// whole line\n",
            "namespace example {\n",
            "int twice(int value); // trailing\n",
            "}\n",
        ),
    );

    let output = run_special_comments(&project, &["remove", "example.hpp"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "example.hpp"),
        concat!(
            "#pragma once\n",
            "namespace example {\n",
            "int twice(int value);\n",
            "}\n",
        )
    );
}

#[test]
fn removes_rust_comments_and_doc_comments_but_keeps_attributes() {
    let project = create_project_with_configuration("");
    write_project_file(
        &project,
        "example.rs",
        concat!(
            "//! Crate documentation.\n",
            "/// Item documentation.\n",
            "#[derive(Debug)]\n",
            "struct Thing; // trailing\n",
            "\n",
            "fn main() {\n",
            "    let text = \"// not a comment\";\n",
            "    /* leading block */ let number = 1;\n",
            "}\n",
        ),
    );

    let output = run_special_comments(&project, &["remove", "example.rs"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "example.rs"),
        concat!(
            "#[derive(Debug)]\n",
            "struct Thing;\n",
            "\n",
            "fn main() {\n",
            "    let text = \"// not a comment\";\n",
            "    let number = 1;\n",
            "}\n",
        )
    );
}

#[test]
fn keeps_docstrings_in_files_allowed_to_have_them() {
    let project = create_project_with_configuration(concat!(
        "[[allowed_docstrings]]\n",
        "files = [\"cli/**/*.py\"]\n",
    ));
    write_project_file(
        &project,
        "cli/commands.py",
        concat!(
            "def run() -> None:\n",
            "    \"\"\"Show the command help.\"\"\"\n",
            "    print(\"run\")  # explanation\n",
        ),
    );

    let output = run_special_comments(&project, &["remove", "cli/commands.py"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert_eq!(
        read_project_file(&project, "cli/commands.py"),
        concat!(
            "def run() -> None:\n",
            "    \"\"\"Show the command help.\"\"\"\n",
            "    print(\"run\")\n",
        )
    );
}

#[test]
fn check_succeeds_when_no_disallowed_comments_remain() {
    let project = create_project_with_configuration("");
    write_project_file(&project, "clean.py", "value = 1\n");

    let output = run_special_comments(&project, &["check", "clean.py"]);

    assert!(output.status.success(), "{}", standard_error(&output));
    assert!(standard_output(&output).contains("No disallowed comments or docstrings in 1 file(s)"));
}

#[test]
fn staged_selection_only_processes_files_in_the_git_index() {
    let project = create_project_with_configuration("");
    run_git(&project, &["init", "--quiet"]);
    write_project_file(&project, "staged.py", "value = 1  # staged\n");
    write_project_file(&project, "unstaged.py", "value = 1  # unstaged\n");
    run_git(&project, &["add", "staged.py"]);

    let output = run_special_comments(&project, &["check", "--staged"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_output(&output).contains("staged.py:1:12: comment not allowed: # staged"));
    assert!(!standard_output(&output).contains("unstaged.py"));
}

#[test]
fn leaves_files_with_syntax_errors_unchanged() {
    let project = create_project_with_configuration("");
    let broken_source = "def broken(:\n    return 1  # comment\n";
    write_project_file(&project, "broken.py", broken_source);

    let output = run_special_comments(&project, &["remove", "broken.py"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_error(&output).contains("broken.py: skipped: syntax error before removal"));
    assert_eq!(read_project_file(&project, "broken.py"), broken_source);
}

#[test]
fn leaves_files_unchanged_when_removal_would_break_the_syntax() {
    let project = create_project_with_configuration("");
    let source = "\"\"\"docstring\"\"\"; other = 1\n";
    write_project_file(&project, "joined.py", source);

    let output = run_special_comments(&project, &["remove", "joined.py"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_error(&output).contains("removal would introduce a syntax error"));
    assert_eq!(read_project_file(&project, "joined.py"), source);
}

#[test]
fn reports_explicit_files_in_unsupported_languages() {
    let project = create_project_with_configuration("");
    write_project_file(&project, "notes.md", "# heading\n");

    let output = run_special_comments(&project, &["check", "notes.md"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(standard_error(&output).contains("notes.md: skipped: unsupported language"));
}

#[test]
fn fails_without_a_configuration_file() {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("example.py"), "value = 1\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_special-comments"))
        .current_dir(project.path())
        .args(["check", "example.py"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(standard_error(&output).contains("special-comments.toml"));
}

#[test]
fn rejects_unknown_configuration_keys() {
    let project = create_project_with_configuration("allowed_everything = true\n");
    write_project_file(&project, "example.py", "value = 1\n");

    let output = run_special_comments(&project, &["check", "example.py"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(standard_error(&output).contains("Invalid"));
}
