//! Lower [`crate::ast::Program`] to [`crate::ir::IrProgram`] (three-address code, one procedure).

use std::collections::HashMap;

use crate::ast::{BinOp, Expr, Program, Stmt, UnaryOp};
use crate::ir::{Instr, IrProgram};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LowerError {
    UndefinedVariable(String),
}

struct LowerCtx {
    ir: IrProgram,
    /// Lexical scopes: source name → unique IR variable slot (e.g. `v3`).
    scopes: Vec<HashMap<String, String>>,
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

    fn declare(&mut self, source_name: &str) -> String {
        let slot = format!("v{}", self.slot_id);
        self.slot_id += 1;
        self.scopes
            .last_mut()
            .expect("lower: always at least one scope")
            .insert(source_name.to_string(), slot.clone());
        slot
    }

    fn resolve(&self, source_name: &str) -> Result<String, LowerError> {
        for scope in self.scopes.iter().rev() {
            if let Some(slot) = scope.get(source_name) {
                return Ok(slot.clone());
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
            Expr::Var(name) => {
                let slot = self.resolve(name)?;
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
            Stmt::VarDecl { name, init, .. } => {
                let slot = self.declare(name);
                let tmp = self.lower_expr(init)?;
                self.ir.push(Instr::StoreVar(slot, tmp));
            }
            Stmt::Assign { name, value } => {
                let slot = self.resolve(name)?;
                let tmp = self.lower_expr(value)?;
                self.ir.push(Instr::StoreVar(slot, tmp));
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
}
