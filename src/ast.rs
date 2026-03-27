#![allow(dead_code)] // Built by the parser, typechecker, and tests.

//! Abstract syntax for a small C-shaped language: **`int`**, **`bool`**, **`char`** (one byte / ASCII + escapes).
//!
//! Typing is enforced while lowering: `if` conditions must be **`bool`**, **`char`** promotes like C for `+`/`-`/comparisons with **`int`**.

/// Value types in the surface language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Bool,
    /// Single byte (values 0–255 in memory; use **`char`** literals or **`int`** literals 0–255 where allowed).
    Char,
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
    /// `name += expr;`
    AddAssign { name: String, rhs: Expr },
    /// `name++;`
    PostInc { name: String },
    /// `if (cond) ... [ else ... ]` — `cond` must have type `bool` (checked later).
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    /// `while (cond) stmt` — `cond` must be **`bool`**.
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    /// `for (init; cond; step) stmt` — **`init`** is a variable declaration or assignment (or omitted); **`cond`** is **`bool`** or omitted (infinite loop); **`step`** is an assignment or omitted.
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
    /// `{ stmt* }`
    Block(Vec<Stmt>),
    /// `print_int(expr);` — `expr` must be **`int`**.
    PrintInt { arg: Expr },
    /// `print_bool(expr);` — `expr` must be **`bool`**.
    PrintBool { arg: Expr },
    /// `print_char(expr);` — `expr` must be **`char`** (byte printed with `%c`).
    PrintChar { arg: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    BoolLit(bool),
    /// One byte (`'a'`, `'\n'`, …).
    CharLit(u8),
    Var(String),
    /// Postfix `i++` — operand must be **`int`** or **`char`**; value is the old value (**`int`**).
    PostInc(String),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    /// `-` on `int` or `char` (result **`int`**, C-style promotion)
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
