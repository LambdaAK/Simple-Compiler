//! Recursive-descent parser for the README grammar (`program` → `stmt*` → `expr` with C-like precedence).

use crate::ast::{BinOp, Expr, Program, Stmt, Ty, UnaryOp};
use crate::lexer::{lex, LexError};
use crate::token::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub at: usize,
    pub message: String,
}

impl ParseError {
    fn new(at: usize, message: impl Into<String>) -> Self {
        Self {
            at,
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontEndError {
    Lex(LexError),
    Parse(ParseError),
}

impl From<LexError> for FrontEndError {
    fn from(e: LexError) -> Self {
        FrontEndError::Lex(e)
    }
}

impl From<ParseError> for FrontEndError {
    fn from(e: ParseError) -> Self {
        FrontEndError::Parse(e)
    }
}

/// Lex and parse a full program.
pub fn parse_program(source: &str) -> Result<Program, FrontEndError> {
    let tokens = lex(source)?;
    Parser::new(&tokens).parse_program()
}

#[derive(Clone, Copy)]
enum AfterIdentEnd {
    Semicolon,
    /// For-step: closing `)` of `for (...)` is consumed after this clause.
    ForStep,
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .expect("parser: token stream must end with Eof")
    }

    fn bump(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn error(&self, msg: impl Into<String>) -> ParseError {
        ParseError::new(self.pos, msg)
    }

    fn parse_program(&mut self) -> Result<Program, FrontEndError> {
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        Ok(Program { stmts })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, FrontEndError> {
        match self.peek().clone() {
            Token::KwInt => {
                self.bump();
                let s = self.parse_decl_tail_after_type(Ty::Int)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(s)
            }
            Token::KwBool => {
                self.bump();
                let s = self.parse_decl_tail_after_type(Ty::Bool)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(s)
            }
            Token::KwChar => {
                self.bump();
                let s = self.parse_decl_tail_after_type(Ty::Char)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(s)
            }
            Token::KwPrintInt => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let arg = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::PrintInt { arg })
            }
            Token::KwPrintBool => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let arg = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::PrintBool { arg })
            }
            Token::KwPrintChar => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let arg = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::PrintChar { arg })
            }
            Token::KwPrintString => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let arg = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::PrintString { arg })
            }
            Token::KwIf => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let cond = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                let then_branch = Box::new(self.parse_stmt()?);
                let else_branch = if matches!(self.peek(), Token::KwElse) {
                    self.bump();
                    Some(Box::new(self.parse_stmt()?))
                } else {
                    None
                };
                Ok(Stmt::If {
                    cond,
                    then_branch,
                    else_branch,
                })
            }
            Token::KwWhile => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let cond = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::While { cond, body })
            }
            Token::KwFor => {
                self.bump();
                self.consume(Token::LParen, "`(`")?;
                let init = self.parse_for_init()?;
                let cond = self.parse_for_cond()?;
                let step = self.parse_for_step()?;
                self.consume(Token::RParen, "`)`")?;
                let body = Box::new(self.parse_stmt()?);
                Ok(Stmt::For {
                    init,
                    cond,
                    step,
                    body,
                })
            }
            Token::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            Token::Ident(name) => {
                self.bump();
                if matches!(self.peek(), Token::LBracket) {
                    self.bump();
                    let index = self.parse_expr()?;
                    self.consume(Token::RBracket, "`]`")?;
                    self.parse_stmt_after_index_subscript(name, index, AfterIdentEnd::Semicolon)
                } else {
                    self.parse_stmt_after_ident(name, AfterIdentEnd::Semicolon)
                }
            }
            other => Err(self
                .error(format!("statement cannot start with `{other:?}`"))
                .into()),
        }
    }

    /// After `int` / `bool` / `char`: `name = expr`, `name [ N ]`, `name [] = { ... }`, or `char name [] = "..."`.
    fn parse_decl_tail_after_type(&mut self, elem: Ty) -> Result<Stmt, FrontEndError> {
        let name = self.expect_ident()?;
        if matches!(self.peek(), Token::LBracket) {
            self.bump();
            if matches!(self.peek(), Token::RBracket) {
                self.bump();
                self.consume(Token::Assign, "`=`")?;
                if elem == Ty::Char && matches!(self.peek(), Token::StringLit(_)) {
                    let mut raw = self.expect_string_lit()?;
                    raw.push(0);
                    return Ok(Stmt::CharArrayFromString { name, bytes: raw });
                }
                self.consume(Token::LBrace, "`{`")?;
                let elements = self.parse_brace_init_list()?;
                return Ok(Stmt::ArrayFromExprs {
                    name,
                    elem_ty: elem,
                    elements,
                });
            }
            let len = match self.bump() {
                Token::IntLit(n) => n,
                other => {
                    return Err(self
                        .error(format!(
                            "array length must be a positive integer literal, found `{other:?}`"
                        ))
                        .into());
                }
            };
            if len <= 0 {
                return Err(self.error("array length must be positive").into());
            }
            let len = len as usize;
            self.consume(Token::RBracket, "`]`")?;
            return Ok(Stmt::ArrayDecl {
                name,
                elem_ty: elem,
                len,
            });
        }
        self.consume(Token::Assign, "`=`")?;
        let init = self.parse_expr()?;
        Ok(Stmt::VarDecl {
            name,
            ty: elem,
            init,
        })
    }

    /// At least one expression; trailing comma allowed. Closing **`}`** not consumed before call; consumed here.
    fn parse_brace_init_list(&mut self) -> Result<Vec<Expr>, FrontEndError> {
        let mut out = Vec::new();
        if matches!(self.peek(), Token::RBrace) {
            return Err(self.error("empty array initializer `{ }`").into());
        }
        loop {
            out.push(self.parse_expr()?);
            match self.peek().clone() {
                Token::Comma => {
                    self.bump();
                    if matches!(self.peek(), Token::RBrace) {
                        self.bump();
                        break;
                    }
                }
                Token::RBrace => {
                    self.bump();
                    break;
                }
                other => {
                    return Err(self
                        .error(format!(
                            "expected `,` or `}}` in array initializer, found `{other:?}`"
                        ))
                        .into());
                }
            }
        }
        Ok(out)
    }

    fn parse_var_decl_stmt(&mut self) -> Result<Stmt, FrontEndError> {
        match self.peek().clone() {
            Token::KwInt => {
                self.bump();
                self.parse_decl_tail_after_type(Ty::Int)
            }
            Token::KwBool => {
                self.bump();
                self.parse_decl_tail_after_type(Ty::Bool)
            }
            Token::KwChar => {
                self.bump();
                self.parse_decl_tail_after_type(Ty::Char)
            }
            other => Err(self
                .error(format!("expected `int`, `bool`, or `char` in for-init, found `{other:?}`"))
                .into()),
        }
    }

    /// `;` only → `None`; `int x = 0;` / `x = 0;` → `Some`.
    fn parse_for_init(&mut self) -> Result<Option<Box<Stmt>>, FrontEndError> {
        if matches!(self.peek(), Token::Semicolon) {
            self.bump();
            return Ok(None);
        }
        if matches!(
            self.peek(),
            Token::KwInt | Token::KwBool | Token::KwChar
        ) {
            let stmt = self.parse_var_decl_stmt()?;
            self.consume(Token::Semicolon, "`;`")?;
            return Ok(Some(Box::new(stmt)));
        }
        if let Token::Ident(name) = self.peek().clone() {
            self.bump();
            let s = if matches!(self.peek(), Token::LBracket) {
                self.bump();
                let index = self.parse_expr()?;
                self.consume(Token::RBracket, "`]`")?;
                self.parse_stmt_after_index_subscript(name, index, AfterIdentEnd::Semicolon)?
            } else {
                self.parse_stmt_after_ident(name, AfterIdentEnd::Semicolon)?
            };
            return Ok(Some(Box::new(s)));
        }
        Err(self
            .error("expected `;`, `int`/`bool`/`char`, or assignment/`+=`/`++` in for-init")
            .into())
    }

    /// `;` only → `None`; else expression then `;`.
    fn parse_for_cond(&mut self) -> Result<Option<Expr>, FrontEndError> {
        if matches!(self.peek(), Token::Semicolon) {
            self.bump();
            return Ok(None);
        }
        let e = self.parse_expr()?;
        self.consume(Token::Semicolon, "`;`")?;
        Ok(Some(e))
    }

    /// `)` only → `None`; else `name = expr` before `)`.
    fn parse_for_step(&mut self) -> Result<Option<Box<Stmt>>, FrontEndError> {
        if matches!(self.peek(), Token::RParen) {
            return Ok(None);
        }
        if let Token::Ident(name) = self.peek().clone() {
            self.bump();
            let s = if matches!(self.peek(), Token::LBracket) {
                self.bump();
                let index = self.parse_expr()?;
                self.consume(Token::RBracket, "`]`")?;
                self.parse_stmt_after_index_subscript(name, index, AfterIdentEnd::ForStep)?
            } else {
                self.parse_stmt_after_ident(name, AfterIdentEnd::ForStep)?
            };
            return Ok(Some(Box::new(s)));
        }
        Err(self
            .error("expected `)` or assignment / `+=` / `++` in for-step")
            .into())
    }

    /// After consuming the identifier: `=`, `+=`, or `++`, then `;` or `)`.
    fn parse_stmt_after_ident(
        &mut self,
        name: String,
        end: AfterIdentEnd,
    ) -> Result<Stmt, FrontEndError> {
        let stmt = match self.peek().clone() {
            Token::Assign => {
                self.bump();
                let value = self.parse_expr()?;
                Stmt::Assign { name, value }
            }
            Token::PlusAssign => {
                self.bump();
                let rhs = self.parse_expr()?;
                Stmt::AddAssign { name, rhs }
            }
            Token::PlusPlus => {
                self.bump();
                Stmt::PostInc { name }
            }
            other => {
                return Err(self
                    .error(format!(
                        "expected `=`, `+=`, or `++` after identifier, found `{other:?}`"
                    ))
                    .into());
            }
        };
        match end {
            AfterIdentEnd::Semicolon => self.consume(Token::Semicolon, "`;`")?,
            AfterIdentEnd::ForStep => {}
        }
        Ok(stmt)
    }

    fn parse_stmt_after_index_subscript(
        &mut self,
        base: String,
        index: Expr,
        end: AfterIdentEnd,
    ) -> Result<Stmt, FrontEndError> {
        match self.peek().clone() {
            Token::Assign => {
                self.bump();
                let value = self.parse_expr()?;
                let stmt = Stmt::IndexAssign {
                    base,
                    index,
                    value,
                };
                match end {
                    AfterIdentEnd::Semicolon => self.consume(Token::Semicolon, "`;`")?,
                    AfterIdentEnd::ForStep => {}
                }
                Ok(stmt)
            }
            other => Err(self
                .error(format!(
                    "expected `=` after `a[i]`, found `{other:?}`"
                ))
                .into()),
        }
    }

    fn parse_block(&mut self) -> Result<Vec<Stmt>, FrontEndError> {
        self.consume(Token::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        while !matches!(self.peek(), Token::RBrace | Token::Eof) {
            stmts.push(self.parse_stmt()?);
        }
        self.consume(Token::RBrace, "`}`")?;
        Ok(stmts)
    }

    fn parse_expr(&mut self) -> Result<Expr, FrontEndError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_and()?;
        while matches!(self.peek(), Token::OrOr) {
            self.bump();
            let r = self.parse_and()?;
            e = Expr::Binary(BinOp::Or, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_and(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_eq()?;
        while matches!(self.peek(), Token::AndAnd) {
            self.bump();
            let r = self.parse_eq()?;
            e = Expr::Binary(BinOp::And, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_eq(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_rel()?;
        loop {
            let op = match self.peek() {
                Token::EqEq => Some(BinOp::Eq),
                Token::Ne => Some(BinOp::Ne),
                _ => None,
            };
            let Some(op) = op else { break };
            self.bump();
            let r = self.parse_rel()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_rel(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_add()?;
        loop {
            let op = match self.peek() {
                Token::Lt => Some(BinOp::Lt),
                Token::Le => Some(BinOp::Le),
                Token::Gt => Some(BinOp::Gt),
                Token::Ge => Some(BinOp::Ge),
                _ => None,
            };
            let Some(op) = op else { break };
            self.bump();
            let r = self.parse_add()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_add(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                Token::Plus => Some(BinOp::Add),
                Token::Minus => Some(BinOp::Sub),
                _ => None,
            };
            let Some(op) = op else { break };
            self.bump();
            let r = self.parse_mul()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_mul(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Token::Star => Some(BinOp::Mul),
                Token::Slash => Some(BinOp::Div),
                _ => None,
            };
            let Some(op) = op else { break };
            self.bump();
            let r = self.parse_unary()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }

    fn parse_unary(&mut self) -> Result<Expr, FrontEndError> {
        if matches!(self.peek(), Token::Minus) {
            self.bump();
            return Ok(Expr::Unary(UnaryOp::Neg, Box::new(self.parse_unary()?)));
        }
        if matches!(self.peek(), Token::Bang) {
            self.bump();
            return Ok(Expr::Unary(UnaryOp::Not, Box::new(self.parse_unary()?)));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, FrontEndError> {
        let mut e = self.parse_primary()?;
        loop {
            if matches!(self.peek(), Token::LBracket) {
                self.bump();
                let ix = self.parse_expr()?;
                self.consume(Token::RBracket, "`]`")?;
                match e {
                    Expr::Var(name) => {
                        e = Expr::Index {
                            base: name,
                            index: Box::new(ix),
                        };
                    }
                    _ => {
                        return Err(self
                            .error("`[` only applies to an array variable")
                            .into());
                    }
                }
                continue;
            }
            if matches!(self.peek(), Token::PlusPlus) {
                self.bump();
                match e {
                    Expr::Var(name) => {
                        e = Expr::PostInc(name);
                    }
                    _ => {
                        return Err(self
                            .error("`++` postfix only applies to a variable")
                            .into());
                    }
                }
                continue;
            }
            break;
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> Result<Expr, FrontEndError> {
        match self.peek().clone() {
            Token::IntLit(n) => {
                self.bump();
                Ok(Expr::IntLit(n))
            }
            Token::CharLit(b) => {
                self.bump();
                Ok(Expr::CharLit(b))
            }
            Token::StringLit(s) => {
                self.bump();
                Ok(Expr::StringLit(s))
            }
            Token::KwTrue => {
                self.bump();
                Ok(Expr::BoolLit(true))
            }
            Token::KwFalse => {
                self.bump();
                Ok(Expr::BoolLit(false))
            }
            Token::Ident(s) => {
                self.bump();
                Ok(Expr::Var(s))
            }
            Token::LParen => {
                self.bump();
                let e = self.parse_expr()?;
                self.consume(Token::RParen, "`)`")?;
                Ok(e)
            }
            other => Err(self
                .error(format!("expected expression, found `{other:?}`"))
                .into()),
        }
    }

    fn expect_ident(&mut self) -> Result<String, FrontEndError> {
        match self.bump() {
            Token::Ident(s) => Ok(s),
            other => Err(self.error(format!("expected identifier, found `{other:?}`")).into()),
        }
    }

    fn expect_string_lit(&mut self) -> Result<std::vec::Vec<u8>, FrontEndError> {
        match self.bump() {
            Token::StringLit(bytes) => Ok(bytes),
            other => Err(self
                .error(format!("expected string literal, found `{other:?}`"))
                .into()),
        }
    }

    fn consume(&mut self, expected: Token, label: &'static str) -> Result<(), FrontEndError> {
        let next = self.peek().clone();
        if next == expected {
            self.bump();
            Ok(())
        } else {
            Err(self
                .error(format!("expected {label}, found `{next:?}`"))
                .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_program;
    use crate::ast::{BinOp, Expr, Stmt, Ty};

    #[test]
    fn empty_program() {
        let p = parse_program("").unwrap();
        assert!(p.stmts.is_empty());
    }

    #[test]
    fn decl_and_assign() {
        let p = parse_program("int x = 1; x = x + 2;").unwrap();
        assert_eq!(p.stmts.len(), 2);
    }

    #[test]
    fn array_decl_and_index_expr() {
        let p = parse_program("int a[3]; a[1] = 42; print_int(a[1]);").unwrap();
        assert!(matches!(&p.stmts[0], Stmt::ArrayDecl { len: 3, .. }));
        assert!(matches!(&p.stmts[1], Stmt::IndexAssign { .. }));
    }

    #[test]
    fn char_array_from_string_literal() {
        let p = parse_program(r#"char s[] = "hi";"#).unwrap();
        match &p.stmts[0] {
            Stmt::CharArrayFromString { bytes, .. } => assert_eq!(bytes, &[b'h', b'i', 0]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn print_string_stmt() {
        parse_program(r#"print_string("x");"#).unwrap();
    }

    #[test]
    fn int_array_literal_trailing_comma() {
        let p = parse_program("int arr[] = {1, 2, 3,};").unwrap();
        match &p.stmts[0] {
            Stmt::ArrayFromExprs {
                elem_ty: Ty::Int,
                elements,
                ..
            } => assert_eq!(elements.len(), 3),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn bool_array_literal() {
        let p = parse_program("bool f[] = { true, false };").unwrap();
        match &p.stmts[0] {
            Stmt::ArrayFromExprs {
                elem_ty: Ty::Bool,
                elements,
                ..
            } => assert_eq!(elements.len(), 2),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn post_inc_and_add_assign_stmts() {
        let p = parse_program("int i = 0; i++; i += 1;").unwrap();
        assert!(matches!(&p.stmts[1], Stmt::PostInc { .. }));
        assert!(matches!(&p.stmts[2], Stmt::AddAssign { .. }));
    }

    #[test]
    fn post_inc_in_expr() {
        let p = parse_program("int i = 0; int j = i++;").unwrap();
        let Stmt::VarDecl { init, .. } = &p.stmts[1] else {
            panic!("expected var decl");
        };
        assert!(matches!(init, Expr::PostInc(_)));
    }

    #[test]
    fn for_step_post_inc() {
        let p = parse_program("for (int i = 0; i < 2; i++) { }").unwrap();
        assert!(matches!(&p.stmts[0], Stmt::For { step: Some(b), .. } if matches!(**b, Stmt::PostInc { .. })));
    }

    #[test]
    fn mul_binds_tighter_than_add() {
        let p = parse_program("int x = 1 + 2 * 3;").unwrap();
        let Stmt::VarDecl { init, .. } = &p.stmts[0] else {
            panic!("expected var decl");
        };
        assert_eq!(
            init,
            &Expr::Binary(
                BinOp::Add,
                Box::new(Expr::IntLit(1)),
                Box::new(Expr::Binary(
                    BinOp::Mul,
                    Box::new(Expr::IntLit(2)),
                    Box::new(Expr::IntLit(3)),
                )),
            )
        );
    }

    #[test]
    fn if_else_branch() {
        let p = parse_program("if (true) int x = 1; else { x = 2; }").unwrap();
        assert!(matches!(&p.stmts[0], Stmt::If { else_branch: Some(_), .. }));
    }

    #[test]
    fn bool_decl() {
        let p = parse_program("bool b = false;").unwrap();
        match &p.stmts[0] {
            Stmt::VarDecl { ty: Ty::Bool, init: Expr::BoolLit(false), .. } => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn line_comment_skipped() {
        let p = parse_program("// init\nint x = 0;").unwrap();
        assert_eq!(p.stmts.len(), 1);
    }

    #[test]
    fn or_is_left_associative_in_ast_shape() {
        let p = parse_program("bool b = true || false || false;").unwrap();
        let Stmt::VarDecl { init, .. } = &p.stmts[0] else {
            panic!("expected var decl");
        };
        assert!(matches!(
            init,
            Expr::Binary(BinOp::Or, _, _) | Expr::BoolLit(_)
        ));
        // (true || false) || false  →  outer Or
        let Expr::Binary(BinOp::Or, l, _) = init else {
            panic!("expected or");
        };
        assert!(matches!(**l, Expr::Binary(BinOp::Or, _, _)));
    }
}
