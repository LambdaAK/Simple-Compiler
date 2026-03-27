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
                let name = self.expect_ident()?;
                self.consume(Token::Assign, "`=`")?;
                let init = self.parse_expr()?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::VarDecl {
                    name,
                    ty: Ty::Int,
                    init,
                })
            }
            Token::KwBool => {
                self.bump();
                let name = self.expect_ident()?;
                self.consume(Token::Assign, "`=`")?;
                let init = self.parse_expr()?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::VarDecl {
                    name,
                    ty: Ty::Bool,
                    init,
                })
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
            Token::LBrace => Ok(Stmt::Block(self.parse_block()?)),
            Token::Ident(name) => {
                self.bump();
                self.consume(Token::Assign, "`=`")?;
                let value = self.parse_expr()?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::Assign { name, value })
            }
            other => Err(self
                .error(format!("statement cannot start with `{other:?}`"))
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
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, FrontEndError> {
        match self.peek().clone() {
            Token::IntLit(n) => {
                self.bump();
                Ok(Expr::IntLit(n))
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
