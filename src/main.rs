//! CLI: `compiler <path-to-source>` — parse, lower to IR, print instructions on stdout.

use std::env;
use std::fs;
use std::process::ExitCode;

use compiler::{lower_program, parse_program};

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

    for instr in &ir.instrs {
        println!("{instr:?}");
    }

    ExitCode::SUCCESS
}
