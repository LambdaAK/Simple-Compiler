pub mod ast;
pub mod codegen;
pub mod ir;
pub mod lexer;
pub mod lower;
pub mod parser;
pub mod token;

pub use codegen::emit_arm64;
pub use ir::{IrFunction, IrModule, IrProgram};
pub use lexer::LexError;
pub use lower::{build_fn_table, lower_program, LowerError};
pub use parser::{parse_program, FrontEndError, ParseError};
