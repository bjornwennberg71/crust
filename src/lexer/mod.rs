use crate::ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Literals
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    KwTrue,
    KwFalse,
    Ident(String),

    // Keywords — declarations
    KwLambda,
    KwLet,
    KwReturn,
    KwConst,
    KwStatic,
    KwType,
    KwUse,
    KwImpl,
    KwPub,
    KwMut,
    KwSelf,
    KwUnsafe,
    KwAs,
    KwExtern,
    KwString,
    KwTrait,
    KwAsync,
    KwAwait,

    // Keywords — control flow
    KwIf,
    KwElse,
    KwWhile,
    KwFor,
    KwIn,
    KwBreak,
    KwContinue,
    KwSwitch,
    KwCase,
    KwDefault,
    KwMatch,

    // Keywords — types
    KwInt,
    KwFloat,
    KwBool,
    KwVoid,
    KwChar,
    KwStruct,
    KwEnum,

    // Punctuation
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,   // [
    RBracket,   // ]
    Semicolon,
    Colon,
    ColonColon, // ::
    Comma,
    Arrow,      // ->
    FatArrow,   // =>
    Eq,         // =
    PlusEq,     // +=
    MinusEq,    // -=
    StarEq,     // *=
    SlashEq,    // /=
    PipeEq,     // |=
    AmpEq,      // &=
    CaretEq,    // ^=
    LtLtEq,     // <<=
    GtGtEq,     // >>=
    EqEq,       // ==
    Bang,       // !
    BangEq,     // !=
    Lt,         // <
    LtEq,       // <=
    LtLt,       // <<
    Gt,         // >
    GtEq,       // >=
    GtGt,       // >>
    Plus,
    PlusPlus,   // ++
    Minus,
    MinusMinus, // --
    Star,
    Slash,
    Percent,    // %
    Tilde,      // ~
    Caret,      // ^
    Ampersand,
    AmpAmp,     // &&
    Pipe,       // |
    PipePipe,   // ||
    Dot,
    DotDot,     // ..
    DotDotEq,   // ..=
    Question,   // ?
    At,         // @
    Hash,       // #

    Eof,
}

