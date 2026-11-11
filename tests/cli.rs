use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_compiler-mini-rust"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap()
}

#[test]
fn examples_match_cpp_token_dumps() {
    for (path, expected) in [
        (
            "examples/return_42.mc",
            include_bytes!("fixtures/return_42.tokens").as_slice(),
        ),
        (
            "examples/hello.mc",
            include_bytes!("fixtures/hello.tokens").as_slice(),
        ),
        (
            "examples/lexer.mc",
            include_bytes!("fixtures/lexer.tokens").as_slice(),
        ),
        (
            "examples/arrays.mc",
            include_bytes!("fixtures/arrays.tokens").as_slice(),
        ),
    ] {
        let output = run(&["--tokens", path]);
        assert_eq!(output.status.code(), Some(0), "{path}");
        assert_eq!(output.stdout, expected, "{path}");
        assert!(output.stderr.is_empty(), "{path}");
    }
}

#[test]
fn lexical_errors_have_positions_and_no_partial_output() {
    for (path, message) in [
        (
            "tests/fixtures/invalid.mc",
            "2:3: error: Unexpected character \"@\".",
        ),
        (
            "tests/fixtures/unpaired_and.mc",
            "2:17: error: Unexpected character \"&\".",
        ),
        (
            "tests/fixtures/unterminated.mc",
            "2:3: error: Unterminated block comment.",
        ),
    ] {
        let output = run(&["--tokens", path]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("{path}:{message}\n").as_bytes());
    }
}

#[test]
fn missing_file_produces_a_file_diagnostic() {
    let output = run(&["--tokens", "tests/fixtures/missing.mc"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"tests/fixtures/missing.mc: error: Cannot open file.\n"
    );
}

#[test]
fn invalid_arguments_report_usage() {
    for args in [
        &[][..],
        &["--tokens"],
        &["--unknown", "file"],
        &["--tokens", "file", "extra"],
    ] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"minic: error: Expected --tokens <file>. Use --help for usage.\n"
        );
    }
}

#[test]
fn examples_match_cpp_syntax_trees() {
    for (path, expected) in [
        (
            "examples/hello.mc",
            include_bytes!("fixtures/hello.ast").as_slice(),
        ),
        (
            "examples/expressions.mc",
            include_bytes!("fixtures/expressions.ast").as_slice(),
        ),
        (
            "examples/calls.mc",
            include_bytes!("fixtures/calls.ast").as_slice(),
        ),
        (
            "examples/control_flow.mc",
            include_bytes!("fixtures/control_flow.ast").as_slice(),
        ),
    ] {
        let output = run(&["--ast", path]);
        assert_eq!(output.status.code(), Some(0), "{path}");
        assert_eq!(output.stdout, expected, "{path}");
        assert!(output.stderr.is_empty(), "{path}");
    }
}

#[test]
fn ast_reports_errors_without_a_partial_tree() {
    for (path, error) in [
        (
            "tests/fixtures/syntax_error.mc",
            "2:1: error: Expected \";\" after return value. Received RIGHT_BRACE.",
        ),
        (
            "tests/fixtures/lex_before_syntax.mc",
            "2:1: error: Unexpected character \"@\".",
        ),
    ] {
        let output = run(&["--ast", path]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, format!("{path}:{error}\n").as_bytes());
    }
    let tokens = run(&["--tokens", "tests/fixtures/syntax_error.mc"]);
    assert_eq!(tokens.status.code(), Some(0));
    assert!(tokens.stderr.is_empty());
}

#[test]
fn ast_requires_one_file() {
    for args in [&["--ast"][..], &["--ast", "file", "extra"]] {
        let output = run(args);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            b"minic: error: Expected --ast <file>. Use --help for usage.\n"
        );
    }
}

#[test]
fn resolution_matches_cpp_and_reports_the_first_name_error() {
    for (path, expected) in [
        (
            "examples/scopes.mc",
            include_bytes!("fixtures/scopes.resolution").as_slice(),
        ),
        (
            "examples/recursion.mc",
            include_bytes!("fixtures/recursion.resolution").as_slice(),
        ),
    ] {
        let output = run(&["--resolve", path]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, expected);
        assert!(output.stderr.is_empty());
    }
    let output = run(&["--resolve", "examples/expressions.mc"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"examples/expressions.mc:9:10: error: Undeclared name \"left\".\n"
    );
}

#[test]
fn help_lists_analysis_commands() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            output.stdout,
            b"Usage: compiler-mini-rust --tokens <file>\n       compiler-mini-rust --ast <file>\n       compiler-mini-rust --resolve <file>\n"
        );
        assert!(output.stderr.is_empty());
    }
}
