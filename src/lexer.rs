//! Scan source text into a [`Vec<Token>`] ending with [`Token::Eof`].

use crate::token::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub offset: usize,
    pub message: String,
}

impl LexError {
    fn new(offset: usize, message: impl Into<String>) -> Self {
        Self {
            offset,
            message: message.into(),
        }
    }
}

/// Tokenize `source`. The returned vector always ends with [`Token::Eof`].
pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let bytes = source.as_bytes();
    let mut i = 0usize;
    let mut out = Vec::new();

    while i < bytes.len() {
        let b = bytes[i];

        if b.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        if b == b'/' && bytes.get(i + 1) == Some(&b'/') {
            i += 2;
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        match b {
            b'+' => {
                if bytes.get(i + 1) == Some(&b'+') {
                    out.push(Token::PlusPlus);
                    i += 2;
                } else if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Token::PlusAssign);
                    i += 2;
                } else {
                    out.push(Token::Plus);
                    i += 1;
                }
            }
            b'-' => {
                out.push(Token::Minus);
                i += 1;
            }
            b'*' => {
                out.push(Token::Star);
                i += 1;
            }
            b'/' => {
                out.push(Token::Slash);
                i += 1;
            }
            b'(' => {
                out.push(Token::LParen);
                i += 1;
            }
            b')' => {
                out.push(Token::RParen);
                i += 1;
            }
            b'{' => {
                out.push(Token::LBrace);
                i += 1;
            }
            b'}' => {
                out.push(Token::RBrace);
                i += 1;
            }
            b'[' => {
                out.push(Token::LBracket);
                i += 1;
            }
            b']' => {
                out.push(Token::RBracket);
                i += 1;
            }
            b';' => {
                out.push(Token::Semicolon);
                i += 1;
            }
            b',' => {
                out.push(Token::Comma);
                i += 1;
            }
            b'=' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Token::EqEq);
                    i += 2;
                } else {
                    out.push(Token::Assign);
                    i += 1;
                }
            }
            b'!' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Token::Ne);
                    i += 2;
                } else {
                    out.push(Token::Bang);
                    i += 1;
                }
            }
            b'<' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Token::Le);
                    i += 2;
                } else {
                    out.push(Token::Lt);
                    i += 1;
                }
            }
            b'>' => {
                if bytes.get(i + 1) == Some(&b'=') {
                    out.push(Token::Ge);
                    i += 2;
                } else {
                    out.push(Token::Gt);
                    i += 1;
                }
            }
            b'&' => {
                if bytes.get(i + 1) == Some(&b'&') {
                    out.push(Token::AndAnd);
                    i += 2;
                } else {
                    return Err(LexError::new(i, "expected `&&`"));
                }
            }
            b'|' => {
                if bytes.get(i + 1) == Some(&b'|') {
                    out.push(Token::OrOr);
                    i += 2;
                } else {
                    return Err(LexError::new(i, "expected `||`"));
                }
            }
            b'\'' => {
                i += 1;
                if i >= bytes.len() {
                    return Err(LexError::new(i - 1, "unclosed character literal"));
                }
                let byte = if bytes[i] == b'\\' {
                    i += 1;
                    if i >= bytes.len() {
                        return Err(LexError::new(i - 1, "unclosed character literal after `\\`"));
                    }
                    match bytes[i] {
                        b'n' => b'\n',
                        b't' => b'\t',
                        b'r' => b'\r',
                        b'0' => 0,
                        b'\\' => b'\\',
                        b'\'' => b'\'',
                        c => {
                            return Err(LexError::new(
                                i,
                                format!("unknown escape `\\{}` in character literal", c as char),
                            ));
                        }
                    }
                } else {
                    if bytes[i] == b'\n' {
                        return Err(LexError::new(i, "newline in character literal"));
                    }
                    if bytes[i] == b'\'' {
                        return Err(LexError::new(i, "empty character literal"));
                    }
                    bytes[i]
                };
                i += 1;
                if i >= bytes.len() || bytes[i] != b'\'' {
                    return Err(LexError::new(
                        i.saturating_sub(1),
                        "expected `'` to close character literal",
                    ));
                }
                i += 1;
                out.push(Token::CharLit(byte));
            }
            b'"' => {
                i += 1;
                let mut s = Vec::<u8>::new();
                loop {
                    if i >= bytes.len() {
                        return Err(LexError::new(
                            i.saturating_sub(1),
                            "unclosed string literal",
                        ));
                    }
                    if bytes[i] == b'"' {
                        i += 1;
                        break;
                    }
                    if bytes[i] == b'\\' {
                        i += 1;
                        if i >= bytes.len() {
                            return Err(LexError::new(
                                i.saturating_sub(1),
                                "unclosed string literal after `\\`",
                            ));
                        }
                        let b = match bytes[i] {
                            b'n' => b'\n',
                            b't' => b'\t',
                            b'r' => b'\r',
                            b'0' => 0,
                            b'\\' => b'\\',
                            b'"' => b'"',
                            c => {
                                return Err(LexError::new(
                                    i,
                                    format!("unknown escape `\\{}` in string literal", c as char),
                                ));
                            }
                        };
                        s.push(b);
                        i += 1;
                        continue;
                    }
                    if bytes[i] == b'\n' {
                        return Err(LexError::new(i, "newline in string literal"));
                    }
                    s.push(bytes[i]);
                    i += 1;
                }
                out.push(Token::StringLit(s));
            }
            b'0'..=b'9' => {
                let start = i;
                let mut value: i64 = 0;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    let d = (bytes[i] - b'0') as i64;
                    value = value
                        .checked_mul(10)
                        .and_then(|v| v.checked_add(d))
                        .ok_or_else(|| LexError::new(start, "integer literal overflow"))?;
                    i += 1;
                }
                out.push(Token::IntLit(value));
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    let c = bytes[i];
                    if c.is_ascii_alphanumeric() || c == b'_' {
                        i += 1;
                    } else {
                        break;
                    }
                }
                let text = source.get(start..i).ok_or_else(|| LexError::new(start, "bad slice"))?;
                let tok = Token::keyword(text).unwrap_or_else(|| Token::Ident(text.to_string()));
                out.push(tok);
            }
            _ => {
                let ch = b as char;
                return Err(LexError::new(
                    i,
                    format!("unexpected character `{ch}` (byte {b})"),
                ));
            }
        }
    }

    out.push(Token::Eof);
    Ok(out)
}
