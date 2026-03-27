#![allow(dead_code)] // Produced by lowering; used by codegen once wired up.

//! Three-address intermediate representation for a **single procedure** (e.g. `_main`).
//!
//! - Each instruction is one step; results go into a [`Temp`] or named variables via [`Instr::LoadVar`] / [`Instr::StoreVar`].
//! - AST **`bool`** lowers to an **`i64` word**: `0` or `1` (same representation as in registers).
//! - `And` / `Or` assume operands are already `0`/`1`; use them when expressions are side-effect-free,
//!   or lower `&&` / `||` with branches later for full C short-circuit semantics.

pub type Temp = String;

/// Linear sequence of instructions (no explicit basic-block graph yet).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IrProgram {
    pub instrs: Vec<Instr>,
}

impl IrProgram {
    pub fn new() -> Self {
        Self {
            instrs: Vec::new(),
        }
    }

    pub fn push(&mut self, instr: Instr) {
        self.instrs.push(instr);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    /// `dst = imm`
    Const(Temp, i64),

    // --- int arithmetic (signed) ---
    /// `dst = a + b`
    Add(Temp, Temp, Temp),
    /// `dst = a - b`
    Sub(Temp, Temp, Temp),
    /// `dst = a * b`
    Mul(Temp, Temp, Temp),
    /// `dst = a / b` (signed; division by zero is undefined)
    Div(Temp, Temp, Temp),

    /// `dst = -a`
    Neg(Temp, Temp),
    /// `dst = !a` with `a` in `{0, 1}` → result in `{1, 0}`
    Not(Temp, Temp),

    // --- comparisons: `dst` is `0` or `1` ---
    Eq(Temp, Temp, Temp),
    Ne(Temp, Temp, Temp),
    Lt(Temp, Temp, Temp),
    Le(Temp, Temp, Temp),
    Gt(Temp, Temp, Temp),
    Ge(Temp, Temp, Temp),

    /// `dst = a & b` (use for `&&` when operands are `0`/`1`)
    And(Temp, Temp, Temp),
    /// `dst = a | b` (use for `||` when operands are `0`/`1`)
    Or(Temp, Temp, Temp),

    /// `dst = *name` (load from named variable slot)
    LoadVar(Temp, String),
    /// `*name = src`
    StoreVar(String, Temp),

    // --- control flow ---
    Label(String),
    Jump(String),
    /// If `cond == 0`, jump to `label` (useful for branching on lowered `bool`).
    JumpIfZero(Temp, String),
}
