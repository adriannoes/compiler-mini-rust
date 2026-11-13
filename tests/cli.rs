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
fn analysis_and_run_commands_require_one_file_and_propagate_earlier_errors() {
    for mode in [
        "--resolve",
        "--check",
        "--run",
        "--bytecode",
        "--run-vm",
        "--bytecode-opt",
        "--run-vm-opt",
    ] {
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
fn run_prints_the_entry_value_and_exits_successfully() {
    for (path, expected) in [
        ("examples/return_42.mc", "42\n"),
        ("examples/short_circuit.mc", "true\n"),
        ("examples/calls.mc", "6\n"),
        ("examples/factorial.mc", "720\n"),
        ("examples/fibonacci.mc", "55\n"),
        ("tests/fixtures/void_main.mc", ""),
    ] {
        let output = run(&["--run", path]);
        assert_eq!(output.status.code(), Some(0), "{path}");
        assert_eq!(output.stdout, expected.as_bytes(), "{path}");
        assert!(output.stderr.is_empty(), "{path}");
    }
}

#[test]
fn run_reports_static_entry_and_runtime_errors_without_output() {
    for (path, diagnostic) in [
        (
            "examples/control_flow.mc",
            "1:1: error: The entry function \"main\" does not exist.",
        ),
        (
            "examples/runtime_error.mc",
            "3:15: error: The variable \"value\" has no value.",
        ),
        (
            "examples/expressions.mc",
            "9:10: error: Undeclared name \"left\".",
        ),
        (
            "examples/type_error.mc",
            "3:16: error: Expected type bool. Received int.",
        ),
    ] {
        let output = run(&["--run", path]);
        assert_eq!(output.status.code(), Some(1), "{path}");
        assert!(output.stdout.is_empty(), "{path}");
        assert_eq!(output.stderr, format!("{path}:{diagnostic}\n").as_bytes());
    }
    let check = run(&["--check", "examples/runtime_error.mc"]);
    assert_eq!(check.status.code(), Some(0));
    assert!(check.stdout.is_empty() && check.stderr.is_empty());
}

#[test]
fn bytecode_prints_the_reference_listings() {
    for (name, expected) in [
        (
            "hello",
            include_bytes!("fixtures/hello.bytecode").as_slice(),
        ),
        (
            "calls",
            include_bytes!("fixtures/calls.bytecode").as_slice(),
        ),
        (
            "control_flow",
            include_bytes!("fixtures/control_flow.bytecode").as_slice(),
        ),
        (
            "short_circuit",
            include_bytes!("fixtures/short_circuit.bytecode").as_slice(),
        ),
        (
            "runtime_error",
            include_bytes!("fixtures/runtime_error.bytecode").as_slice(),
        ),
        (
            "factorial",
            include_bytes!("fixtures/factorial.bytecode").as_slice(),
        ),
    ] {
        let path = format!("examples/{name}.mc");
        let output = run(&["--bytecode", &path]);
        assert_eq!(output.status.code(), Some(0), "{path}");
        assert_eq!(output.stdout, expected, "{path}");
        assert!(output.stderr.is_empty(), "{path}");
    }
    for path in ["examples/expressions.mc", "examples/type_error.mc"] {
        let check = run(&["--check", path]);
        let dump = run(&["--bytecode", path]);
        assert_eq!(dump.status.code(), Some(1));
        assert!(dump.stdout.is_empty());
        assert_eq!(dump.stderr, check.stderr);
    }
}

#[test]
fn vm_command_matches_run_output_errors_and_status() {
    for path in [
        "examples/return_42.mc",
        "examples/calls.mc",
        "examples/factorial.mc",
        "examples/fibonacci.mc",
        "examples/runtime_error.mc",
        "examples/control_flow.mc",
        "examples/short_circuit.mc",
        "tests/fixtures/void_main.mc",
        "examples/type_error.mc",
        "examples/expressions.mc",
    ] {
        let expected = run(&["--run", path]);
        let actual = run(&["--run-vm", path]);
        assert_eq!(actual.status.code(), expected.status.code(), "{path}");
        assert_eq!(actual.stdout, expected.stdout, "{path}");
        assert_eq!(actual.stderr, expected.stderr, "{path}");
    }
}

#[test]
fn help_lists_available_commands() {
    for flag in ["--help", "-h"] {
        let output = run(&[flag]);
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(
            output.stdout,
            b"Usage: compiler-mini-rust --tokens <file>\n       compiler-mini-rust --ast <file>\n       compiler-mini-rust --resolve <file>\n       compiler-mini-rust --check <file>\n       compiler-mini-rust --run <file>\n       compiler-mini-rust --bytecode <file>\n       compiler-mini-rust --run-vm <file>\n       compiler-mini-rust --bytecode-opt <file>\n       compiler-mini-rust --run-vm-opt <file>\n"
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn optimized_listings_match_reference_files() {
    for name in [
        "hello",
        "calls",
        "control_flow",
        "short_circuit",
        "runtime_error",
        "factorial",
        "optimization",
    ] {
        let path = format!("examples/{name}.mc");
        let output = run(&["--bytecode-opt", &path]);
        assert!(output.status.success(), "{path}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            std::fs::read(format!("tests/fixtures/{name}.optimized.bytecode")).unwrap(),
            "{path}"
        );
    }
    let path = "examples/optimization.mc";
    assert_eq!(
        run(&["--bytecode", path]).stdout,
        include_bytes!("fixtures/optimization.bytecode")
    );
    for mode in ["--run", "--run-vm", "--run-vm-opt"] {
        let output = run(&[mode, path]);
        assert!(output.status.success());
        assert_eq!(output.stdout, b"24\n");
        assert!(output.stderr.is_empty());
    }
}
