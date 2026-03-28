//! Three-address intermediate representation (**multi-procedure**: [`IrModule`]).
//!
//! - Each instruction is one step; results go into a [`Temp`] or named variables via [`Instr::LoadVar`] / [`Instr::StoreVar`].
//! - AST **`bool`** lowers to an **`i64` word**: `0` or `1` (same representation as in registers).
//! - **`char`** is a byte stored in the low 8 bits of an **`i64`** stack word (zero-extended).
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

/// One user function (**not** `_main`).
#[derive(Debug, Clone, PartialEq)]
pub struct IrFunction {
    /// Source name (codegen emits `_name` on macOS).
    pub name: String,
    pub instrs: Vec<Instr>,
}

/// `_main` plus user procedures.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IrModule {
    pub main: IrProgram,
    pub functions: Vec<IrFunction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instr {
    /// At function entry: copy incoming **AAPCS** argument register `x{0 + reg_idx}` into `slot` (**first 8** argument words).
    RecvParam(Temp, u8),
    /// Incoming argument word **past the registers**: copy from **`[fp, #16 + 8 * stack_word]`** into `slot` (matches our prologue: first stack word is **`fp+16`**).
    RecvParamStack(Temp, u32),

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
    /// `*name = imm` without a temp — keeps **`v0 .. v(n-1)`** consecutive so [`Instr::LoadIndex`] / [`Instr::StoreIndex`] stay valid.
    StoreVarImm(String, i64),

    /// `dst = first_slot[index]` — `first_slot` is the IR slot for element 0; elements are contiguous 8-byte words.
    LoadIndex(Temp, String, Temp, usize),
    /// `first_slot[index] = src`
    StoreIndex(String, Temp, Temp, usize),

    /// `dst = sp + offset(key)` where **`key`** names a stack slot (`v0`…); **`byte_off`** added to that slot’s offset (field / inner LEA).
    AddrLocal(Temp, String, usize),
    /// `dst = *ptr` (**64-bit** word loaded from address **`ptr`**).
    LoadMem(Temp, Temp),
    /// `*ptr = src` (**64-bit** word stored).
    StoreMem(Temp, Temp),

    // --- calls (`callee` is source name; codegen adds `_`) ---
    /// User function call; **`dst`** absent for **void** callees.
    Call {
        dst: Option<Temp>,
        callee: String,
        args: Vec<Temp>,
    },

    // --- host / libc I/O (lowering only; macOS uses `_printf`) ---
    /// Print one signed 64-bit line (`%lld\\n`).
    PrintInt(Temp),
    /// Print `true` or `false` plus newline (operand is `0` or `1`).
    PrintBool(Temp),
    /// `printf("%%c\\n", byte)` via the same Apple variadic stack convention as [`Instr::PrintInt`].
    PrintChar(Temp),
    /// `printf("%%c", byte)` — no trailing newline from the format string.
    PrintCharOnly(Temp),
    /// `printf("\\n")` — end a line (used after `print_string`'s character loop).
    PrintNl,

    // --- control flow ---
    Label(String),
    Jump(String),
    /// If `cond == 0`, jump to `label` (useful for branching on lowered `bool`).
    JumpIfZero(Temp, String),

    /// Return to caller; move **`src`** into **`x0`** when present.
    Ret(Option<Temp>),
}
