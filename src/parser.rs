//! Recursive-descent parser for the README grammar (`program` → `stmt*` → `expr` with C-like precedence).

use crate::ast::{BinOp, Expr, Item, Program, RetTy, Stmt, Ty, UnaryOp};
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
        let mut items = Vec::new();
        while !matches!(self.peek(), Token::Eof) {
            items.push(self.parse_item()?);
        }
        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, FrontEndError> {
        match self.peek().clone() {
            Token::KwVoid => {
                self.bump();
                let name = self.expect_ident()?;
                self.consume(Token::LParen, "`(`")?;
                let params = self.parse_param_list()?;
                self.consume(Token::RParen, "`)`")?;
                match self.peek().clone() {
                    Token::Semicolon => {
                        self.bump();
                        Ok(Item::FnDecl {
                            name,
                            params,
                            ret: RetTy::Void,
                        })
                    }
                    Token::LBrace => {
                        let body = self.parse_block()?;
                        Ok(Item::FnDef {
                            name,
                            params,
                            ret: RetTy::Void,
                            body,
                        })
                    }
                    other => Err(self
                        .error(format!(
                            "expected `;` or `{{` after `void {name}(...)`, found `{other:?}`"
                        ))
                        .into()),
                }
            }
            Token::KwStruct => {
                self.bump();
                let tag = self.expect_ident()?;
                match self.peek().clone() {
                    Token::LBrace => {
                        let fields = self.parse_struct_fields()?;
                        self.consume(Token::Semicolon, "`;`")?;
                        Ok(Item::StructDef { name: tag, fields })
                    }
                    Token::Ident(id) => {
                        self.bump();
                        if matches!(self.peek(), Token::LParen) {
                            self.bump();
                            let params = self.parse_param_list()?;
                            self.consume(Token::RParen, "`)`")?;
                            let ret = RetTy::Scalar(Ty::Struct(tag));
                            match self.peek().clone() {
                                Token::Semicolon => {
                                    self.bump();
                                    Ok(Item::FnDecl {
                                        name: id,
                                        params,
                                        ret,
                                    })
                                }
                                Token::LBrace => {
                                    let body = self.parse_block()?;
                                    Ok(Item::FnDef {
                                        name: id,
                                        params,
                                        ret,
                                        body,
                                    })
                                }
                                other => Err(self
                                    .error(format!(
                                        "expected `;` or `{{` after function header, found `{other:?}`"
                                    ))
                                    .into()),
                            }
                        } else {
                            self.consume(Token::Semicolon, "`;`")?;
                            Ok(Item::Stmt(Stmt::VarDecl {
                                name: id,
                                ty: Ty::Struct(tag),
                                init: None,
                            }))
                        }
                    }
                    other => Err(self
                        .error(format!(
                            "expected `{{` or identifier after `struct {tag}`, found `{other:?}`"
                        ))
                        .into()),
                }
            }
            Token::KwInt | Token::KwBool | Token::KwChar => {
                let ty = match self.peek() {
                    Token::KwInt => Ty::Int,
                    Token::KwBool => Ty::Bool,
                    Token::KwChar => Ty::Char,
                    _ => unreachable!(),
                };
                self.bump();
                let name = self.expect_ident()?;
                if matches!(self.peek(), Token::LParen) {
                    self.bump();
                    let params = self.parse_param_list()?;
                    self.consume(Token::RParen, "`)`")?;
                    let ret = RetTy::Scalar(ty);
                    match self.peek().clone() {
                        Token::Semicolon => {
                            self.bump();
                            Ok(Item::FnDecl {
                                name,
                                params,
                                ret,
                            })
                        }
                        Token::LBrace => {
                            let body = self.parse_block()?;
                            Ok(Item::FnDef {
                                name,
                                params,
                                ret,
                                body,
                            })
                        }
                        other => Err(self
                            .error(format!(
                                "expected `;` or `{{` after function header, found `{other:?}`"
                            ))
                            .into()),
                    }
                } else {
                    let stmt = self.decl_tail_for_var(ty, name)?;
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(Item::Stmt(stmt))
                }
            }
            _ => Ok(Item::Stmt(self.parse_stmt()?)),
        }
    }

    fn parse_struct_fields(&mut self) -> Result<Vec<(String, Ty)>, FrontEndError> {
        self.consume(Token::LBrace, "`{`")?;
        let mut fields = Vec::new();
        if matches!(self.peek(), Token::RBrace) {
            return Err(self.error("struct must have at least one member").into());
        }
        loop {
            let base_ty = self.parse_type()?;
            let fname = self.expect_ident()?;
            let ty = self.maybe_array_suffix_after_declarator(base_ty)?;
            self.consume(Token::Semicolon, "`;`")?;
            fields.push((fname, ty));
            match self.peek().clone() {
                Token::RBrace => {
                    self.bump();
                    break;
                }
                _ => {}
            }
        }
        Ok(fields)
    }

    fn parse_type(&mut self) -> Result<Ty, FrontEndError> {
        match self.peek().clone() {
            Token::KwInt => {
                self.bump();
                Ok(Ty::Int)
            }
            Token::KwBool => {
                self.bump();
                Ok(Ty::Bool)
            }
            Token::KwChar => {
                self.bump();
                Ok(Ty::Char)
            }
            Token::KwStruct => {
                self.bump();
                Ok(Ty::Struct(self.expect_ident()?))
            }
            other => Err(self
                .error(format!(
                    "expected type (`int`, `bool`, `char`, or `struct Tag`), found `{other:?}`"
                ))
                .into()),
        }
    }

    /// After the field name: optional **`[n]`** for a fixed array member (same as `int x[3];`).
    fn maybe_array_suffix_after_declarator(&mut self, base: Ty) -> Result<Ty, FrontEndError> {
        if matches!(self.peek(), Token::LBracket) {
            self.bump();
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
            if matches!(self.peek(), Token::LBracket) {
                return Err(self
                    .error("nested array fields (`[][]`) are not supported")
                    .into());
            }
            Ok(Ty::Array(Box::new(base), len))
        } else {
            Ok(base)
        }
    }

    fn parse_param_ty(&mut self) -> Result<Ty, FrontEndError> {
        self.parse_type()
    }

    fn parse_param_list(&mut self) -> Result<Vec<(String, Ty)>, FrontEndError> {
        let mut params = Vec::new();
        if matches!(self.peek(), Token::RParen) {
            return Ok(params);
        }
        loop {
            let ty = self.parse_param_ty()?;
            let pname = self.expect_ident()?;
            params.push((pname, ty));
            match self.peek().clone() {
                Token::Comma => {
                    self.bump();
                    if matches!(self.peek(), Token::RParen) {
                        return Err(self
                            .error("trailing `,` in parameter list is not allowed")
                            .into());
                    }
                }
                Token::RParen => break,
                other => {
                    return Err(self
                        .error(format!(
                            "expected `,` or `)` in parameter list, found `{other:?}`"
                        ))
                        .into());
                }
            }
        }
        Ok(params)
    }

    fn parse_stmt(&mut self) -> Result<Stmt, FrontEndError> {
        match self.peek().clone() {
            Token::KwStruct => {
                self.bump();
                let tag = self.expect_ident()?;
                let name = self.expect_ident()?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::VarDecl {
                    name,
                    ty: Ty::Struct(tag),
                    init: None,
                })
            }
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
            Token::KwReturn => {
                self.bump();
                if matches!(self.peek(), Token::Semicolon) {
                    self.bump();
                    return Ok(Stmt::Return(None));
                }
                let e = self.parse_expr()?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Stmt::Return(Some(e)))
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
                if matches!(self.peek(), Token::LParen) {
                    self.bump();
                    let args = self.parse_call_arg_list()?;
                    self.consume(Token::RParen, "`)`")?;
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(Stmt::Expr(Expr::Call { name, args }))
                } else {
                    self.parse_stmt_after_ident(name, AfterIdentEnd::Semicolon)
                }
            }
            other => Err(self
                .error(format!("statement cannot start with `{other:?}`"))
                .into()),
        }
    }

    /// After `int` / `bool` / `char` in a statement: `name = expr`, `name [ N ]`, etc.
    fn parse_decl_tail_after_type(&mut self, elem: Ty) -> Result<Stmt, FrontEndError> {
        let name = self.expect_ident()?;
        self.decl_tail_for_var(elem, name)
    }

    /// Same as [`Self::parse_decl_tail_after_type`] but **`name`** already consumed.
    fn decl_tail_for_var(&mut self, elem: Ty, name: String) -> Result<Stmt, FrontEndError> {
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
            init: Some(init),
        })
    }

    fn parse_member_chain(&mut self, mut e: Expr) -> Result<Expr, FrontEndError> {
        while matches!(self.peek(), Token::Dot) {
            self.bump();
            let field = self.expect_ident()?;
            e = Expr::Member {
                base: Box::new(e),
                field,
            };
        }
        Ok(e)
    }

    fn parse_call_arg_list(&mut self) -> Result<Vec<Expr>, FrontEndError> {
        let mut args = Vec::new();
        if matches!(self.peek(), Token::RParen) {
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            match self.peek().clone() {
                Token::Comma => {
                    self.bump();
                    if matches!(self.peek(), Token::RParen) {
                        return Err(self
                            .error("trailing `,` in argument list is not allowed")
                            .into());
                    }
                }
                Token::RParen => break,
                other => {
                    return Err(self
                        .error(format!(
                            "expected `,` or `)` in argument list, found `{other:?}`"
                        ))
                        .into());
                }
            }
        }
        Ok(args)
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
            Token::KwStruct => {
                self.bump();
                let tag = self.expect_ident()?;
                let name = self.expect_ident()?;
                Ok(Stmt::VarDecl {
                    name,
                    ty: Ty::Struct(tag),
                    init: None,
                })
            }
            other => Err(self
                .error(format!("expected `int`, `bool`, `char`, or `struct` in for-init, found `{other:?}`"))
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
            Token::KwInt | Token::KwBool | Token::KwChar | Token::KwStruct
        ) {
            let stmt = self.parse_var_decl_stmt()?;
            self.consume(Token::Semicolon, "`;`")?;
            return Ok(Some(Box::new(stmt)));
        }
        if let Token::Ident(name) = self.peek().clone() {
            self.bump();
            let s = self.parse_stmt_after_ident(name, AfterIdentEnd::Semicolon)?;
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
            let s = self.parse_stmt_after_ident(name, AfterIdentEnd::ForStep)?;
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
        let mut lhs = Expr::Var(name.clone());
        if matches!(self.peek(), Token::LBracket) {
            self.bump();
            let index = self.parse_expr()?;
            self.consume(Token::RBracket, "`]`")?;
            lhs = Expr::Index {
                base: Box::new(Expr::Var(name)),
                index: Box::new(index),
            };
        }
        lhs = self.parse_member_chain(lhs)?;
        while matches!(self.peek(), Token::LBracket) {
            self.bump();
            let index = self.parse_expr()?;
            self.consume(Token::RBracket, "`]`")?;
            lhs = Expr::Index {
                base: Box::new(lhs),
                index: Box::new(index),
            };
        }
        let stmt = match self.peek().clone() {
            Token::Assign => {
                self.bump();
                let value = self.parse_expr()?;
                Stmt::Assign { target: lhs, value }
            }
            Token::PlusAssign => {
                self.bump();
                let rhs = self.parse_expr()?;
                Stmt::AddAssign { target: lhs, rhs }
            }
            Token::PlusPlus => {
                self.bump();
                Stmt::PostInc { target: lhs }
            }
            other => {
                return Err(self
                    .error(format!(
                        "expected `=`, `+=`, or `++` after lvalue, found `{other:?}`"
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
            if matches!(self.peek(), Token::LParen) {
                let callee = match e {
                    Expr::Var(name) => name,
                    _ => {
                        return Err(self
                            .error("call `f(...)` requires a plain function name")
                            .into());
                    }
                };
                self.bump();
                let args = self.parse_call_arg_list()?;
                self.consume(Token::RParen, "`)`")?;
                e = Expr::Call {
                    name: callee,
                    args,
                };
                continue;
            }
            if matches!(self.peek(), Token::LBracket) {
                self.bump();
                let ix = self.parse_expr()?;
                self.consume(Token::RBracket, "`]`")?;
                match e {
                    Expr::Var(_) | Expr::Member { .. } => {
                        e = Expr::Index {
                            base: Box::new(e),
                            index: Box::new(ix),
                        };
                    }
                    _ => {
                        return Err(self
                            .error("`[` only applies to an array variable or struct array member")
                            .into());
                    }
                }
                continue;
            }
            if matches!(self.peek(), Token::Dot) {
                self.bump();
                let field = self.expect_ident()?;
                e = Expr::Member {
                    base: Box::new(e),
                    field,
                };
                continue;
            }
            if matches!(self.peek(), Token::PlusPlus) {
                self.bump();
                if !matches!(e, Expr::Var(_) | Expr::Index { .. } | Expr::Member { .. }) {
                    return Err(self
                        .error("`++` postfix requires a variable, subscript, or member lvalue")
                        .into());
                }
                e = Expr::PostInc(Box::new(e));
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
    use crate::ast::{BinOp, Expr, Item, Stmt, Ty};

    #[test]
    fn empty_program() {
        let p = parse_program("").unwrap();
        assert!(p.items.is_empty());
    }

    #[test]
    fn decl_and_assign() {
        let p = parse_program("int x = 1; x = x + 2;").unwrap();
        assert_eq!(p.items.len(), 2);
    }

    #[test]
    fn array_decl_and_index_expr() {
        let p = parse_program("int a[3]; a[1] = 42; print_int(a[1]);").unwrap();
        assert!(matches!(&p.items[0], Item::Stmt(Stmt::ArrayDecl { len: 3, .. })));
        assert!(matches!(
            &p.items[1],
            Item::Stmt(Stmt::Assign {
                target: Expr::Index { .. },
                ..
            })
        ));
        assert!(matches!(
            &p.items[2],
            Item::Stmt(Stmt::Expr(Expr::Call { name, .. })) if name == "print_int"
        ));
    }

    #[test]
    fn char_array_from_string_literal() {
        let p = parse_program(r#"char s[] = "hi";"#).unwrap();
        match &p.items[0] {
            Item::Stmt(Stmt::CharArrayFromString { bytes, .. }) => {
                assert_eq!(bytes, &[b'h', b'i', 0])
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn print_string_stmt() {
        parse_program(r#"print_string("x");"#).unwrap();
    }

    #[test]
    fn function_forward_and_def() {
        let p = parse_program(
            "int f(int x);\n\
             int f(int x) { return x; }\n\
             print_int(f(5));",
        )
        .unwrap();
        assert!(matches!(&p.items[0], Item::FnDecl { .. }));
        assert!(matches!(&p.items[1], Item::FnDef { .. }));
        assert!(matches!(&p.items[2], Item::Stmt(_)));
    }

    #[test]
    fn void_function_empty_params() {
        let p = parse_program("void g() { return; }").unwrap();
        let Item::FnDef { body, .. } = &p.items[0] else {
            panic!("expected fn def");
        };
        assert!(matches!(body.as_slice(), [Stmt::Return(None)]));
    }

    #[test]
    fn int_array_literal_trailing_comma() {
        let p = parse_program("int arr[] = {1, 2, 3,};").unwrap();
        match &p.items[0] {
            Item::Stmt(Stmt::ArrayFromExprs {
                elem_ty: Ty::Int,
                elements,
                ..
            }) => assert_eq!(elements.len(), 3),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn bool_array_literal() {
        let p = parse_program("bool f[] = { true, false };").unwrap();
        match &p.items[0] {
            Item::Stmt(Stmt::ArrayFromExprs {
                elem_ty: Ty::Bool,
                elements,
                ..
            }) => assert_eq!(elements.len(), 2),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn post_inc_and_add_assign_stmts() {
        let p = parse_program("int i = 0; i++; i += 1;").unwrap();
        assert!(matches!(&p.items[1], Item::Stmt(Stmt::PostInc { .. })));
        assert!(matches!(&p.items[2], Item::Stmt(Stmt::AddAssign { .. })));
    }

    #[test]
    fn post_inc_in_expr() {
        let p = parse_program("int i = 0; int j = i++;").unwrap();
        let Item::Stmt(Stmt::VarDecl { init, .. }) = &p.items[1] else {
            panic!("expected var decl");
        };
        assert!(matches!(init, Some(Expr::PostInc(_))));
    }

    #[test]
    fn for_step_post_inc() {
        let p = parse_program("for (int i = 0; i < 2; i++) { }").unwrap();
        assert!(matches!(&p.items[0], Item::Stmt(Stmt::For { step: Some(b), .. }) if matches!(**b, Stmt::PostInc { .. })));
    }

    #[test]
    fn mul_binds_tighter_than_add() {
        let p = parse_program("int x = 1 + 2 * 3;").unwrap();
        let Item::Stmt(Stmt::VarDecl { init, .. }) = &p.items[0] else {
            panic!("expected var decl");
        };
        assert_eq!(
            init.as_ref(),
            Some(&Expr::Binary(
                BinOp::Add,
                Box::new(Expr::IntLit(1)),
                Box::new(Expr::Binary(
                    BinOp::Mul,
                    Box::new(Expr::IntLit(2)),
                    Box::new(Expr::IntLit(3)),
                )),
            ))
        );
    }

    #[test]
    fn if_else_branch() {
        let p = parse_program("if (true) int x = 1; else { x = 2; }").unwrap();
        assert!(matches!(&p.items[0], Item::Stmt(Stmt::If { else_branch: Some(_), .. })));
    }

    #[test]
    fn bool_decl() {
        let p = parse_program("bool b = false;").unwrap();
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl {
                ty: Ty::Bool,
                init: Some(Expr::BoolLit(false)),
                ..
            }) => {}
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn line_comment_skipped() {
        let p = parse_program("// init\nint x = 0;").unwrap();
        assert_eq!(p.items.len(), 1);
    }

    #[test]
    fn struct_def_with_array_field() {
        let p = parse_program(
            "struct S { int xs[2]; bool ok; };\n\
             struct S s;\n\
             s.xs[0] = 9;\n\
             s.ok = true;",
        )
        .unwrap();
        let Item::StructDef { fields, .. } = &p.items[0] else {
            panic!("expected struct def");
        };
        assert!(matches!(
            fields.as_slice(),
            [
                (_, Ty::Array(e, 2)),
                (_, Ty::Bool),
            ] if **e == Ty::Int
        ));
    }

    #[test]
    fn or_is_left_associative_in_ast_shape() {
        let p = parse_program("bool b = true || false || false;").unwrap();
        let Item::Stmt(Stmt::VarDecl { init, .. }) = &p.items[0] else {
            panic!("expected var decl");
        };
        assert!(matches!(
            init.as_ref(),
            Some(Expr::Binary(BinOp::Or, _, _)) | Some(Expr::BoolLit(_))
        ));
        // (true || false) || false  →  outer Or
        let Some(Expr::Binary(BinOp::Or, l, _)) = init.as_ref() else {
            panic!("expected or");
        };
        assert!(matches!(**l, Expr::Binary(BinOp::Or, _, _)));
    }
}
