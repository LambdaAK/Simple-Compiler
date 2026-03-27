#![allow(dead_code)] // Built by the parser, typechecker, and tests.

//! Abstract syntax for a small C-shaped language with **`int`** and **`bool`** only.
//!
//! Typing is enforced in a later pass: e.g. `if` conditions must be **`bool`**, arithmetic only on **`int`**, etc.

/// Value types in the surface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Bool,
}

/// A full program: a flat list of statements (blocks nest inside statements).
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub stmts: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `int x = expr;` / `bool b = expr;`
    VarDecl {
        name: String,
        ty: Ty,
        init: Expr,
    },
    /// `name = expr;`
    Assign { name: String, value: Expr },
    /// `if (cond) ... [ else ... ]` — `cond` must have type `bool` (checked later).
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    /// `{ stmt* }`
    Block(Vec<Stmt>),
    /// `print_int(expr);` — `expr` must be **`int`**.
    PrintInt { arg: Expr },
    /// `print_bool(expr);` — `expr` must be **`bool`**.
    PrintBool { arg: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    BoolLit(bool),
    Var(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// `-` on `int`
    Neg,
    /// `!` on `bool`
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    /// `+` — `int`, `int` → `int`
    Add,
    /// `-` — `int`, `int` → `int`
    Sub,
    /// `*` — `int`, `int` → `int`
    Mul,
    /// `/` — `int`, `int` → `int`
    Div,

    /// `==` — `int`/`bool` pairs must match (checked later)
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,

    /// `&&` — `bool`, `bool` → `bool`
    And,
    /// `||` — `bool`, `bool` → `bool`
    Or,
}
