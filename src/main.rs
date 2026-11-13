use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use compiler_mini_rust::{
    Position, Value, check_types, compile_bytecode, execute_bytecode, interpret, lex,
    optimize_bytecode, parse, resolve_names,
};

const MODES: &[(&str, &str)] = &[
    (
        "--tokens",
        "Print token types, source text, and line:column positions.",
    ),
    ("--ast", "Use --ast to print the syntax tree."),
    (
        "--resolve",
        "Use --resolve to print scopes, symbols, and name bindings.",
    ),
    ("--check", "Use --check to analyze Mini-C types."),
    ("--bytecode", "Use --bytecode to print Mini-C instructions."),
    (
        "--bytecode-opt",
        "Use --bytecode-opt to print optimized Mini-C instructions.",
    ),
    ("--run", "Use --run to execute Mini-C programs."),
    (
        "--run-vm",
        "Use --run-vm to compile and execute Mini-C on the VM.",
    ),
    (
        "--run-vm-opt",
        "Use --run-vm-opt to optimize and execute Mini-C on the VM.",
    ),
];

enum CliError {
    Message(String),
    File {
        path: PathBuf,
        position: Option<Position>,
        message: String,
    },
}

impl CliError {
    fn write_to(&self, mut out: impl Write) -> io::Result<()> {
        match self {
            Self::Message(message) => writeln!(out, "{message}"),
            Self::File {
                path,
                position,
                message,
            } => {
                #[cfg(unix)]
                out.write_all(path.as_os_str().as_bytes())?;
                #[cfg(not(unix))]
                write!(out, "{}", path.display())?;
                if let Some(position) = position {
                    write!(out, ":{position}")?;
                }
                writeln!(out, ": error: {message}")
            }
        }
    }
}

impl From<io::Error> for CliError {
    fn from(error: io::Error) -> Self {
        Self::Message(error.to_string())
    }
}

fn run() -> Result<(), CliError> {
    let mut args = env::args_os().skip(1);
    let flag = args.next();
    let flag = flag.as_deref().and_then(OsStr::to_str);
    let path = args.next();
    if matches!(flag, Some("--help" | "-h")) && path.is_none() {
        let mut out = io::stdout().lock();
        writeln!(
            out,
            "minic: Mini-C educational compiler\n\nUsage:\n  minic --help"
        )?;
        for (mode, _) in MODES {
            writeln!(out, "  minic {mode} <file>")?;
        }
        writeln!(out)?;
        for (_, description) in MODES {
            writeln!(out, "{description}")?;
        }
        return Ok(());
    }
    let known = flag.filter(|flag| MODES.iter().any(|(mode, _)| mode == flag));
    let (Some(mode), Some(path), None) = (known, path, args.next()) else {
        let expected = known.unwrap_or("--tokens");
        return Err(CliError::Message(format!(
            "minic: error: Expected {expected} <file>. Use --help for usage."
        )));
    };
    let path = Path::new(&path);
    let diagnostic = |position, message| CliError::File {
        path: path.to_owned(),
        position,
        message,
    };
    let mut file = File::open(path).map_err(|_| diagnostic(None, "Cannot open file.".into()))?;
    let mut source = Vec::new();
    let read_error = |_| diagnostic(None, "Cannot read file.".into());
    if !file.metadata().map_err(read_error)?.is_dir() {
        file.read_to_end(&mut source).map_err(read_error)?;
    }
    let tokens = lex(&source).map_err(|e| diagnostic(Some(e.position), e.to_string()))?;

    let mut output = BufWriter::new(io::stdout().lock());
    if mode == "--tokens" {
        for token in tokens {
            writeln!(
                output,
                "{}:{} {} \"{}\"",
                token.position.line,
                token.position.column,
                token.kind.name(),
                token.text
            )?;
        }
    } else {
        let program = parse(&tokens).map_err(|e| diagnostic(Some(e.position), e.to_string()))?;
        let compile = || {
            let code = compile_bytecode(&program)
                .map_err(|error| diagnostic(Some(error.position), error.to_string()))?;
            Ok::<_, CliError>(if matches!(mode, "--bytecode-opt" | "--run-vm-opt") {
                optimize_bytecode(&code)
            } else {
                code
            })
        };
        match mode {
            "--run" | "--run-vm" | "--run-vm-opt" => {
                let value = if mode == "--run" {
                    interpret(&program, "main")
                        .map_err(|error| diagnostic(error.position(), error.to_string()))?
                } else {
                    execute_bytecode(&compile()?, "main")
                        .map_err(|error| diagnostic(error.position(), error.to_string()))?
                };
                if value != Value::Void {
                    writeln!(output, "{value}")?;
                }
            }
            "--bytecode" | "--bytecode-opt" => {
                write!(output, "{}", compile()?)?;
            }
            "--check" => {
                check_types(&program).map_err(|e| diagnostic(Some(e.position), e.to_string()))?;
            }
            "--resolve" => {
                let resolution = resolve_names(&program)
                    .map_err(|e| diagnostic(Some(e.position), e.to_string()))?;
                write!(output, "{resolution}")?;
            }
            _ => write!(output, "{program}")?,
        }
    }
    output.flush()?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = error.write_to(io::stderr().lock());
            ExitCode::FAILURE
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn positioned_diagnostic_preserves_the_unix_path() {
        let error = CliError::File {
            path: PathBuf::from(OsStr::from_bytes(b"invalid-\xff.mc")),
            position: Some(Position { line: 2, column: 3 }),
            message: "Unterminated block comment.".into(),
        };
        let mut output = Vec::new();
        error.write_to(&mut output).unwrap();
        assert_eq!(
            output,
            b"invalid-\xff.mc:2:3: error: Unterminated block comment.\n"
        );
    }
}
