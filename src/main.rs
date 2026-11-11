use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use compiler_mini_rust::{Position, lex, parse, resolve_names};

const MODES: &[&str] = &["--tokens", "--ast", "--resolve"];

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let flag = args.next();
    let flag = flag.as_deref().and_then(OsStr::to_str);
    let path = args.next();
    if matches!(flag, Some("--help" | "-h")) && path.is_none() {
        println!("Usage: compiler-mini-rust --tokens <file>");
        for mode in &MODES[1..] {
            println!("       compiler-mini-rust {mode} <file>");
        }
        return Ok(());
    }
    let known = flag.filter(|f| MODES.contains(f));
    let (Some(mode), Some(path), None) = (known, path, args.next()) else {
        let expected = known.unwrap_or("--tokens");
        return Err(format!(
            "minic: error: Expected {expected} <file>. Use --help for usage."
        ));
    };
    let path = Path::new(&path);
    let mut file =
        File::open(path).map_err(|_| format!("{}: error: Cannot open file.", path.display()))?;
    let mut source = Vec::new();
    file.read_to_end(&mut source)
        .map_err(|_| format!("{}: error: Cannot read file.", path.display()))?;
    let diagnostic = |position: Position, error: &dyn std::fmt::Display| {
        format!("{}:{position}: error: {error}", path.display())
    };
    let tokens = lex(&source).map_err(|e| diagnostic(e.position, &e))?;

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
            )
            .map_err(|e| e.to_string())?;
        }
    } else {
        let program = parse(&tokens).map_err(|e| diagnostic(e.position, &e))?;
        if mode == "--resolve" {
            let resolution = resolve_names(&program).map_err(|e| diagnostic(e.position, &e))?;
            write!(output, "{resolution}").map_err(|e| e.to_string())?;
        } else {
            write!(output, "{program}").map_err(|e| e.to_string())?;
        }
    }
    output.flush().map_err(|e| e.to_string())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
