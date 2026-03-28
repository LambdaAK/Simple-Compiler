//! Lower [`crate::ast::Program`] to [`crate::ir::IrModule`] (three-address code, `_main` + user functions).

use std::collections::{HashMap, HashSet};

use crate::ast::{BinOp, Expr, Item, Program, RetTy, Stmt, Ty, UnaryOp};
use crate::ir::{Instr, IrFunction, IrModule, IrProgram};

/// Max total argument **words** (each `int`/`bool`/`char` is 1 word; structs count their fields).
const MAX_ARG_WORDS: usize = 8;

/// `print_*` and user functions share this name list for resolution.
pub const BUILTIN_PRINT_INT: &str = "print_int";
pub const BUILTIN_PRINT_BOOL: &str = "print_bool";
pub const BUILTIN_PRINT_CHAR: &str = "print_char";
pub const BUILTIN_PRINT_STRING: &str = "print_string";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FnSig {
    pub params: Vec<Ty>,
    pub ret: RetTy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    UndefinedVariable(String),
    TypeMismatch(String),
    DuplicateFunction(String),
    ConflictingPrototype(String),
    UndefinedFunction(String),
    ForwardDeclWithoutDefinition(String),
    ReturnOutsideFunction,
    BadReturn(String),
    ParameterShadowsFunction(String),
    DuplicateStruct(String),
    UnknownStruct(String),
    RecursiveStruct(String),
    StructReturnNotSupported(String),
}

#[derive(Debug, Clone)]
pub struct StructLayout {
    pub size_words: usize,
    pub fields: Vec<(String, Ty, usize)>,
}

#[derive(Debug, Clone)]
enum Binding {
    Scalar { slot: String, ty: Ty },
    /// Contiguous stack slots; `first_slot` is element 0.
    Array {
        first_slot: String,
        elem: Ty,
        len: usize,
    },
    /// Struct instance: `first_slot` is `v{n}` for word 0 of the object.
    Struct {
        tag: String,
        first_slot: String,
        #[allow(dead_code)]
        size_words: usize,
    },
}

struct LowerCtx<'a> {
    ir: IrProgram,
    scopes: Vec<HashMap<String, Binding>>,
    temp_id: usize,
    label_id: usize,
    slot_id: usize,
    fn_sigs: &'a HashMap<String, FnSig>,
    struct_layouts: &'a HashMap<String, StructLayout>,
    /// Prefix so labels are unique across functions (`main`, user name).
    label_prefix: String,
    /// `None` while lowering top-level script (`return` forbidden).
    current_ret: Option<RetTy>,
}

impl<'a> LowerCtx<'a> {
    fn new_main(fn_sigs: &'a HashMap<String, FnSig>, struct_layouts: &'a HashMap<String, StructLayout>) -> Self {
        Self {
            ir: IrProgram::new(),
            scopes: vec![HashMap::new()],
            temp_id: 0,
            label_id: 0,
            slot_id: 0,
            fn_sigs,
            struct_layouts,
            label_prefix: "main".into(),
            current_ret: None,
        }
    }

    fn new_function(
        fn_sigs: &'a HashMap<String, FnSig>,
        struct_layouts: &'a HashMap<String, StructLayout>,
        name: &str,
        ret: RetTy,
        params: &[(String, Ty)],
    ) -> Result<Self, LowerError> {
        let total_param_words: usize = params
            .iter()
            .map(|(_, t)| ty_size_words(t, struct_layouts))
            .sum::<Result<usize, LowerError>>()?;
        let mut reg_cursor: u8 = 0;
        let mut slot_cursor: usize = 0;
        let mut cx = Self {
            ir: IrProgram::new(),
            scopes: vec![HashMap::new()],
            temp_id: 0,
            label_id: 0,
            slot_id: total_param_words,
            fn_sigs,
            struct_layouts,
            label_prefix: name.to_string(),
            current_ret: Some(ret),
        };
        for (pname, ty) in params {
            let words = ty_size_words(ty, struct_layouts)?;
            if reg_cursor as usize + words > MAX_ARG_WORDS {
                return Err(LowerError::TypeMismatch(format!(
                    "function `{name}`: arguments use more than {MAX_ARG_WORDS} words"
                )));
            }
            match ty {
                Ty::Int | Ty::Bool | Ty::Char => {
                    let slot = format!("v{}", slot_cursor);
                    cx.scopes
                        .last_mut()
                        .expect("lower: root scope")
                        .insert(
                            pname.clone(),
                            Binding::Scalar {
                                slot: slot.clone(),
                                ty: ty.clone(),
                            },
                        );
                    cx.ir.push(Instr::RecvParam(slot, reg_cursor));
                    reg_cursor += 1;
                    slot_cursor += 1;
                }
                Ty::Array(..) => {
                    return Err(LowerError::TypeMismatch(format!(
                        "function `{name}`: array parameters are not supported"
                    )));
                }
                Ty::Struct(tag) => {
                    let slot = format!("v{}", slot_cursor);
                    cx.scopes
                        .last_mut()
                        .expect("lower: root scope")
                        .insert(
                            pname.clone(),
                            Binding::Struct {
                                tag: tag.clone(),
                                first_slot: slot.clone(),
                                size_words: words,
                            },
                        );
                    for j in 0..words {
                        cx.ir.push(Instr::RecvParam(
                            format!("v{}", slot_cursor + j),
                            reg_cursor + j as u8,
                        ));
                    }
                    reg_cursor += words as u8;
                    slot_cursor += words;
                }
            }
        }
        Ok(cx)
    }

    fn finish_function(&mut self, ret: RetTy) -> Result<(), LowerError> {
        let ends_with_ret = self
            .ir
            .instrs
            .last()
            .is_some_and(|i| matches!(i, Instr::Ret(_)));
        if ends_with_ret {
            return Ok(());
        }
        match ret {
            RetTy::Void => self.ir.push(Instr::Ret(None)),
            RetTy::Scalar(_) => {
                return Err(LowerError::TypeMismatch(format!(
                    "function `{}` may fall off without returning a value",
                    self.label_prefix
                )));
            }
        }
        Ok(())
    }

    fn fresh_temp(&mut self) -> String {
        let t = format!("{}_t{}", self.label_prefix, self.temp_id);
        self.temp_id += 1;
        t
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let l = format!("{}.{}.{}", self.label_prefix, prefix, self.label_id);
        self.label_id += 1;
        l
    }

    fn declare_struct(&mut self, source_name: &str, tag: &str) -> Result<(), LowerError> {
        if self.fn_sigs.contains_key(source_name) {
            return Err(LowerError::ParameterShadowsFunction(source_name.to_string()));
        }
        let layout = self
            .struct_layouts
            .get(tag)
            .ok_or_else(|| LowerError::UnknownStruct(tag.to_string()))?;
        let start = self.slot_id;
        let n = layout.size_words;
        self.slot_id += n;
        for i in 0..n {
            self
                .ir
                .push(Instr::StoreVarImm(format!("v{}", start + i), 0));
        }
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(
                source_name.to_string(),
                Binding::Struct {
                    tag: tag.to_string(),
                    first_slot: format!("v{}", start),
                    size_words: n,
                },
            );
        Ok(())
    }

    fn declare(&mut self, source_name: &str, ty: Ty) -> Result<String, LowerError> {
        if matches!(ty, Ty::Struct(_)) {
            return Err(LowerError::TypeMismatch(
                "internal: use `declare_struct` for struct variables".into(),
            ));
        }
        if self.fn_sigs.contains_key(source_name) {
            return Err(LowerError::ParameterShadowsFunction(source_name.to_string()));
        }
        let slot = format!("v{}", self.slot_id);
        self.slot_id += 1;
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(
                source_name.to_string(),
                Binding::Scalar {
                    slot: slot.clone(),
                    ty,
                },
            );
        Ok(slot)
    }