impl TokenKind {
    /// Human-readable name for diagnostics: `';'`, `keyword 'function'`,
    /// `identifier 'foo'` — never the Debug variant name.
    pub fn describe(&self) -> String {
        use TokenKind::*;
        let fixed = match self {
            IntLit(n)    => return format!("integer literal '{n}'"),
            FloatLit(f)  => return format!("float literal '{f}'"),
            StringLit(_) => return "string literal".to_string(),
            Ident(s)     => return format!("identifier '{s}'"),
            Eof          => return "end of file".to_string(),

            KwTrue => "true", KwFalse => "false",
            KwLambda => "lambda",
            KwLet => "let", KwReturn => "return",
            KwConst => "const", KwStatic => "static", KwType => "type",
            KwUse => "use", KwImpl => "impl", KwPub => "public", KwMut => "mut",
            KwSelf => "this", KwUnsafe => "unsafe", KwAs => "as",
            KwExtern => "extern", KwString => "string", KwTrait => "trait",
            KwAsync => "async", KwAwait => "await",
            KwIf => "if", KwElse => "else", KwWhile => "while", KwFor => "for",
            KwIn => "in", KwBreak => "break", KwContinue => "continue",
            KwSwitch => "switch", KwCase => "case", KwDefault => "default",
            KwMatch => "match",
            KwInt => "int", KwFloat => "float", KwBool => "bool",
            KwVoid => "void", KwChar => "char",
            KwStruct => "struct", KwEnum => "enum",

            LParen => "(", RParen => ")", LBrace => "{", RBrace => "}",
            LBracket => "[", RBracket => "]",
            Semicolon => ";", Colon => ":", ColonColon => "::", Comma => ",",
            Arrow => "->", FatArrow => "=>", Eq => "=",
            PlusEq => "+=", MinusEq => "-=", StarEq => "*=", SlashEq => "/=",
            PipeEq => "|=", AmpEq => "&=", CaretEq => "^=",
            LtLtEq => "<<=", GtGtEq => ">>=",
            EqEq => "==", Bang => "!", BangEq => "!=",
            Lt => "<", LtEq => "<=", LtLt => "<<",
            Gt => ">", GtEq => ">=", GtGt => ">>",
            Plus => "+", PlusPlus => "++", Minus => "-", MinusMinus => "--",
            Star => "*", Slash => "/", Percent => "%",
            Tilde => "~", Caret => "^",
            Ampersand => "&", AmpAmp => "&&", Pipe => "|", PipePipe => "||",
            Dot => ".", DotDot => "..", DotDotEq => "..=",
            Question => "?", At => "@", Hash => "#",
        };
        let is_keyword = fixed.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
        if is_keyword {
            format!("keyword '{fixed}'")
        } else {
            format!("'{fixed}'")
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = source.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {}

            '/' if matches!(chars.peek(), Some((_, '/'))) => {
                for (_, c) in chars.by_ref() { if c == '\n' { break; } }
            }

            '(' => tokens.push(tok(TokenKind::LParen,    start, start + 1)),
            ')' => tokens.push(tok(TokenKind::RParen,    start, start + 1)),
            '{' => tokens.push(tok(TokenKind::LBrace,    start, start + 1)),
            '}' => tokens.push(tok(TokenKind::RBrace,    start, start + 1)),
            '[' => tokens.push(tok(TokenKind::LBracket,  start, start + 1)),
            ']' => tokens.push(tok(TokenKind::RBracket,  start, start + 1)),
            ';' => tokens.push(tok(TokenKind::Semicolon, start, start + 1)),
            ',' => tokens.push(tok(TokenKind::Comma,     start, start + 1)),
            '%' => tokens.push(tok(TokenKind::Percent,   start, start + 1)),
            '?' => tokens.push(tok(TokenKind::Question,  start, start + 1)),
            '@' => tokens.push(tok(TokenKind::At,        start, start + 1)),
            '#' => tokens.push(tok(TokenKind::Hash,      start, start + 1)),
            '~' => tokens.push(tok(TokenKind::Tilde,     start, start + 1)),
            '^' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::CaretEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Caret, start, start + 1));
                }
            }
            '.' => {
                if matches!(chars.peek(), Some((_, '.'))) {
                    chars.next();
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(tok(TokenKind::DotDotEq, start, start + 3));
                    } else {
                        tokens.push(tok(TokenKind::DotDot, start, start + 2));
                    }
                } else {
                    tokens.push(tok(TokenKind::Dot, start, start + 1));
                }
            }

            ':' => {
                if matches!(chars.peek(), Some((_, ':'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::ColonColon, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Colon, start, start + 1));
                }
            }

            '+' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::PlusEq, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '+'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::PlusPlus, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Plus, start, start + 1));
                }
            }

            '-' => {
                if matches!(chars.peek(), Some((_, '>'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::Arrow, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::MinusEq, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '-'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::MinusMinus, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Minus, start, start + 1));
                }
            }

            '*' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::StarEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Star, start, start + 1));
                }
            }

            '/' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::SlashEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Slash, start, start + 1));
                }
            }

            '&' => {
                if matches!(chars.peek(), Some((_, '&'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::AmpAmp, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::AmpEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Ampersand, start, start + 1));
                }
            }

            '|' => {
                if matches!(chars.peek(), Some((_, '|'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::PipePipe, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::PipeEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Pipe, start, start + 1));
                }
            }

            '=' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::EqEq, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '>'))) {
                    chars.next();
                    tokens.push(tok(TokenKind::FatArrow, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Eq, start, start + 1));
                }
            }

            '!' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::BangEq, start, start + 2));
                } else {
                    tokens.push(tok(TokenKind::Bang, start, start + 1));
                }
            }

            '<' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::LtEq, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '<'))) {
                    chars.next();
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(tok(TokenKind::LtLtEq, start, start + 3));
                    } else {
                        tokens.push(tok(TokenKind::LtLt, start, start + 2));
                    }
                } else {
                    tokens.push(tok(TokenKind::Lt, start, start + 1));
                }
            }

            '>' => {
                if matches!(chars.peek(), Some((_, '='))) {
                    chars.next();
                    tokens.push(tok(TokenKind::GtEq, start, start + 2));
                } else if matches!(chars.peek(), Some((_, '>'))) {
                    chars.next();
                    if matches!(chars.peek(), Some((_, '='))) {
                        chars.next();
                        tokens.push(tok(TokenKind::GtGtEq, start, start + 3));
                    } else {
                        tokens.push(tok(TokenKind::GtGt, start, start + 2));
                    }
                } else {
                    tokens.push(tok(TokenKind::Gt, start, start + 1));
                }
            }

            '"' => {
                let mut s = String::new();
                let mut end = start + 1;
                while let Some((i, c)) = chars.next() {
                    end = i + 1;
                    if c == '"' { break; }
                    if c == '\\' {
                        if let Some((_, esc)) = chars.next() {
                            end += 1;
                            match esc {
                                'n'  => s.push('\n'),
                                't'  => s.push('\t'),
                                'r'  => s.push('\r'),
                                '0'  => s.push('\0'),
                                '"'  => s.push('"'),
                                '\\' => s.push('\\'),
                                'x'  => {
                                    // \xNN — read two hex digits
                                    let h1 = chars.next().map(|(i, c)| { end = i + 1; c }).unwrap_or('0');
                                    let h2 = chars.next().map(|(i, c)| { end = i + 1; c }).unwrap_or('0');
                                    let val = u8::from_str_radix(&format!("{h1}{h2}"), 16).unwrap_or(0);
                                    s.push(val as char);
                                }
                                other => { s.push('\\'); s.push(other); }
                            }
                        }
                    } else {
                        s.push(c);
                    }
                }
                tokens.push(tok(TokenKind::StringLit(s), start, end));
            }

            '0'..='9' => {
                let mut end = start + 1;
                // hex literal: 0x...
                if ch == '0' && matches!(chars.peek(), Some((_, 'x')) | Some((_, 'X'))) {
                    chars.next(); // consume 'x'
                    end = start + 2;
                    while matches!(chars.peek(), Some((_, '0'..='9')) | Some((_, 'a'..='f')) | Some((_, 'A'..='F')) | Some((_, '_'))) {
                        let (i, c) = chars.next().unwrap();
                        if c != '_' { end = i + 1; }
                    }
                    let hex = source[start+2..end].replace('_', "");
                    let n = i64::from_str_radix(&hex, 16).unwrap_or(0);
                    tokens.push(tok(TokenKind::IntLit(n), start, end));
                } else {
                    while matches!(chars.peek(), Some((_, '0'..='9'))) {
                        end = chars.next().unwrap().0 + 1;
                    }
                    // Only treat as float if next char is '.' AND char after '.' is a digit
                    // (prevents consuming '.' from '..' range syntax like 0..5)
                    let is_float = matches!(chars.peek(), Some((_, '.')))
                        && matches!(source.as_bytes().get(end + 1), Some(b'0'..=b'9'));
                    if is_float {
                        end = chars.next().unwrap().0 + 1;
                        while matches!(chars.peek(), Some((_, '0'..='9'))) {
                            end = chars.next().unwrap().0 + 1;
                        }
                        let f: f64 = source[start..end].parse().unwrap();
                        tokens.push(tok(TokenKind::FloatLit(f), start, end));
                    } else {
                        let n: i64 = source[start..end].parse().unwrap();
                        tokens.push(tok(TokenKind::IntLit(n), start, end));
                    }
                }
            }

            'a'..='z' | 'A'..='Z' | '_' => {
                let mut end = start + 1;
                while matches!(chars.peek(), Some((_, c)) if c.is_alphanumeric() || *c == '_') {
                    end = chars.next().unwrap().0 + 1;
                }
                let word = &source[start..end];
                let kind = match word {
                    "lambda"   => TokenKind::KwLambda,
                    "let"      => TokenKind::KwLet,
                    "auto"     => TokenKind::KwLet,   // C++ spelling, same meaning
                    "return"   => TokenKind::KwReturn,
                    "const"    => TokenKind::KwConst,
                    "static"   => TokenKind::KwStatic,
                    "type"     => TokenKind::KwType,
                    "use"      => TokenKind::KwUse,
                    "impl"     => TokenKind::KwImpl,
                    "pub"      => TokenKind::KwPub,
                    "public"   => TokenKind::KwPub,
                    "mut"      => TokenKind::KwMut,
                    "mutable"  => TokenKind::KwMut,
                    "self"     => TokenKind::KwSelf,
                    "this"     => TokenKind::KwSelf,
                    "unsafe"   => TokenKind::KwUnsafe,
                    "as"       => TokenKind::KwAs,
                    "extern"   => TokenKind::KwExtern,
                    "char"     => TokenKind::KwChar,
                    "string"   => TokenKind::KwString,
                    "trait"    => TokenKind::KwTrait,
                    "async"    => TokenKind::KwAsync,
                    "await"    => TokenKind::KwAwait,
                    "if"       => TokenKind::KwIf,
                    "else"     => TokenKind::KwElse,
                    "while"    => TokenKind::KwWhile,
                    "for"      => TokenKind::KwFor,
                    "in"       => TokenKind::KwIn,
                    "break"    => TokenKind::KwBreak,
                    "continue" => TokenKind::KwContinue,
                    "switch"   => TokenKind::KwSwitch,
                    "case"     => TokenKind::KwCase,
                    "default"  => TokenKind::KwDefault,
                    "match"    => TokenKind::KwMatch,
                    "int"      => TokenKind::KwInt,
                    "float"    => TokenKind::KwFloat,
                    "bool"     => TokenKind::KwBool,
                    "void"     => TokenKind::KwVoid,
                    "struct"   => TokenKind::KwStruct,
                    "enum"     => TokenKind::KwEnum,
                    "true"     => TokenKind::KwTrue,
                    "false"    => TokenKind::KwFalse,
                    _          => TokenKind::Ident(word.to_string()),
                };
                tokens.push(tok(kind, start, end));
            }

            other => crate::error::raise(
                Span { start, end: start + other.len_utf8() },
                format!("unexpected character {:?}", other),
            ),
        }
    }

    let end = source.len();
    tokens.push(tok(TokenKind::Eof, end, end));
    tokens
}

fn tok(kind: TokenKind, start: usize, end: usize) -> Token {
    Token { kind, span: Span { start, end } }
}
