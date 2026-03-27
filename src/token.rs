//! Lexical tokens for the C-shaped surface syntax (`int` / `bool`, statements, expressions).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // --- keywords ---
    KwInt,
    KwBool,
    KwIf,
    KwElse,
    KwTrue,
    KwFalse,
    KwPrintInt,
    KwPrintBool,
    KwChar,
    KwPrintChar,

    // --- identifiers & literals ---
    Ident(String),
    IntLit(i64),
    /// One ASCII / byte (`'x'`, `'\n'`, …).
    CharLit(u8),

    // --- operators ---
    /// `+`
    Plus,
    /// `-` (subtraction or unary negation; arity decided in the parser)
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `==`
    EqEq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `&&`
    AndAnd,
    /// `||`
    OrOr,
    /// `!`
    Bang,
    /// `=`
    Assign,

    // --- punctuation ---
    Semicolon,
    LParen,
    RParen,
    LBrace,
    RBrace,

    /// End of input (convenient for recursive-descent parsers).
    Eof,
}

impl Token {
    /// Maps a reserved spelling to a keyword token. Ordinary identifiers are not handled here.
    pub fn keyword(ident: &str) -> Option<Token> {
        let tok = match ident {
            "int" => Token::KwInt,
            "bool" => Token::KwBool,
            "if" => Token::KwIf,
            "else" => Token::KwElse,
            "true" => Token::KwTrue,
            "false" => Token::KwFalse,
            "print_int" => Token::KwPrintInt,
            "print_bool" => Token::KwPrintBool,
            "char" => Token::KwChar,
            "print_char" => Token::KwPrintChar,
            _ => return None,
        };
        Some(tok)
    }
}
