use std::ffi::OsStr;
use std::process::{Command, Output};

fn run(args: &[impl AsRef<OsStr>]) -> Output {
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

#[cfg(unix)]
#[test]
fn missing_file_diagnostics_preserve_each_unix_path() {
    use std::os::unix::ffi::OsStrExt;

    for path in [
        b"tests/fixtures/missing-\xff.mc",
        b"tests/fixtures/missing-\xfe.mc",
    ] {
        let output = run(&[OsStr::new("--tokens"), OsStr::from_bytes(path)]);
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            [path.as_slice(), b": error: Cannot open file.\n"].concat()
        );
    }
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
fn type_check_is_silent_on_success_and_reports_a_position_on_failure() {
    for path in [
        "examples/types.mc",
        "examples/scopes.mc",
        "examples/recursion.mc",
    ] {
        let output = run(&["--check", path]);
        assert_eq!(output.status.code(), Some(0), "{path}");
        assert!(output.stdout.is_empty() && output.stderr.is_empty());
    }
    let path = "examples/type_error.mc";
    for mode in ["--tokens", "--ast", "--resolve"] {
        assert_eq!(run(&[mode, path]).status.code(), Some(0));
    }
    let output = run(&["--check", path]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"examples/type_error.mc:3:16: error: Expected type bool. Received int.\n"
    );
}

#[test]
fn semantic_commands_require_one_file_and_propagate_earlier_errors() {
    for mode in ["--resolve", "--check"] {
        for args in [&[mode][..], &[mode, "file", "extra"]] {
            let output = run(args);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(
                output.stderr,
                format!("minic: error: Expected {mode} <file>. Use --help for usage.\n").as_bytes()
            );
        }
        for path in [
            "tests/fixtures/missing.mc",
            "tests/fixtures/syntax_error.mc",
            "tests/fixtures/lex_before_syntax.mc",
        ] {
            let expected = run(&["--ast", path]);
            let output = run(&[mode, path]);
            assert_eq!(output.status.code(), Some(1));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, expected.stderr);
        }
    }
}

#[test]
fn help_lists_analysis_commands() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            output.stdout,
            b"Usage: compiler-mini-rust --tokens <file>\n       compiler-mini-rust --ast <file>\n       compiler-mini-rust --resolve <file>\n       compiler-mini-rust --check <file>\n"
        );
        assert!(output.stderr.is_empty());
    }
}
