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
fn help_lists_the_token_command() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            output.stdout,
            b"Usage: compiler-mini-rust --tokens <file>\n"
        );
        assert!(output.stderr.is_empty());
    }
}
