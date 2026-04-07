//! Lexical tokens for the C-shaped surface syntax (`int` / `bool`, statements, expressions).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    // --- keywords ---
    KwInt,
    KwBool,
    KwVoid,
    KwIf,
    KwWhile,
    KwFor,
    KwElse,
    KwReturn,
    KwTrue,
    KwFalse,
    KwChar,
    KwStruct,
    KwTypedef,

    // --- identifiers & literals ---
    Ident(String),
    IntLit(i64),
    /// One ASCII / byte (`'x'`, `'\n'`, …).
    CharLit(u8),
    /// UTF-8 / bytes inside `"..."` (no implicit NUL; lowering adds `\0` for C strings).
    StringLit(Vec<u8>),

    // --- operators ---
    /// `+`
    Plus,
    /// `++`
    PlusPlus,
    /// `+=`
    PlusAssign,
    /// `-` (subtraction or unary negation; arity decided in the parser)
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
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
    /// `.` (struct member)
    Dot,
    /// `&` (address-of); `&&` stays [`Token::AndAnd`].
    Ampersand,
    /// `->` (member through pointer)
    Arrow,

    // --- punctuation ---
    Semicolon,
    Comma,
    LParen,
    RParen,
    LBrace,
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,

    /// End of input (convenient for recursive-descent parsers).
    Eof,
}

impl Token {
    /// Maps a reserved spelling to a keyword token. Ordinary identifiers are not handled here.
    pub fn keyword(ident: &str) -> Option<Token> {
        let tok = match ident {
            "int" => Token::KwInt,
            "bool" => Token::KwBool,
            "void" => Token::KwVoid,
            "if" => Token::KwIf,
            "while" => Token::KwWhile,
            "for" => Token::KwFor,
            "else" => Token::KwElse,
            "return" => Token::KwReturn,
            "true" => Token::KwTrue,
            "false" => Token::KwFalse,
            "char" => Token::KwChar,
            "struct" => Token::KwStruct,
            "typedef" => Token::KwTypedef,
            _ => return None,
        };
        Some(tok)
    }
}