    fn declare_array(&mut self, source_name: &str, elem: Ty, len: usize) -> Result<(), LowerError> {
        if matches!(elem, Ty::Struct(_)) {
            return Err(LowerError::TypeMismatch(
                "arrays of `struct` are not supported".into(),
            ));
        }
        if self.fn_sigs.contains_key(source_name) {
            return Err(LowerError::ParameterShadowsFunction(source_name.to_string()));
        }
        let start = self.slot_id;
        self.slot_id += len;
        let first_slot = format!("v{}", start);
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(
                source_name.to_string(),
                Binding::Array {
                    first_slot: first_slot.clone(),
                    elem,
                    len,
                },
            );
        for i in 0..len {
            let slot = format!("v{}", start + i);
            self.ir.push(Instr::StoreVarImm(slot, 0));
        }
        Ok(())
    }

    /// `char s[] = "...";` — **bytes** includes the trailing **`0`** (NUL).
    fn declare_char_array_from_bytes(
        &mut self,
        source_name: &str,
        bytes: &[u8],
    ) -> Result<(), LowerError> {
        if self.fn_sigs.contains_key(source_name) {
            return Err(LowerError::ParameterShadowsFunction(source_name.to_string()));
        }
        let len = bytes.len();
        let start = self.slot_id;
        self.slot_id += len;
        let first_slot = format!("v{}", start);
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(
                source_name.to_string(),
                Binding::Array {
                    first_slot: first_slot.clone(),
                    elem: Ty::Char,
                    len,
                },
            );
        for (i, b) in bytes.iter().enumerate() {
            self
                .ir
                .push(Instr::StoreVarImm(format!("v{}", start + i), i64::from(*b)));
        }
        Ok(())
    }

    /// Anonymous NUL-terminated bytes on the stack (for string literals). Returns first slot id.
    fn emit_stack_c_string(&mut self, bytes_including_nul: &[u8]) -> String {
        let start = self.slot_id;
        let len = bytes_including_nul.len();
        self.slot_id += len;
        let first = format!("v{}", start);
        for (i, b) in bytes_including_nul.iter().enumerate() {
            self.ir.push(Instr::StoreVarImm(
                format!("v{}", start + i),
                i64::from(*b),
            ));
        }
        first
    }

    /// `print_string`: one stack word per character; walk until NUL (then one `\\n`).
    fn lower_print_string_chars(&mut self, first_slot: &str, len: usize) {
        let end_l = self.fresh_label("print_string_end");
        for i in 0..len {
            let idx_t = self.fresh_temp();
            self.ir.push(Instr::Const(idx_t.clone(), i as i64));
            let ch_t = self.fresh_temp();
            self.ir.push(Instr::LoadIndex(
                ch_t.clone(),
                first_slot.to_string(),
                idx_t,
                len,
            ));
            self.ir.push(Instr::JumpIfZero(ch_t.clone(), end_l.clone()));
            self.ir.push(Instr::PrintCharOnly(ch_t));
        }
        self.ir.push(Instr::Label(end_l));
        self.ir.push(Instr::PrintNl);
    }

