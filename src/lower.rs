//! Lower [`crate::ast::Program`] to [`crate::ir::IrProgram`] (three-address code, one procedure).

use std::collections::HashMap;

use crate::ast::{BinOp, Expr, Program, Stmt, Ty, UnaryOp};
use crate::ir::{Instr, IrProgram};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    UndefinedVariable(String),
    TypeMismatch(String),
}

struct LowerCtx {
    ir: IrProgram,
    /// Lexical scopes: source name → (IR slot, type).
    scopes: Vec<HashMap<String, (String, Ty)>>,
    temp_id: usize,
    label_id: usize,
    slot_id: usize,
}

impl LowerCtx {
    fn new() -> Self {
        Self {
            ir: IrProgram::new(),
            scopes: vec![HashMap::new()],
            temp_id: 0,
            label_id: 0,
            slot_id: 0,
        }
    }

    fn fresh_temp(&mut self) -> String {
        let t = format!("t{}", self.temp_id);
        self.temp_id += 1;
        t
    }

    fn fresh_label(&mut self, prefix: &str) -> String {
        let l = format!("{}.{}", prefix, self.label_id);
        self.label_id += 1;
        l
    }

    fn declare(&mut self, source_name: &str, ty: Ty) -> String {
        let slot = format!("v{}", self.slot_id);
        self.slot_id += 1;
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(source_name.to_string(), (slot.clone(), ty));
        slot
    }

    fn resolve(&self, source_name: &str) -> Result<(String, Ty), LowerError> {
        for scope in self.scopes.iter().rev() {
            if let Some((s, t)) = scope.get(source_name) {
                return Ok((s.clone(), *t));
            }
        }
        Err(LowerError::UndefinedVariable(source_name.to_string()))
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

    fn expr_ty(&self, expr: &Expr) -> Result<Ty, LowerError> {
        match expr {
            Expr::IntLit(_) => Ok(Ty::Int),
            Expr::BoolLit(_) => Ok(Ty::Bool),
            Expr::CharLit(_) => Ok(Ty::Char),
            Expr::Var(name) => Ok(self.resolve(name)?.1),
            Expr::Unary(UnaryOp::Neg, e) => {
                let t = self.expr_ty(e)?;
                if t == Ty::Int || t == Ty::Char {
                    Ok(Ty::Int)
                } else {
                    Err(LowerError::TypeMismatch(
                        "unary `-` expects int or char operand".into(),
                    ))
                }
            }
            Expr::Unary(UnaryOp::Not, e) => {
                Self::expect_ty(self.expr_ty(e)?, Ty::Bool, "unary `!` expects bool operand")?;
                Ok(Ty::Bool)
            }
            Expr::Binary(op, l, r) => {
                let lt = self.expr_ty(l)?;
                let rt = self.expr_ty(r)?;
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => {
                        let ok = |t: Ty| t == Ty::Int || t == Ty::Char;
                        if !ok(lt) || !ok(rt) {
                            return Err(LowerError::TypeMismatch(format!(
                                "`{op:?}` expects int or char operands, got {lt:?} and {rt:?}"
                            )));
                        }
                        Ok(Ty::Int)
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
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
            Expr::Var(name) => {
                let (slot, _) = self.resolve(name)?;
                let dst = self.fresh_temp();
                self.ir.push(Instr::LoadVar(dst.clone(), slot));
                Ok(dst)
            }
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

    fn lower_stmt(&mut self, stmt: &Stmt) -> Result<(), LowerError> {
        match stmt {
            Stmt::VarDecl { name, ty, init } => {
                match (*ty, init) {
                    (Ty::Char, Expr::IntLit(n)) => {
                        if *n < 0 || *n > 255 {
                            return Err(LowerError::TypeMismatch(format!(
                                "`char` initializer from integer must be 0..=255, got {}",
                                *n
                            )));
                        }
                    }
                    _ => {
                        let init_ty = self.expr_ty(init)?;
                        Self::expect_ty(init_ty, *ty, "variable initializer vs declared type")?;
                    }
                }
                let slot = self.declare(name, *ty);
                let tmp = self.lower_expr(init)?;
                self.ir.push(Instr::StoreVar(slot, tmp));
            }
            Stmt::Assign { name, value } => {
                let (slot, var_ty) = self.resolve(name)?;
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
                        Self::expect_ty(val_ty, var_ty, "assignment RHS vs variable type")?;
                    }
                }
                let tmp = self.lower_expr(value)?;
                self.ir.push(Instr::StoreVar(slot, tmp));
            }
            Stmt::PrintInt { arg } => {
                Self::expect_ty(self.expr_ty(arg)?, Ty::Int, "`print_int` argument")?;
                let tmp = self.lower_expr(arg)?;
                self.ir.push(Instr::PrintInt(tmp));
            }
            Stmt::PrintBool { arg } => {
                Self::expect_ty(self.expr_ty(arg)?, Ty::Bool, "`print_bool` argument")?;
                let tmp = self.lower_expr(arg)?;
                self.ir.push(Instr::PrintBool(tmp));
            }
            Stmt::PrintChar { arg } => {
                Self::expect_ty(self.expr_ty(arg)?, Ty::Char, "`print_char` argument")?;
                let tmp = self.lower_expr(arg)?;
                self.ir.push(Instr::PrintChar(tmp));
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
        }
        Ok(())
    }
}

/// Lower a full program to linear IR for one procedure.
pub fn lower_program(program: &Program) -> Result<IrProgram, LowerError> {
    let mut cx = LowerCtx::new();
    for stmt in &program.stmts {
        cx.lower_stmt(stmt)?;
    }
    Ok(cx.ir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Stmt, Ty};
    use crate::parser::parse_program;

    #[test]
    fn lower_int_const_and_add() {
        let p = parse_program("int x = 1 + 2;").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(
            ir.instrs.iter().any(|i| matches!(i, Instr::Add(_, _, _))),
            "{ir:?}"
        );
        assert!(
            ir.instrs
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
        assert!(ir.instrs.iter().any(|i| matches!(i, Instr::JumpIfZero(_, _))));
        assert!(ir.instrs.iter().any(|i| matches!(i, Instr::Jump(_))));
        assert!(
            ir.instrs
                .iter()
                .filter(|i| matches!(i, Instr::Label(_)))
                .count()
                >= 2
        );
    }

    #[test]
    fn bool_lowers_to_zero_one() {
        let p = Program {
            stmts: vec![Stmt::VarDecl {
                name: "b".into(),
                ty: Ty::Bool,
                init: Expr::BoolLit(false),
            }],
        };
        let ir = lower_program(&p).unwrap();
        assert!(ir.instrs.iter().any(|i| matches!(i, Instr::Const(_, 0))));
    }

    #[test]
    fn print_int_ir() {
        let p = parse_program("print_int(42);").unwrap();
        let ir = lower_program(&p).unwrap();
        assert!(ir
            .instrs
            .iter()
            .any(|i| matches!(i, Instr::PrintInt(t) if t.starts_with('t')))
            );
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
        assert!(ir.instrs.iter().filter(|i| matches!(i, Instr::PrintChar(_))).count() >= 2);
    }
}
