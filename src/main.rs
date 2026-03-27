//! CLI: `compiler <path-to-source>` — parse, lower to an `IrModule`, emit ARM64 `output.s` in cwd.

use std::env;
use std::fs;
use std::process::ExitCode;

use compiler::{emit_arm64, lower_program, parse_program};

fn main() -> ExitCode {
    let mut args = env::args_os();
    args.next(); // skip executable name

    let Some(path) = args.next() else {
        eprintln!("usage: compiler <path-to-source-file>");
        return ExitCode::from(1);
    };

    let src = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}: {e}", path.to_string_lossy());
            return ExitCode::from(1);
        }
    };

    let ast = match parse_program(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("parse error: {e:?}");
            return ExitCode::from(1);
        }
    };

    let ir = match lower_program(&ast) {
        Ok(ir) => ir,
        Err(e) => {
            eprintln!("lower error: {e:?}");
            return ExitCode::from(1);
        }
    };

    let asm = emit_arm64(&ir);
    let out = "output.s";
    if let Err(e) = fs::write(out, asm) {
        eprintln!("{out}: {e}");
        return ExitCode::from(1);
    }
    eprintln!("wrote {out} — run: clang {out} -o program && ./program");

    ExitCode::SUCCESS
}
