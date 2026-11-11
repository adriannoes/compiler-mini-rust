use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Read, Write};
use std::path::Path;
use std::process::ExitCode;

use compiler_mini_rust::{lex, parse};

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let (ast, path) = match (args.next(), args.next(), args.next()) {
        (Some(mode), None, None) if mode == "--help" || mode == "-h" => {
            println!(
                "Usage: compiler-mini-rust --tokens <file>\n       compiler-mini-rust --ast <file>"
            );
            return Ok(());
        }
        (Some(mode), Some(path), None) if mode == "--tokens" => (false, path),
        (Some(mode), Some(path), None) if mode == "--ast" => (true, path),
        (Some(mode), _, _) if mode == "--ast" => {
            return Err("minic: error: Expected --ast <file>. Use --help for usage.".to_owned());
        }
        _ => {
            return Err("minic: error: Expected --tokens <file>. Use --help for usage.".to_owned());
        }
    };
    let path = Path::new(&path);
    let mut file =
        File::open(path).map_err(|_| format!("{}: error: Cannot open file.", path.display()))?;
    let mut source = Vec::new();
    file.read_to_end(&mut source)
        .map_err(|_| format!("{}: error: Cannot read file.", path.display()))?;
    let tokens = lex(&source).map_err(|e| {
        format!(
            "{}:{}:{}: error: {e}",
            path.display(),
            e.position.line,
            e.position.column
        )
    })?;

    let mut output = BufWriter::new(io::stdout().lock());
    if ast {
        let program = parse(&tokens).map_err(|e| {
            format!(
                "{}:{}:{}: error: {e}",
                path.display(),
                e.position.line,
                e.position.column
            )
        })?;
        write!(output, "{program}").map_err(|e| e.to_string())?;
    } else {
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