    fn resolve(&self, source_name: &str) -> Result<Binding, LowerError> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.get(source_name) {
                return Ok(b.clone());
            }
        }
        Err(LowerError::UndefinedVariable(source_name.to_string()))
    }

    fn resolve_array(&self, source_name: &str) -> Result<(String, Ty, usize), LowerError> {
        match self.resolve(source_name)? {
            Binding::Array {
                first_slot,
                elem,
                len,
            } => Ok((first_slot, elem, len)),
            Binding::Scalar { .. } | Binding::Struct { .. } => Err(LowerError::TypeMismatch(format!(
                "`{source_name}` is not an array"
            ))),
        }
    }

    /// Address of an array lvalue: local **`[]`** binding or **`struct`** member of array type.
    fn resolve_array_expr(&self, base: &Expr) -> Result<(String, Ty, usize), LowerError> {
        match base {
            Expr::Var(name) => self.resolve_array(name),
            Expr::Member { .. } => {
                let (idx, fty) = self.member_object_and_ty(base)?;
                match fty {
                    Ty::Array(elem, len) => {
                        if matches!(*elem, Ty::Struct(_)) {
                            return Err(LowerError::TypeMismatch(
                                "arrays of `struct` are not supported".into(),
                            ));
                        }
                        Ok((format!("v{}", idx), *elem, len))
                    }
                    _ => Err(LowerError::TypeMismatch(
                        "`[` requires an array (variable or struct member array)".into(),
                    )),
                }
            }
            _ => Err(LowerError::TypeMismatch(
                "`[` base must be an array variable or struct member array".into(),
            )),
        }
    }

    fn is_struct_ty(&self, ty: &Ty) -> bool {
        matches!(ty, Ty::Struct(_))
    }

    fn struct_field_lookup(
        &self,
        tag: &str,
        field: &str,
    ) -> Result<(Ty, usize), LowerError> {
        let layout = self
            .struct_layouts
            .get(tag)
            .ok_or_else(|| LowerError::UnknownStruct(tag.to_string()))?;
        for (fname, fty, off) in &layout.fields {
            if fname == field {
                return Ok((fty.clone(), *off));
            }
        }
        Err(LowerError::TypeMismatch(format!(
            "struct `{tag}` has no field `{field}`"
        )))
    }

    /// Address of a (possibly nested) member: first slot index + complete field type.
    fn member_object_and_ty(&self, e: &Expr) -> Result<(usize, Ty), LowerError> {
        match e {
            Expr::Var(name) => match self.resolve(name)? {
                Binding::Struct {
                    first_slot, tag, ..
                } => Ok((slot_index(&first_slot)?, Ty::Struct(tag.clone()))),
                _ => Err(LowerError::TypeMismatch(
                    "`.` requires a struct on the left".into(),
                )),
            },
            Expr::Member { base, field } => {
                let (bidx, bty) = self.member_object_and_ty(base)?;
                let Ty::Struct(tag) = bty else {
                    return Err(LowerError::TypeMismatch(
                        "`.` requires struct on the left".into(),
                    ));
                };
                let (fty, off) = self.struct_field_lookup(&tag, field)?;
                Ok((bidx + off, fty))
            }
            _ => Err(LowerError::TypeMismatch(
                "`.` expects variable or member chain".into(),
            )),
        }
    }

    fn lvalue_slot_index_and_ty(&self, e: &Expr) -> Result<(usize, Ty), LowerError> {
        match e {
            Expr::Var(name) => Ok(match self.resolve(name)? {
                Binding::Scalar { slot, ty } => (slot_index(&slot)?, ty),
                Binding::Struct { .. } => {
                    return Err(LowerError::TypeMismatch(
                        "assign or copy a whole struct with `a = b`".into(),
                    ));
                }
                Binding::Array { .. } => {
                    return Err(LowerError::TypeMismatch(
                        "use indexed assignment for arrays".into(),
                    ));
                }
            }),
            Expr::Member { .. } => {
                let (idx, fty) = self.member_object_and_ty(e)?;
                if matches!(fty, Ty::Array(..)) {
                    return Err(LowerError::TypeMismatch(
                        "assign array elements with `[i]`, not the array member alone".into(),
                    ));
                }
                Ok((idx, fty))
            }
            Expr::Index { .. } => Err(LowerError::TypeMismatch(
                "internal: index assign uses StoreIndex".into(),
            )),
            _ => Err(LowerError::TypeMismatch(
                "expression is not a valid assignment target".into(),
            )),
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes
            .pop()
            .expect("lower: cannot pop root scope");
        assert!(!self.scopes.is_empty(), "lower: root scope must remain");
    }

    fn expect_ty(got: Ty, want: Ty, msg: &str) -> Result<(), LowerError> {
        if got != want {
            return Err(LowerError::TypeMismatch(format!("{msg} (got {got:?}, want {want:?})")));
        }
        Ok(())
    }

    fn is_builtin(name: &str) -> bool {
        matches!(
            name,
            BUILTIN_PRINT_INT
                | BUILTIN_PRINT_BOOL
                | BUILTIN_PRINT_CHAR
                | BUILTIN_PRINT_STRING
        )
    }

    fn check_array_literal_elem(&self, elem_ty: Ty, e: &Expr) -> Result<(), LowerError> {
        match elem_ty {
            Ty::Struct(_) => {
                return Err(LowerError::TypeMismatch(
                    "arrays of `struct` are not supported".into(),
                ));
            }
            Ty::Array(_, _) => {
                return Err(LowerError::TypeMismatch(
                    "nested array types are not supported".into(),
                ));
            }
            Ty::Bool => {
                Self::expect_ty(self.expr_ty(e)?, Ty::Bool, "`bool[]` element")?;
            }
            Ty::Int => {
                let t = self.expr_ty(e)?;
                if t != Ty::Int && t != Ty::Char {
                    return Err(LowerError::TypeMismatch(format!(
                        "`int[]` element expects int or char, got {t:?}"
                    )));
                }
            }
            Ty::Char => match e {
                Expr::IntLit(n) => {
                    if *n < 0 || *n > 255 {
                        return Err(LowerError::TypeMismatch(format!(
                            "`char[]` element from integer literal must be 0..=255, got {}",
                            *n
                        )));
                    }
                }
                _ => {
                    let t = self.expr_ty(e)?;
                    if t != Ty::Int && t != Ty::Char {
                        return Err(LowerError::TypeMismatch(format!(
                            "`char[]` element expects int or char, got {t:?}"
                        )));
                    }
                }
            },
        }
        Ok(())
    }

    fn expr_ty(&self, expr: &Expr) -> Result<Ty, LowerError> {
        match expr {
            Expr::IntLit(_) => Ok(Ty::Int),
            Expr::BoolLit(_) => Ok(Ty::Bool),
            Expr::CharLit(_) => Ok(Ty::Char),
            Expr::StringLit(_) => Err(LowerError::TypeMismatch(
                "string literal cannot be used as a scalar expression".into(),
            )),
            Expr::Var(name) => match self.resolve(name)? {
                Binding::Scalar { ty, .. } => Ok(ty),
                Binding::Struct { tag, .. } => Ok(Ty::Struct(tag.clone())),
                Binding::Array { .. } => Err(LowerError::TypeMismatch(format!(
                    "array `{name}` cannot be used as a scalar value"
                ))),
            },
            Expr::Member { base, field } => {
                let bty = self.expr_ty(base)?;
                let Ty::Struct(tag) = bty else {
                    return Err(LowerError::TypeMismatch(
                        "`.` requires struct on the left".into(),
                    ));
                };
                let (fty, _) = self.struct_field_lookup(&tag, field)?;
                if matches!(fty, Ty::Array(..)) {
                    return Err(LowerError::TypeMismatch(
                        "struct array member is not a scalar; subscript it with `[`".into(),
                    ));
                }
                Ok(fty)
            }
            Expr::Index { base, index } => {
                let (_, elem, _) = self.resolve_array_expr(base)?;
                if matches!(elem, Ty::Struct(_)) {
                    return Err(LowerError::TypeMismatch(
                        "arrays of `struct` are not supported".into(),
                    ));
                }
                let it = self.expr_ty(index)?;
                let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                if !ok(it.clone()) {
                    return Err(LowerError::TypeMismatch(format!(
                        "array index must be int or char, got {it:?}"
                    )));
                }
                Ok(elem)
            }
            Expr::PostInc(inner) => {
                let t = self.expr_ty(inner)?;
                match t {
                    Ty::Bool => Err(LowerError::TypeMismatch(
                        "postfix `++` expects int or char variable".into(),
                    )),
                    Ty::Struct(_) => Err(LowerError::TypeMismatch(
                        "postfix `++` cannot apply to struct".into(),
                    )),
                    Ty::Array(_, _) => Err(LowerError::TypeMismatch(
                        "postfix `++` cannot apply to an array".into(),
                    )),
                    Ty::Int | Ty::Char => Ok(Ty::Int),
                }
            }
            Expr::Unary(UnaryOp::Neg, e) => {
                let t = self.expr_ty(e)?;
                if matches!(t, Ty::Struct(_)) {
                    return Err(LowerError::TypeMismatch(
                        "unary `-` expects int or char operand".into(),
                    ));
                }
                if t == Ty::Int || t == Ty::Char {
                    Ok(Ty::Int)
                } else {
                    Err(LowerError::TypeMismatch(
                        "unary `-` expects int or char operand".into(),
                    ))
                }
            }
            Expr::Unary(UnaryOp::Not, e) => {
                let t = self.expr_ty(e)?;
                if matches!(t, Ty::Struct(_)) {
                    return Err(LowerError::TypeMismatch(
                        "unary `!` expects bool operand".into(),
                    ));
                }
                Self::expect_ty(t, Ty::Bool, "unary `!` expects bool operand")?;
                Ok(Ty::Bool)
            }
            Expr::Call { name, args } => {
                if Self::is_builtin(name) {
                    return Err(LowerError::TypeMismatch(format!(
                        "builtin `{name}` does not produce a value"
                    )));
                }
                let sig = self
                    .fn_sigs
                    .get(name)
                    .ok_or_else(|| LowerError::UndefinedFunction(name.clone()))?;
                if args.len() != sig.params.len() {
                    return Err(LowerError::TypeMismatch(format!(
                        "function `{name}` expects {} arguments, got {}",
                        sig.params.len(),
                        args.len()
                    )));
                }
                for (a, pty) in args.iter().zip(&sig.params) {
                    let at = self.expr_ty(a)?;
                    Self::expect_ty(at, pty.clone(), &format!("argument to `{name}`"))?;
                }
                match &sig.ret {
                    RetTy::Void => Err(LowerError::TypeMismatch(format!(
                        "function `{name}` returns void and cannot be used as an expression"
                    ))),
                    RetTy::Scalar(t) => Ok(t.clone()),
                }
            }
            Expr::Binary(op, l, r) => {
                let lt = self.expr_ty(l)?;
                let rt = self.expr_ty(r)?;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        if matches!(lt, Ty::Struct(_))
                            || matches!(rt, Ty::Struct(_))
                            || matches!(lt, Ty::Array(..))
                            || matches!(rt, Ty::Array(..))
                        {
                            return Err(LowerError::TypeMismatch(format!(
                                "`{op:?}` expects int or char operands, got {lt:?} and {rt:?}"
                            )));
                        }
                        let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                        if !ok(lt.clone()) || !ok(rt.clone()) {
                            return Err(LowerError::TypeMismatch(format!(
                                "`{op:?}` expects int or char operands, got {lt:?} and {rt:?}"
                            )));
                        }
                        Ok(Ty::Int)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        if matches!(lt, Ty::Struct(_))
                            || matches!(rt, Ty::Struct(_))
                            || matches!(lt, Ty::Array(..))
                            || matches!(rt, Ty::Array(..))
                        {
                            return Err(LowerError::TypeMismatch(
                                "cannot compare structs or arrays".into(),
                            ));
                        }
                        let bool_cmp = lt == Ty::Bool && rt == Ty::Bool;
                        let num_cmp = (lt == Ty::Int || lt == Ty::Char)
                            && (rt == Ty::Int || rt == Ty::Char);
                        if !bool_cmp && !num_cmp {
                            return Err(LowerError::TypeMismatch(format!(
                                "comparison expects both bool or both numeric (int/char), got {lt:?} and {rt:?}"
                            )));
                        }
                        Ok(Ty::Bool)
                    }
                    BinOp::And | BinOp::Or => {
                        Self::expect_ty(lt, Ty::Bool, &format!("`{op:?}` LHS"))?;
                        Self::expect_ty(rt, Ty::Bool, &format!("`{op:?}` RHS"))?;
                        Ok(Ty::Bool)
                    }
                }
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> Result<String, LowerError> {
        match expr {
            Expr::IntLit(n) => {
                let dst = self.fresh_temp();
                self.ir.push(Instr::Const(dst.clone(), *n));
                Ok(dst)
            }
            Expr::BoolLit(b) => {
                let dst = self.fresh_temp();
                self.ir
                    .push(Instr::Const(dst.clone(), if *b { 1 } else { 0 }));
                Ok(dst)
            }
            Expr::CharLit(b) => {
                let dst = self.fresh_temp();
                self.ir
                    .push(Instr::Const(dst.clone(), i64::from(*b)));
                Ok(dst)
            }
            Expr::StringLit(_) => Err(LowerError::TypeMismatch(
                "string literal must be used only as `print_string(\"...\")` or `char s[] = \"...\";`"
                    .into(),
            )),
            Expr::Var(name) => {
                match self.resolve(name)? {
                    Binding::Scalar { slot, .. } => {
                        let dst = self.fresh_temp();
                        self.ir.push(Instr::LoadVar(dst.clone(), slot));
                        Ok(dst)
                    }
                    Binding::Struct { .. } => Err(LowerError::TypeMismatch(
                        "struct cannot be used as a scalar expression".into(),
                    )),
                    Binding::Array { .. } => Err(LowerError::TypeMismatch(
                        "array cannot be used as a scalar expression".into(),
                    )),
                }
            }
            Expr::Member { .. } => {
                let (idx, fty) = self.member_object_and_ty(expr)?;
                if self.is_struct_ty(&fty) || matches!(fty, Ty::Array(..)) {
                    return Err(LowerError::TypeMismatch(
                        "use struct only in assignment, call argument, or member chain; index array members with `[`".into(),
                    ));
                }
                let dst = self.fresh_temp();
                self
                    .ir
                    .push(Instr::LoadVar(dst.clone(), format!("v{}", idx)));
                Ok(dst)
            }
            Expr::Index { base, index } => {
                let (first, _, len) = self.resolve_array_expr(base)?;
                let idx_t = self.lower_expr(index)?;
                let dst = self.fresh_temp();
                self
                    .ir
                    .push(Instr::LoadIndex(dst.clone(), first, idx_t, len));
                Ok(dst)
            }
            Expr::PostInc(inner) => self.lower_post_inc_expr(inner),
            Expr::Unary(op, inner) => {
                let src = self.lower_expr(inner)?;
                let dst = self.fresh_temp();
                let instr = match op {
                    UnaryOp::Neg => Instr::Neg(dst.clone(), src),
                    UnaryOp::Not => Instr::Not(dst.clone(), src),
                };
                self.ir.push(instr);
                Ok(dst)
            }
            Expr::Call { name, args } => {
                let v = self.lower_call(name, args, true)?;
                v.ok_or_else(|| {
                    LowerError::TypeMismatch(format!("void function `{name}` has no value"))
                })
            }
            Expr::Binary(op, l, r) => {
                let a = self.lower_expr(l)?;
                let b = self.lower_expr(r)?;
                let dst = self.fresh_temp();
                let instr = match op {
                    BinOp::Add => Instr::Add(dst.clone(), a, b),
                    BinOp::Sub => Instr::Sub(dst.clone(), a, b),
                    BinOp::Mul => Instr::Mul(dst.clone(), a, b),
                    BinOp::Div => Instr::Div(dst.clone(), a, b),
                    BinOp::Eq => Instr::Eq(dst.clone(), a, b),
                    BinOp::Ne => Instr::Ne(dst.clone(), a, b),
                    BinOp::Lt => Instr::Lt(dst.clone(), a, b),
                    BinOp::Le => Instr::Le(dst.clone(), a, b),
                    BinOp::Gt => Instr::Gt(dst.clone(), a, b),
                    BinOp::Ge => Instr::Ge(dst.clone(), a, b),
                    BinOp::And => Instr::And(dst.clone(), a, b),
                    BinOp::Or => Instr::Or(dst.clone(), a, b),
                };
                self.ir.push(instr);
                Ok(dst)
            }
        }
    }

    /// Postfix `i++`: load old, add 1, store (mask **`char`** to 0–255), return old value (**`int`**).
    fn lower_post_inc_expr(&mut self, inner: &Expr) -> Result<String, LowerError> {
        match inner {
            Expr::Index { base, index } => {
                let (first, var_ty, len) = self.resolve_array_expr(base)?;
                if matches!(var_ty, Ty::Struct(_)) {
                    return Err(LowerError::TypeMismatch(
                        "arrays of struct not supported".into(),
                    ));
                }
                match var_ty {
                    Ty::Bool => Err(LowerError::TypeMismatch(
                        "postfix `++` expects int or char variable".into(),
                    )),
                    Ty::Int | Ty::Char => {
                        let idx_t = self.lower_expr(index)?;
                        let old = self.fresh_temp();
                        self.ir.push(Instr::LoadIndex(
                            old.clone(),
                            first.clone(),
                            idx_t.clone(),
                            len,
                        ));
                        let one = self.fresh_temp();
                        self.ir.push(Instr::Const(one.clone(), 1));
                        let new = self.fresh_temp();
                        self.ir.push(Instr::Add(new.clone(), old.clone(), one));
                        let stored = if var_ty == Ty::Char {
                            let c255 = self.fresh_temp();
                            self.ir.push(Instr::Const(c255.clone(), 255));
                            let masked = self.fresh_temp();
                            self.ir.push(Instr::And(masked.clone(), new, c255));
                            masked
                        } else {
                            new
                        };
                        self
                            .ir
                            .push(Instr::StoreIndex(first, idx_t, stored, len));
                        Ok(old)
                    }
                    Ty::Struct(_) | Ty::Array(_, _) => unreachable!(),
                }
            }
            assign_target => {
        let (idx, var_ty) = self.lvalue_slot_index_and_ty(assign_target)?;
        let slot = format!("v{}", idx);
        match var_ty {
            Ty::Bool => Err(LowerError::TypeMismatch(
                "postfix `++` expects int or char variable".into(),
            )),
            Ty::Struct(_) => Err(LowerError::TypeMismatch(
                "postfix `++` on struct member must be scalar".into(),
            )),
            Ty::Array(_, _) => Err(LowerError::TypeMismatch(
                "postfix `++` on array member must use `[i]`".into(),
            )),
            Ty::Int => {
                let old = self.fresh_temp();
                self.ir.push(Instr::LoadVar(old.clone(), slot.clone()));
                let one = self.fresh_temp();
                self.ir.push(Instr::Const(one.clone(), 1));
                let new = self.fresh_temp();
                self.ir.push(Instr::Add(new.clone(), old.clone(), one));
                self.ir.push(Instr::StoreVar(slot, new));
                Ok(old)
            }
            Ty::Char => {
                let old = self.fresh_temp();
                self.ir.push(Instr::LoadVar(old.clone(), slot.clone()));
                let one = self.fresh_temp();
                self.ir.push(Instr::Const(one.clone(), 1));
                let new = self.fresh_temp();
                self.ir.push(Instr::Add(new.clone(), old.clone(), one));
                let c255 = self.fresh_temp();
                self.ir.push(Instr::Const(c255.clone(), 255));
                let masked = self.fresh_temp();
                self.ir.push(Instr::And(masked.clone(), new, c255));
                self.ir.push(Instr::StoreVar(slot, masked));
                Ok(old)
            }
        }
            }
        }
    }

    fn lower_add_assign(&mut self, target: &Expr, rhs: &Expr) -> Result<(), LowerError> {
        match target {
            Expr::Index { base, index } => {
                let (first, var_ty, len) = self.resolve_array_expr(base)?;
                if matches!(var_ty, Ty::Struct(_)) {
                    return Err(LowerError::TypeMismatch(
                        "arrays of struct not supported".into(),
                    ));
                }
                let rt = self.expr_ty(rhs)?;
                let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                if !ok(rt.clone()) {
                    return Err(LowerError::TypeMismatch(format!(
                        "`+=` to array element expects int or char RHS, got {rt:?}"
                    )));
                }
                let idx_t = self.lower_expr(index)?;
                let old = self.fresh_temp();
                self.ir.push(Instr::LoadIndex(
                    old.clone(),
                    first.clone(),
                    idx_t.clone(),
                    len,
                ));
                let r = self.lower_expr(rhs)?;
                let new = self.fresh_temp();
                self.ir.push(Instr::Add(new.clone(), old, r));
                let stored = if var_ty == Ty::Char {
                    let c255 = self.fresh_temp();
                    self.ir.push(Instr::Const(c255.clone(), 255));
                    let masked = self.fresh_temp();
                    self.ir.push(Instr::And(masked.clone(), new, c255));
                    masked
                } else {
                    new
                };
                self
                    .ir
                    .push(Instr::StoreIndex(first, idx_t, stored, len));
                Ok(())
            }
            _ => {
        let (idx, var_ty) = self.lvalue_slot_index_and_ty(target)?;
        let slot = format!("v{}", idx);
        match var_ty {
            Ty::Bool => Err(LowerError::TypeMismatch(
                "`+=` expects int or char variable".into(),
            )),
            Ty::Struct(_) => Err(LowerError::TypeMismatch(
                "`+=` cannot apply to a struct".into(),
            )),
            Ty::Array(_, _) => Err(LowerError::TypeMismatch(
                "`+=` on an array member must use `[i]`".into(),
            )),
            Ty::Int => {
                let rt = self.expr_ty(rhs)?;
                let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                if !ok(rt.clone()) {
                    return Err(LowerError::TypeMismatch(format!(
                        "`+=` to int expects int or char RHS, got {rt:?}"
                    )));
                }
                let old = self.fresh_temp();
                self.ir.push(Instr::LoadVar(old.clone(), slot.clone()));
                let r = self.lower_expr(rhs)?;
                let new = self.fresh_temp();
                self.ir.push(Instr::Add(new.clone(), old, r));
                self.ir.push(Instr::StoreVar(slot, new));
                Ok(())
            }
            Ty::Char => {
                let rt = self.expr_ty(rhs)?;
                let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                if !ok(rt.clone()) {
                    return Err(LowerError::TypeMismatch(format!(
                        "`+=` to char expects int or char RHS, got {rt:?}"
                    )));
                }
                let old = self.fresh_temp();
                self.ir.push(Instr::LoadVar(old.clone(), slot.clone()));
                let r = self.lower_expr(rhs)?;
                let new = self.fresh_temp();
                self.ir.push(Instr::Add(new.clone(), old, r));
                let c255 = self.fresh_temp();
                self.ir.push(Instr::Const(c255.clone(), 255));
                let masked = self.fresh_temp();
                self.ir.push(Instr::And(masked.clone(), new, c255));
                self.ir.push(Instr::StoreVar(slot, masked));
                Ok(())
            }
        }
            }
        }
    }

    fn whole_struct_value(&self, e: &Expr) -> Result<(usize, Ty), LowerError> {
        let ty = self.expr_ty(e)?;
        if !self.is_struct_ty(&ty) {
            return Err(LowerError::TypeMismatch("expected struct value".into()));
        }
        match e {
            Expr::Var(n) => {
                let Binding::Struct { first_slot, .. } = self.resolve(n)? else {
                    return Err(LowerError::TypeMismatch(
                        "expected struct variable".into(),
                    ));
                };
                Ok((slot_index(&first_slot)?, ty))
            }
            Expr::Member { .. } => self.member_object_and_ty(e),
            _ => Err(LowerError::TypeMismatch(
                "struct copy requires variable or member on each side".into(),
            )),
        }
    }

    fn lower_builtin_call(
        &mut self,
        name: &str,
        args: &[Expr],
        need_value: bool,
    ) -> Result<Option<String>, LowerError> {
        if need_value {
            return Err(LowerError::TypeMismatch(format!(
                "builtin `{name}` does not return a value"
            )));
        }
        match name {
            BUILTIN_PRINT_INT => {
                if args.len() != 1 {
                    return Err(LowerError::TypeMismatch(format!(
                        "`print_int` expects 1 argument, got {}",
                        args.len()
                    )));
                }
                Self::expect_ty(self.expr_ty(&args[0])?, Ty::Int, "`print_int` argument")?;
                let tmp = self.lower_expr(&args[0])?;
                self.ir.push(Instr::PrintInt(tmp));
            }
            BUILTIN_PRINT_BOOL => {
                if args.len() != 1 {
                    return Err(LowerError::TypeMismatch(format!(
                        "`print_bool` expects 1 argument, got {}",
                        args.len()
                    )));
                }
                Self::expect_ty(self.expr_ty(&args[0])?, Ty::Bool, "`print_bool` argument")?;
                let tmp = self.lower_expr(&args[0])?;
                self.ir.push(Instr::PrintBool(tmp));
            }
            BUILTIN_PRINT_CHAR => {
                if args.len() != 1 {
                    return Err(LowerError::TypeMismatch(format!(
                        "`print_char` expects 1 argument, got {}",
                        args.len()
                    )));
                }
                Self::expect_ty(self.expr_ty(&args[0])?, Ty::Char, "`print_char` argument")?;
                let tmp = self.lower_expr(&args[0])?;
                self.ir.push(Instr::PrintChar(tmp));
            }
            BUILTIN_PRINT_STRING => {
                if args.len() != 1 {
                    return Err(LowerError::TypeMismatch(format!(
                        "`print_string` expects 1 argument, got {}",
                        args.len()
                    )));
                }
                let (first_slot, len) = match &args[0] {
                    Expr::StringLit(raw) => {
                        let mut v = raw.clone();
                        v.push(0);
                        let first = self.emit_stack_c_string(&v);
                        (first, v.len())
                    }
                    Expr::Var(aname) => {
                        let (first, elem, len) = self.resolve_array(aname)?;
                        if elem != Ty::Char {
                            return Err(LowerError::TypeMismatch(
                                "`print_string` expects `char[]`, got non-char array".into(),
                            ));
                        }
                        (first, len)
                    }
                    _ => {
                        return Err(LowerError::TypeMismatch(
                            "`print_string` expects `\"...\"` or a `char[]` variable".into(),
                        ));
                    }
                };
                self.lower_print_string_chars(&first_slot, len);
            }
            _ => {
                return Err(LowerError::TypeMismatch(format!(
                    "unknown builtin `{name}`"
                )));
            }
        }
        Ok(None)
    }

    fn lower_user_call(
        &mut self,
        name: &str,
        args: &[Expr],
        need_value: bool,
    ) -> Result<Option<String>, LowerError> {
        let sig = self
            .fn_sigs
            .get(name)
            .ok_or_else(|| LowerError::UndefinedFunction(name.to_string()))?;
        if args.len() != sig.params.len() {
            return Err(LowerError::TypeMismatch(format!(
                "function `{name}` expects {} arguments, got {}",
                sig.params.len(),
                args.len()
            )));
        }
        for (a, pty) in args.iter().zip(&sig.params) {
            let at = self.expr_ty(a)?;
            Self::expect_ty(at, pty.clone(), &format!("argument to `{name}`"))?;
        }
        let mut arg_temps = Vec::new();
        for a in args {
            match self.expr_ty(a)? {
                Ty::Struct(_) => {
                    let (s0, st) = self.whole_struct_value(a)?;
                    let n = ty_size_words(&st, self.struct_layouts)?;
                    for i in 0..n {
                        let t = self.fresh_temp();
                        self.ir.push(Instr::LoadVar(
                            t.clone(),
                            format!("v{}", s0 + i),
                        ));
                        arg_temps.push(t);
                    }
                }
                _ => arg_temps.push(self.lower_expr(a)?),
            }
        }
        match sig.ret {
            RetTy::Void => {
                if need_value {
                    return Err(LowerError::TypeMismatch(format!(
                        "function `{name}` returns void"
                    )));
                }
                self.ir.push(Instr::Call {
                    dst: None,
                    callee: name.to_string(),
                    args: arg_temps,
                });
                Ok(None)
            }
            RetTy::Scalar(_) => {
                let d = self.fresh_temp();
                self.ir.push(Instr::Call {
                    dst: Some(d.clone()),
                    callee: name.to_string(),
                    args: arg_temps,
                });
                Ok(Some(d))
            }
        }
    }

    fn lower_call(
        &mut self,
        name: &str,
        args: &[Expr],
        need_value: bool,
    ) -> Result<Option<String>, LowerError> {
        if Self::is_builtin(name) {
            self.lower_builtin_call(name, args, need_value)
        } else {
            self.lower_user_call(name, args, need_value)
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<(), LowerError> {
        match stmt {
            Stmt::VarDecl { name, ty, init } => {
                if let Ty::Struct(ref tag) = ty {
                    match init {
                        None => {
                            self.declare_struct(name, tag)?;
                        }
                        Some(init_e) => {
                            let it = self.expr_ty(init_e)?;
                            Self::expect_ty(it, ty.clone(), "struct initializer vs declared type")?;
                            self.declare_struct(name, tag)?;
                            let (dst0, _) = self.whole_struct_value(&Expr::Var(name.clone()))?;
                            let (src0, st) = self.whole_struct_value(init_e)?;
                            let n = ty_size_words(&st, self.struct_layouts)?;
                            for i in 0..n {
                                let t = self.fresh_temp();
                                self.ir.push(Instr::LoadVar(
                                    t.clone(),
                                    format!("v{}", src0 + i),
                                ));
                                self
                                    .ir
                                    .push(Instr::StoreVar(format!("v{}", dst0 + i), t));
                            }
                        }
                    }
                } else {
                    let init_e = init.as_ref().ok_or_else(|| {
                        LowerError::TypeMismatch(
                            "scalar variables require an initializer".into(),
                        )
                    })?;
                    match init_e {
                        Expr::IntLit(n) if matches!(ty, Ty::Char) => {
                            if *n < 0 || *n > 255 {
                                return Err(LowerError::TypeMismatch(format!(
                                    "`char` initializer from integer must be 0..=255, got {}",
                                    *n
                                )));
                            }
                        }
                        _ => {
                            let init_ty = self.expr_ty(init_e)?;
                            Self::expect_ty(init_ty, ty.clone(), "variable initializer vs declared type")?;
                        }
                    }
                    let slot = self.declare(name, ty.clone())?;
                    let tmp = self.lower_expr(init_e)?;
                    self.ir.push(Instr::StoreVar(slot, tmp));
                }
            }
            Stmt::ArrayDecl { name, elem_ty, len } => {
                self.declare_array(name, elem_ty.clone(), *len)?;
            }
            Stmt::CharArrayFromString { name, bytes } => {
                self.declare_char_array_from_bytes(name, bytes)?;
            }
            Stmt::ArrayFromExprs {
                name,
                elem_ty,
                elements,
            } => {
                if self.fn_sigs.contains_key(name) {
                    return Err(LowerError::ParameterShadowsFunction(name.clone()));
                }
                if elements.is_empty() {
                    return Err(LowerError::TypeMismatch(
                        "array initializer list must not be empty".into(),
                    ));
                }
                for e in elements {
                    self.check_array_literal_elem(elem_ty.clone(), e)?;
                }
                let len = elements.len();
                let start = self.slot_id;
                self.slot_id += len;
                let first_slot = format!("v{}", start);
                self.scopes
                    .last_mut()
                    .expect("lower: always at least one scope")
                    .insert(
                        name.clone(),
                        Binding::Array {
                            first_slot: first_slot.clone(),
                            elem: elem_ty.clone(),
                            len,
                        },
                    );
                let mut temps = Vec::new();
                for e in elements {
                    temps.push(self.lower_expr(e)?);
                }
                for (i, tmp) in temps.into_iter().enumerate() {
                    let slot = format!("v{}", start + i);
                    let val = if *elem_ty == Ty::Char {
                        let c255 = self.fresh_temp();
                        self.ir.push(Instr::Const(c255.clone(), 255));
                        let masked = self.fresh_temp();
                        self.ir.push(Instr::And(masked.clone(), tmp, c255));
                        masked
                    } else {
                        tmp
                    };
                    self.ir.push(Instr::StoreVar(slot, val));
                }
            }
            Stmt::Assign { target, value } => {
                match target {
                    Expr::Index { base, index } => {
                        let (first, elem_ty, len) = self.resolve_array_expr(base)?;
                        match elem_ty {
                            Ty::Char => match value {
                                Expr::IntLit(n) => {
                                    if *n < 0 || *n > 255 {
                                        return Err(LowerError::TypeMismatch(format!(
                                            "assign to `char` from integer must be 0..=255, got {}",
                                            *n
                                        )));
                                    }
                                }
                                _ => {
                                    let val_ty = self.expr_ty(value)?;
                                    Self::expect_ty(
                                        val_ty,
                                        Ty::Char,
                                        "assignment to `char[]` element",
                                    )?;
                                }
                            },
                            _ => {
                                let val_ty = self.expr_ty(value)?;
                                Self::expect_ty(val_ty, elem_ty.clone(), "assignment to array element")?;
                            }
                        }
                        let idx_t = self.lower_expr(index)?;
                        let val_t = self.lower_expr(value)?;
                        let stored = if elem_ty == Ty::Char {
                            let c255 = self.fresh_temp();
                            self.ir.push(Instr::Const(c255.clone(), 255));
                            let masked = self.fresh_temp();
                            self.ir.push(Instr::And(masked.clone(), val_t, c255));
                            masked
                        } else {
                            val_t
                        };
                        self
                            .ir
                            .push(Instr::StoreIndex(first, idx_t, stored, len));
                    }
                    _ => {
                        let lhs_ty = self.expr_ty(target)?;
                        if self.is_struct_ty(&lhs_ty) {
                            let rhs_ty = self.expr_ty(value)?;
                            Self::expect_ty(
                                rhs_ty,
                                lhs_ty.clone(),
                                "struct assignment RHS vs target type",
                            )?;
                            let (dst0, _) = self.whole_struct_value(target)?;
                            let (src0, st) = self.whole_struct_value(value)?;
                            let n = ty_size_words(&st, self.struct_layouts)?;
                            for i in 0..n {
                                let t = self.fresh_temp();
                                self.ir.push(Instr::LoadVar(
                                    t.clone(),
                                    format!("v{}", src0 + i),
                                ));
                                self
                                    .ir
                                    .push(Instr::StoreVar(format!("v{}", dst0 + i), t));
                            }
                        } else {
                            let (idx, var_ty) = self.lvalue_slot_index_and_ty(target)?;
                            let slot = format!("v{}", idx);
                            match var_ty {
                                Ty::Char => match value {
                                    Expr::IntLit(n) => {
                                        if *n < 0 || *n > 255 {
                                            return Err(LowerError::TypeMismatch(format!(
                                                "assign to `char` from integer must be 0..=255, got {}",
                                                *n
                                            )));
                                        }
                                    }
                                    _ => {
                                        let val_ty = self.expr_ty(value)?;
                                        Self::expect_ty(val_ty, Ty::Char, "assignment to `char`")?;
                                    }
                                },
                                _ => {
                                    let val_ty = self.expr_ty(value)?;
                                    Self::expect_ty(
                                        val_ty,
                                        var_ty.clone(),
                                        "assignment RHS vs target type",
                                    )?;
                                }
                            }
                            let tmp = self.lower_expr(value)?;
                            let stored = if var_ty == Ty::Char {
                                let c255 = self.fresh_temp();
                                self.ir.push(Instr::Const(c255.clone(), 255));
                                let masked = self.fresh_temp();
                                self.ir.push(Instr::And(masked.clone(), tmp, c255));
                                masked
                            } else {
                                tmp
                            };
                            self.ir.push(Instr::StoreVar(slot, stored));
                        }
                    }
                }
            }
            Stmt::AddAssign { target, rhs } => {
                self.lower_add_assign(target, rhs)?;
            }
            Stmt::PostInc { target } => {
                self.lower_post_inc_expr(target)?;
            }
            Stmt::Expr(Expr::Call { name, args }) => {
                if Self::is_builtin(name) {
                    self.lower_builtin_call(name, args, false)?;
                    return Ok(());
                }
                let sig = self
                    .fn_sigs
                    .get(name)
                    .ok_or_else(|| LowerError::UndefinedFunction(name.clone()))?;
                if sig.ret != RetTy::Void {
                    return Err(LowerError::TypeMismatch(format!(
                        "non-void function `{name}` cannot be used as an expression statement"
                    )));
                }
                self.lower_user_call(name, args, false)?;
            }
            Stmt::Expr(_) => {
                return Err(LowerError::TypeMismatch(
                    "expression statement must be a void function call".into(),
                ));
            }
            Stmt::Return(opt) => {
                let ret_ty = self
                    .current_ret
                    .clone()
                    .ok_or(LowerError::ReturnOutsideFunction)?;
                match (ret_ty, opt) {
                    (RetTy::Void, None) => {
                        self.ir.push(Instr::Ret(None));
                    }
                    (RetTy::Void, Some(_)) => {
                        return Err(LowerError::BadReturn(
                            "void function must use `return;` without a value".into(),
                        ));
                    }
                    (RetTy::Scalar(_), None) => {
                        return Err(LowerError::BadReturn(
                            "non-void function must return a value".into(),
                        ));
                    }
                    (RetTy::Scalar(want), Some(e)) => {
                        let got = self.expr_ty(e)?;
                        Self::expect_ty(got, want, "`return` value vs function return type")?;
                        let t = self.lower_expr(e)?;
                        self.ir.push(Instr::Ret(Some(t)));
                    }
                }
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.lower_stmt(s)?;
                }
                self.pop_scope();
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::expect_ty(self.expr_ty(cond)?, Ty::Bool, "`if` condition")?;
                let c = self.lower_expr(cond)?;
                let end_l = self.fresh_label("end_if");

                match else_branch {
                    None => {
                        self.ir.push(Instr::JumpIfZero(c, end_l.clone()));
                        self.push_scope();
                        self.lower_stmt(then_branch)?;
                        self.pop_scope();
                        self.ir.push(Instr::Label(end_l));
                    }
                    Some(else_s) => {
                        let else_l = self.fresh_label("else");
                        self.ir.push(Instr::JumpIfZero(c, else_l.clone()));
                        self.push_scope();
                        self.lower_stmt(then_branch)?;
                        self.pop_scope();
                        self.ir.push(Instr::Jump(end_l.clone()));
                        self.ir.push(Instr::Label(else_l));
                        self.push_scope();
                        self.lower_stmt(else_s)?;
                        self.pop_scope();
                        self.ir.push(Instr::Label(end_l));
                    }
                }
            }
            Stmt::While { cond, body } => {
                Self::expect_ty(self.expr_ty(cond)?, Ty::Bool, "`while` condition")?;
                let head = self.fresh_label("while_head");
                let end = self.fresh_label("while_end");
                self.ir.push(Instr::Label(head.clone()));
                let c = self.lower_expr(cond)?;
                self.ir.push(Instr::JumpIfZero(c, end.clone()));
                self.push_scope();
                self.lower_stmt(body)?;
                self.pop_scope();
                self.ir.push(Instr::Jump(head));
                self.ir.push(Instr::Label(end));
            }
            Stmt::For {
                init,
                cond,
                step,
                body,
            } => {
                self.push_scope();
                if let Some(init) = init {
                    self.lower_stmt(init)?;
                }
                let head = self.fresh_label("for_head");
                let end = self.fresh_label("for_end");
                self.ir.push(Instr::Label(head.clone()));
                if let Some(cond) = cond {
                    Self::expect_ty(self.expr_ty(cond)?, Ty::Bool, "`for` condition")?;
                    let c = self.lower_expr(cond)?;
                    self.ir.push(Instr::JumpIfZero(c, end.clone()));
                }
                self.lower_stmt(body)?;
                if let Some(step) = step {
                    self.lower_stmt(step)?;
                }
                self.ir.push(Instr::Jump(head));
                self.ir.push(Instr::Label(end));
                self.pop_scope();
            }
        }
        Ok(())
    }
}

fn slot_index(slot: &str) -> Result<usize, LowerError> {
    slot.strip_prefix('v')
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| LowerError::TypeMismatch(format!("internal: bad stack slot `{slot}`")))
}

fn ty_size_words(ty: &Ty, layouts: &HashMap<String, StructLayout>) -> Result<usize, LowerError> {
    match ty {
        Ty::Int | Ty::Bool | Ty::Char => Ok(1),
        Ty::Struct(name) => layouts
            .get(name)
            .map(|l| l.size_words)
            .ok_or_else(|| LowerError::UnknownStruct(name.clone())),
        Ty::Array(elem, n) => {
            if matches!(**elem, Ty::Struct(_)) {
                return Err(LowerError::TypeMismatch(
                    "arrays of `struct` are not supported".into(),
                ));
            }
            if matches!(**elem, Ty::Array(_, _)) {
                return Err(LowerError::TypeMismatch(
                    "nested array types are not supported".into(),
                ));
            }
            let ew = ty_size_words(elem, layouts)?;
            Ok(ew * n)
        }
    }
}

fn collect_struct_defs(program: &Program) -> Result<HashMap<String, Vec<(String, Ty)>>, LowerError> {
    let mut m = HashMap::new();
    for item in &program.items {
        if let Item::StructDef { name, fields } = item {
            if m.insert(name.clone(), fields.clone()).is_some() {
                return Err(LowerError::DuplicateStruct(name.clone()));
            }
        }
    }
    Ok(m)
}

fn compute_struct_layouts(
    defs: &HashMap<String, Vec<(String, Ty)>>,
) -> Result<HashMap<String, StructLayout>, LowerError> {
    let mut done = HashMap::new();
    for name in defs.keys() {
        layout_struct(name, defs, &mut HashSet::new(), &mut done)?;
    }
    Ok(done)
}

fn layout_struct(
    name: &str,
    defs: &HashMap<String, Vec<(String, Ty)>>,
    visiting: &mut HashSet<String>,
    done: &mut HashMap<String, StructLayout>,
) -> Result<(), LowerError> {
    if done.contains_key(name) {
        return Ok(());
    }
    if !visiting.insert(name.to_string()) {
        return Err(LowerError::RecursiveStruct(name.to_string()));
    }
    let fields_def = defs
        .get(name)
        .ok_or_else(|| LowerError::UnknownStruct(name.to_string()))?;
    let mut offset = 0usize;
    let mut fields = Vec::new();
    for (fname, fty) in fields_def {
        if let Ty::Struct(inner) = fty {
            layout_struct(inner, defs, visiting, done)?;
        }
        let sz = ty_size_words(fty, done)?;
        fields.push((fname.clone(), fty.clone(), offset));
        offset += sz;
    }
    visiting.remove(name);
    done.insert(
        name.to_string(),
        StructLayout {
            size_words: offset,
            fields,
        },
    );
    Ok(())
}

fn fn_sig_from_ast(params: &[(String, Ty)], ret: RetTy) -> FnSig {
    FnSig {
        params: params.iter().map(|(_, t)| t.clone()).collect(),
        ret,
    }
}

/// Build the user function table (each forward declaration must match its definition).
pub fn build_fn_table(
    program: &Program,
    struct_layouts: &HashMap<String, StructLayout>,
) -> Result<HashMap<String, FnSig>, LowerError> {
    let mut protos: HashMap<String, FnSig> = HashMap::new();
    let mut defs: HashMap<String, FnSig> = HashMap::new();

    for item in &program.items {
        match item {
            Item::FnDecl { name, params, ret } => {
                if let RetTy::Scalar(Ty::Struct(tn)) = ret {
                    return Err(LowerError::StructReturnNotSupported(format!(
                        "function `{name}` cannot return `struct {tn}`"
                    )));
                }
                let arg_words: usize = params
                    .iter()
                    .map(|(_, t)| ty_size_words(t, struct_layouts))
                    .sum::<Result<usize, LowerError>>()?;
                if arg_words > MAX_ARG_WORDS {
                    return Err(LowerError::TypeMismatch(format!(
                        "function `{name}`: at most {MAX_ARG_WORDS} argument words"
                    )));
                }
                let sig = fn_sig_from_ast(params, ret.clone());
                if let Some(old) = protos.get(name) {
                    if old != &sig {
                        return Err(LowerError::ConflictingPrototype(name.clone()));
                    }
                } else {
                    protos.insert(name.clone(), sig);
                }
            }
            Item::FnDef { name, params, ret, .. } => {
                if let RetTy::Scalar(Ty::Struct(tn)) = ret {
                    return Err(LowerError::StructReturnNotSupported(format!(
                        "function `{name}` cannot return `struct {tn}`"
                    )));
                }
                let arg_words: usize = params
                    .iter()
                    .map(|(_, t)| ty_size_words(t, struct_layouts))
                    .sum::<Result<usize, LowerError>>()?;
                if arg_words > MAX_ARG_WORDS {
                    return Err(LowerError::TypeMismatch(format!(
                        "function `{name}`: at most {MAX_ARG_WORDS} argument words"
                    )));
                }
                let sig = fn_sig_from_ast(params, ret.clone());
                if let Some(p) = protos.get(name) {
                    if p != &sig {
                        return Err(LowerError::ConflictingPrototype(name.clone()));
                    }
                }
                if defs.insert(name.clone(), sig).is_some() {
                    return Err(LowerError::DuplicateFunction(name.clone()));
                }
            }
            Item::Stmt(_) | Item::StructDef { .. } => {}
        }
    }

    for name in protos.keys() {
        if !defs.contains_key(name) {
            return Err(LowerError::ForwardDeclWithoutDefinition(name.clone()));
        }
    }

    Ok(defs)
}

/// Lower a full program to an [`IrModule`] (`_main` plus user functions).
pub fn lower_program(program: &Program) -> Result<IrModule, LowerError> {
    let raw_structs = collect_struct_defs(program)?;
    let struct_layouts = compute_struct_layouts(&raw_structs)?;
    let fn_table = build_fn_table(program, &struct_layouts)?;
    let mut main_cx = LowerCtx::new_main(&fn_table, &struct_layouts);
    let mut functions = Vec::new();

    for item in &program.items {
        match item {
            Item::Stmt(stmt) => main_cx.lower_stmt(stmt)?,
            Item::FnDecl { .. } | Item::StructDef { .. } => {}
            Item::FnDef {
                name,
                params,
                ret,
                body,
            } => {
                let mut cx = LowerCtx::new_function(
                    &fn_table,
                    &struct_layouts,
                    name,
                    ret.clone(),
                    params,
                )?;
                for st in body {
                    cx.lower_stmt(st)?;
                }
                cx.finish_function(ret.clone())?;
                functions.push(IrFunction {
                    name: name.clone(),
                    instrs: cx.ir.instrs,
                });
            }
        }
    }

    Ok(IrModule {
        main: main_cx.ir,
        functions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Item, Program, Stmt, Ty};
    use crate::parser::parse_program;

    #[test]
    fn lower_int_const_and_add() {
        let p = parse_program("int x = 1 + 2;").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main.instrs.iter().any(|i| matches!(i, Instr::Add(_, _, _))),
            "{ir:?}"
        );
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::StoreVar(s, _) if s.starts_with('v'))),
            "{ir:?}"
        );
    }

    #[test]
    fn undefined_var() {
        let p = parse_program("int x = y;").unwrap();
        assert!(matches!(
            lower_program(&p),
            Err(LowerError::UndefinedVariable(y)) if y == "y"
        ));
    }

    #[test]
    fn if_else_labels() {
        let p = parse_program("bool b = true; if (b) { int x = 1; } else { int x = 2; }").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(ir.main.instrs.iter().any(|i| matches!(i, Instr::JumpIfZero(_, _))));
        assert!(ir.main.instrs.iter().any(|i| matches!(i, Instr::Jump(_))));
        assert!(
            ir.main
                .instrs
                .iter()
                .filter(|i| matches!(i, Instr::Label(_)))
                .count()
                >= 2
        );
    }

    #[test]
    fn bool_lowers_to_zero_one() {
        let p = Program {
            items: vec![Item::Stmt(Stmt::VarDecl {
                name: "b".into(),
                ty: Ty::Bool,
                init: Some(Expr::BoolLit(false)),
            })],
        };
        let ir = lower_program(&p).unwrap();
        assert!(ir.main.instrs.iter().any(|i| matches!(i, Instr::Const(_, 0))));
    }

    #[test]
    fn print_int_ir() {
        let p = parse_program("print_int(42);").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(ir
            .main
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::PrintInt(_))));
    }

    #[test]
    fn print_bool_type_error() {
        let p = parse_program("print_bool(1);").unwrap();
        assert!(matches!(
            lower_program(&p),
            Err(LowerError::TypeMismatch(_))
        ));
    }

    #[test]
    fn char_literal_print_and_int_init() {
        let p = parse_program("char c = 'A'; print_char(c); char d = 66; print_char(d);").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(ir.main.instrs.iter().filter(|i| matches!(i, Instr::PrintChar(_))).count() >= 2);
    }

    #[test]
    fn while_loop_jump_pattern() {
        let p = parse_program("int i = 0; while (i < 3) { i = i + 1; }").unwrap();
        let ir = lower_program(&p).unwrap();
        let jumps = ir.main.instrs.iter().filter(|i| matches!(i, Instr::Jump(_))).count();
        assert!(jumps >= 1, "{ir:?}");
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::Label(l) if l.contains("while"))),
            "{ir:?}"
        );
    }

    #[test]
    fn for_loop_desugar() {
        let p = parse_program("for (int i = 0; i < 3; i = i + 1) { print_int(i); }").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::Label(l) if l.contains("for_head"))),
            "{ir:?}"
        );
    }

    #[test]
    fn post_inc_and_add_assign() {
        let p = parse_program("int i = 0; i++; i += 2; print_int(i);").unwrap();
        let ir = lower_program(&p).unwrap();
        let adds = ir
            .main
            .instrs
            .iter()
            .filter(|i| matches!(i, Instr::Add(_, _, _)))
            .count();
        assert!(adds >= 2, "{ir:?}");
    }

    #[test]
    fn post_inc_expr_value() {
        let p = parse_program("int i = 0; print_int(i++); print_int(i);").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main
                .instrs
                .iter()
                .filter(|i| matches!(i, Instr::PrintInt(_)))
                .count()
                >= 2,
            "{ir:?}"
        );
    }

    #[test]
    fn stack_array_load_store_index() {
        let p = parse_program("int a[3]; a[1] = 42; print_int(a[1]);").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::LoadIndex(_, _, _, _))),
            "{ir:?}"
        );
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::StoreIndex(_, _, _, _))),
            "{ir:?}"
        );
    }

    #[test]
    fn user_function_call_in_main() {
        let p = parse_program("int f() { return 7; } print_int(f());").unwrap();
        let m = lower_program(&p).unwrap();
        assert_eq!(m.functions.len(), 1);
        assert!(m.functions[0].name == "f");
        assert!(m
            .main
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::Call { callee, .. } if callee == "f")));
        assert!(m.functions[0]
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::Ret(Some(_)))));
    }

    #[test]
    fn print_string_uses_char_loop() {
        let p = parse_program(r#"print_string("ab");"#).unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main
                .instrs
                .iter()
                .any(|i| matches!(i, Instr::PrintCharOnly(_))),
            "{ir:?}"
        );
        assert!(
            ir.main.instrs.iter().any(|i| matches!(i, Instr::PrintNl)),
            "{ir:?}"
        );
    }

    #[test]
    fn struct_member_array_index_store_load() {
        let p = parse_program(
            "struct S { int xs[3]; };\n\
             struct S s;\n\
             s.xs[1] = 42;\n\
             print_int(s.xs[1]);",
        )
        .unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.main.instrs.iter().any(|i| matches!(i, Instr::StoreIndex(_, _, _, 3))),
            "{ir:?}"
        );
        assert!(
            ir.main.instrs.iter().any(|i| matches!(i, Instr::LoadIndex(_, _, _, 3))),
            "{ir:?}"
        );
    }
}
