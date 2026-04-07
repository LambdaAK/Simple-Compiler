#![allow(dead_code)] // Built by the parser, typechecker, and tests.

//! Abstract syntax for a small C-shaped language: **`int`**, **`bool`**, **`char`**, **`struct Tag`**.
//!
//! Typing is enforced while lowering: `if` conditions must be **`bool`**, **`char`** promotes like C for `+`/`-`/comparisons with **`int`**.
//!
//! **Arrays** are fixed-size (`int a[10];`), stored as contiguous stack slots (one 8-byte word per element). **Arrays of struct are not supported.**
//!
//! **C strings on the stack:** `char s[] = "text";` — **`print_string("...")`** or **`print_string(s)`** prints characters then a newline.
//!
//! **`typedef`:** file-scope aliases, e.g. **`typedef int count_t;`**, **`typedef struct S S;`**, and array typedefs **`typedef int vec4[4];`** — names expand to canonical [`Ty`](Ty) in the AST (no separate alias type).
//!
//! **Structs:** `struct Point { int x; int y; };` then `struct Point p;` (**zero-initialized**), member access **`p.x`**, fields may be fixed arrays (`int xs[4];`), index with **`p.xs[i]`**. Assignment and scratch **`struct` arguments** (flattened to words). **Struct return types** are rejected for now.

/// Value types in the surface language.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Bool,
    Char,
    /// Named struct (`struct Tag` elsewhere).
    Struct(String),
    /// Fixed-size array: **`elem[len]`** in struct fields (see [`Stmt::ArrayDecl`](Stmt::ArrayDecl) for locals). Nested array types and arrays of struct are rejected in lowering.
    Array(Box<Ty>, usize),
    /// Pointer (**`T *`**); address is one machine word. **`void*`** is not supported.
    Ptr(Box<Ty>),
}

/// Function return type (**`void`** or a scalar). **Struct return is not implemented.**
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RetTy {
    Void,
    Scalar(Ty),
}

/// Top-level item: statement, prototype, function definition, or struct definition.
#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    Stmt(Stmt),
    /// `struct Tag { int x; ... };`
    StructDef {
        name: String,
        fields: Vec<(String, Ty)>,
    },
    /// `int foo(int a);` / `struct S` params allowed (flattened at call sites).
    FnDecl {
        name: String,
        params: Vec<(String, Ty)>,
        ret: RetTy,
    },
    FnDef {
        name: String,
        params: Vec<(String, Ty)>,
        ret: RetTy,
        body: Vec<Stmt>,
    },
}

/// A full program: ordered top-level [**`Item`**](Item)s.
#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `int x = expr;` or `struct S s;` — **`init`** is `None` only for uninitialized `struct`.
    VarDecl {
        name: String,
        ty: Ty,
        init: Option<Expr>,
    },
    ArrayDecl {
        name: String,
        elem_ty: Ty,
        len: usize,
    },
    CharArrayFromString {
        name: String,
        bytes: Vec<u8>,
    },
    ArrayFromExprs {
        name: String,
        elem_ty: Ty,
        elements: Vec<Expr>,
    },
    /// `target = value` — **`target`** is [`Expr::Var`], [`Expr::Index`], or [`Expr::Member`].
    Assign {
        target: Expr,
        value: Expr,
    },
    AddAssign {
        target: Expr,
        rhs: Expr,
    },
    PostInc {
        target: Expr,
    },
    If {
        cond: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        cond: Expr,
        body: Box<Stmt>,
    },
    For {
        init: Option<Box<Stmt>>,
        cond: Option<Expr>,
        step: Option<Box<Stmt>>,
        body: Box<Stmt>,
    },
    Block(Vec<Stmt>),
    Expr(Expr),
    Return(Option<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLit(i64),
    BoolLit(bool),
    CharLit(u8),
    StringLit(Vec<u8>),
    Var(String),
    Call {
        name: String,
        args: Vec<Expr>,
    },
    /// `base[index]` — **`base`** is an array variable or a struct member that is an array (`p.arr`).
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
    /// `a.b` / nested `a.b.c` — **`base`** is [`Expr::Var`], [`Expr::Index`], or another [`Expr::Member`].
    Member {
        base: Box<Expr>,
        field: String,
    },
    PostInc(Box<Expr>),
    Unary(UnaryOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg,
    Not,
    /// Address-of **`&x`** (operand must be an lvalue).
    Addr,
    /// Indirection **`*p`** (operand must be a pointer).
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}
