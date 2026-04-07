//! Recursive-descent parser for the README grammar (`program` → `stmt*` → `expr` with C-like precedence).

use std::collections::HashMap;

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
    /// `typedef` aliases visible for the rest of the parse (file scope).
    type_aliases: HashMap<String, Ty>,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Self {
            tokens,
            pos: 0,
            type_aliases: HashMap::new(),
        }
    }

    fn peek_is_typedef_name(&self) -> bool {
        matches!(self.peek(), Token::Ident(n) if self.type_aliases.contains_key(n))
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
            if matches!(self.peek(), Token::KwTypedef) {
                self.parse_typedef()?;
            } else {
                items.push(self.parse_item()?);
            }
        }
        Ok(Program { items })
    }

    /// `typedef <type> <name> ['[' n ']'] ;`
    fn parse_typedef(&mut self) -> Result<(), FrontEndError> {
        self.consume(Token::KwTypedef, "`typedef`")?;
        let base = self.parse_type()?;
        let alias = self.expect_ident()?;
        let ty = self.maybe_array_suffix_after_declarator(base)?;
        if self.type_aliases.insert(alias.clone(), ty).is_some() {
            return Err(self
                .error(format!("duplicate typedef `{alias}`"))
                .into());
        }
        self.consume(Token::Semicolon, "`;`")?;
        Ok(())
    }

    /// After a concrete or aliased type keyword / prefix: function vs global / block variable decl.
    fn parse_item_after_ty(&mut self, ty: Ty) -> Result<Item, FrontEndError> {
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
            if matches!(ty, Ty::Struct(_)) {
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Item::Stmt(Stmt::VarDecl {
                    name,
                    ty,
                    init: None,
                }))
            } else {
                let stmt = self.decl_tail_for_var(ty, name)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(Item::Stmt(stmt))
            }
        }
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
                if matches!(self.peek(), Token::LBrace) {
                    let fields = self.parse_struct_fields()?;
                    self.consume(Token::Semicolon, "`;`")?;
                    return Ok(Item::StructDef { name: tag, fields });
                }
                let mut ty = Ty::Struct(tag);
                while matches!(self.peek(), Token::Star) {
                    self.bump();
                    ty = Ty::Ptr(Box::new(ty));
                }
                self.parse_item_after_ty(ty)
            }
            Token::KwInt | Token::KwBool | Token::KwChar => {
                let ty = self.parse_type()?;
                self.parse_item_after_ty(ty)
            }
            Token::Ident(id) if self.type_aliases.contains_key(&id) => {
                let ty = self.type_aliases.get(&id).expect("checked").clone();
                self.bump();
                self.parse_item_after_ty(ty)
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
        let mut ty = self.parse_type_base()?;
        while matches!(self.peek(), Token::Star) {
            self.bump();
            ty = Ty::Ptr(Box::new(ty));
        }
        Ok(ty)
    }

    fn parse_type_base(&mut self) -> Result<Ty, FrontEndError> {
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
            Token::Ident(name) => {
                if let Some(ty) = self.type_aliases.get(&name).cloned() {
                    self.bump();
                    Ok(ty)
                } else {
                    Err(self
                        .error(format!(
                            "`{name}` is not a type (unknown identifier in type position)"
                        ))
                        .into())
                }
            }
            other => Err(self
                .error(format!(
                    "expected type (`int`, `bool`, `char`, `struct Tag`, or typedef name), found `{other:?}`"
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
                let mut ty = Ty::Struct(tag);
                while matches!(self.peek(), Token::Star) {
                    self.bump();
                    ty = Ty::Ptr(Box::new(ty));
                }
                let name = self.expect_ident()?;
                if matches!(ty, Ty::Struct(_)) {
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(Stmt::VarDecl {
                        name,
                        ty,
                        init: None,
                    })
                } else {
                    let s = self.decl_tail_for_var(ty, name)?;
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(s)
                }
            }
            Token::KwInt => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let s = self.decl_tail_for_var(ty, name)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(s)
            }
            Token::KwBool => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let s = self.decl_tail_for_var(ty, name)?;
                self.consume(Token::Semicolon, "`;`")?;
                Ok(s)
            }
            Token::KwChar => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                let s = self.decl_tail_for_var(ty, name)?;
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
            Token::Star => {
                self.bump();
                let ptr_inner = self.parse_unary()?;
                let lhs = Expr::Unary(UnaryOp::Deref, Box::new(ptr_inner));
                let stmt = match self.peek().clone() {
                    Token::Assign => {
                        self.bump();
                        let value = self.parse_expr()?;
                        Stmt::Assign {
                            target: lhs,
                            value,
                        }
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
                                "expected `=`, `+=`, or `++` after `*lvalue`, found `{other:?}`"
                            ))
                            .into());
                    }
                };
                self.consume(Token::Semicolon, "`;`")?;
                Ok(stmt)
            }
            Token::Ident(ref id) if self.type_aliases.contains_key(id) => {
                let ty = self.type_aliases.get(id).expect("checked").clone();
                self.bump();
                let vname = self.expect_ident()?;
                if matches!(ty, Ty::Struct(_)) {
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(Stmt::VarDecl {
                        name: vname,
                        ty,
                        init: None,
                    })
                } else {
                    let s = self.decl_tail_for_var(ty, vname)?;
                    self.consume(Token::Semicolon, "`;`")?;
                    Ok(s)
                }
            }
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

    /// After `int x` / `int *p` / …: `=`, `[ ]`, `;` (pointers may omit `=`).
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
        if matches!(self.peek(), Token::Semicolon) {
            if matches!(elem, Ty::Ptr(_)) {
                return Ok(Stmt::VarDecl {
                    name,
                    ty: elem,
                    init: None,
                });
            }
            return Err(self
                .error("scalar variables require an initializer or array/`[]` syntax")
                .into());
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
        loop {
            if matches!(self.peek(), Token::Dot) {
                self.bump();
                let field = self.expect_ident()?;
                e = Expr::Member {
                    base: Box::new(e),
                    field,
                };
            } else if matches!(self.peek(), Token::Arrow) {
                self.bump();
                let field = self.expect_ident()?;
                e = Expr::Member {
                    base: Box::new(Expr::Unary(UnaryOp::Deref, Box::new(e))),
                    field,
                };
            } else {
                break;
            }
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
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                self.decl_tail_for_var(ty, name)
            }
            Token::KwBool => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                self.decl_tail_for_var(ty, name)
            }
            Token::KwChar => {
                let ty = self.parse_type()?;
                let name = self.expect_ident()?;
                self.decl_tail_for_var(ty, name)
            }
            Token::KwStruct => {
                self.bump();
                let tag = self.expect_ident()?;
                let mut ty = Ty::Struct(tag);
                while matches!(self.peek(), Token::Star) {
                    self.bump();
                    ty = Ty::Ptr(Box::new(ty));
                }
                let name = self.expect_ident()?;
                if matches!(ty, Ty::Struct(_)) {
                    Ok(Stmt::VarDecl {
                        name,
                        ty,
                        init: None,
                    })
                } else {
                    self.decl_tail_for_var(ty, name)
                }
            }
            Token::Ident(ref id) if self.type_aliases.contains_key(id) => {
                let ty = self.type_aliases.get(id).expect("checked").clone();
                self.bump();
                let vname = self.expect_ident()?;
                if matches!(ty, Ty::Struct(_)) {
                    Ok(Stmt::VarDecl {
                        name: vname,
                        ty,
                        init: None,
                    })
                } else {
                    self.decl_tail_for_var(ty, vname)
                }
            }
            other => Err(self
                .error(format!(
                    "expected `int`, `bool`, `char`, `struct`, or typedef name in for-init, found `{other:?}`"
                ))
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
        ) || self.peek_is_typedef_name()
        {
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
                Token::Percent => Some(BinOp::Mod),
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
        if matches!(self.peek(), Token::Ampersand) {
            self.bump();
            return Ok(Expr::Unary(UnaryOp::Addr, Box::new(self.parse_unary()?)));
        }
        if matches!(self.peek(), Token::Star) {
            self.bump();
            return Ok(Expr::Unary(UnaryOp::Deref, Box::new(self.parse_unary()?)));
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
            if matches!(self.peek(), Token::Arrow) {
                self.bump();
                let field = self.expect_ident()?;
                e = Expr::Member {
                    base: Box::new(Expr::Unary(UnaryOp::Deref, Box::new(e))),
                    field,
                };
                continue;
            }
            if matches!(self.peek(), Token::PlusPlus) {
                self.bump();
                if !matches!(
                    e,
                    Expr::Var(_) | Expr::Index { .. } | Expr::Member { .. }
                        | Expr::Unary(UnaryOp::Deref, _)
                ) {
                    return Err(self
                        .error(
                            "`++` postfix requires a variable, subscript, member, or `*ptr` lvalue",
                        )
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
    use crate::ast::{BinOp, Expr, Item, Stmt, Ty, UnaryOp};

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
    fn pointer_decl_and_star_assign_stmt() {
        let p = parse_program("int x = 0; int *p; p = &x; *p = 3;").unwrap();
        assert_eq!(p.items.len(), 4);
        assert!(matches!(
            &p.items[3],
            Item::Stmt(Stmt::Assign { target, .. })
                if matches!(target, Expr::Unary(UnaryOp::Deref, _))
        ));
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
    fn mod_parses_at_mul_precedence() {
        let p = parse_program("int x = 7 % 3 + 1;").unwrap();
        let Item::Stmt(Stmt::VarDecl { init, .. }) = &p.items[0] else {
            panic!("expected var decl");
        };
        assert_eq!(
            init.as_ref(),
            Some(&Expr::Binary(
                BinOp::Add,
                Box::new(Expr::Binary(
                    BinOp::Mod,
                    Box::new(Expr::IntLit(7)),
                    Box::new(Expr::IntLit(3)),
                )),
                Box::new(Expr::IntLit(1)),
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
    fn typedef_scalar_and_use() {
        let p = parse_program(
            "typedef int footype;\n\
             footype x = 7;\n\
             print_int(x);",
        )
        .unwrap();
        assert_eq!(p.items.len(), 2);
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl { ty: Ty::Int, .. }) => {}
            other => panic!("expected int var, got {other:?}"),
        }
    }

    #[test]
    fn typedef_struct_alias() {
        let p = parse_program(
            "struct S { int a; };\n\
             typedef struct S S_alias;\n\
             S_alias s;\n\
             s.a = 3;",
        )
        .unwrap();
        assert!(
            matches!(
                &p.items[1],
                Item::Stmt(Stmt::VarDecl {
                    ty: Ty::Struct(tag),
                    init: None,
                    ..
                }) if tag == "S"
            ),
            "{:?}",
            p.items
        );
    }

    #[test]
    fn typedef_chain() {
        let p = parse_program(
            "typedef int A;\n\
             typedef A B;\n\
             B y = 1;",
        )
        .unwrap();
        match &p.items[0] {
            Item::Stmt(Stmt::VarDecl { ty: Ty::Int, .. }) => {}
            other => panic!("expected int, got {other:?}"),
        }
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
