//! A small Rust lexer: identifiers, punctuation, strings and the `///` doc comments of a source file; everything else is skipped.

/// One token of a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Tok {
    /// An identifier, keyword or number.
    Ident(String),
    /// A lifetime such as `'a`.
    Lifetime,
    /// One punctuation character.
    Punct(char),
    /// A string literal's content.
    Str(String),
    /// The text of one `///` line.
    Doc(String),
    /// A character or numeric-like literal that carries nothing the scanner reads.
    Other,
}

/// The tokens of `source`, comments dropped and `///` lines kept as `Doc`.
pub(super) fn lex(source: &str) -> Vec<Tok> {
    let chars: Vec<char> = source.chars().collect();
    let mut lexer = Lexer {
        chars: &chars,
        at: 0,
        out: Vec::new(),
    };
    while lexer.at < chars.len() {
        lexer.step();
    }
    lexer.out
}

struct Lexer<'a> {
    chars: &'a [char],
    at: usize,
    out: Vec<Tok>,
}

impl Lexer<'_> {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn step(&mut self) {
        let Some(c) = self.peek(0) else { return };
        match c {
            c if c.is_whitespace() => self.at += 1,
            '/' if self.peek(1) == Some('/') => self.line_comment(),
            '/' if self.peek(1) == Some('*') => self.block_comment(),
            '"' => self.string(),
            'r' if self.raw_string_ahead() => self.raw_string(),
            'b' if self.peek(1) == Some('"') => {
                self.at += 1;
                self.string();
            }
            '\'' => self.quote(),
            c if c.is_alphanumeric() || c == '_' => self.ident(),
            c => {
                self.out.push(Tok::Punct(c));
                self.at += 1;
            }
        }
    }

    fn line_comment(&mut self) {
        let start = self.at;
        while self.peek(0).is_some_and(|c| c != '\n') {
            self.at += 1;
        }
        let line: String = self.chars[start..self.at].iter().collect();
        // `///` is a doc comment, `////` and `//!` are not.
        if let Some(doc) = line.strip_prefix("///")
            && !doc.starts_with('/')
        {
            self.out.push(Tok::Doc(doc.trim().to_owned()));
        }
    }

    fn block_comment(&mut self) {
        let mut depth = 0usize;
        while self.at < self.chars.len() {
            match (self.peek(0), self.peek(1)) {
                (Some('/'), Some('*')) => {
                    depth += 1;
                    self.at += 2;
                }
                (Some('*'), Some('/')) => {
                    depth = depth.saturating_sub(1);
                    self.at += 2;
                    if depth == 0 {
                        return;
                    }
                }
                _ => self.at += 1,
            }
        }
    }

    fn string(&mut self) {
        self.at += 1;
        let mut text = String::new();
        while let Some(c) = self.peek(0) {
            self.at += 1;
            match c {
                '"' => break,
                '\\' => {
                    if let Some(next) = self.peek(0) {
                        text.push(next);
                        self.at += 1;
                    }
                }
                c => text.push(c),
            }
        }
        self.out.push(Tok::Str(text));
    }

    /// Whether `r"` or `r#"` (any number of hashes) starts here.
    fn raw_string_ahead(&self) -> bool {
        let mut ahead = 1;
        while self.peek(ahead) == Some('#') {
            ahead += 1;
        }
        self.peek(ahead) == Some('"')
    }

    fn raw_string(&mut self) {
        self.at += 1;
        let mut hashes = 0;
        while self.peek(0) == Some('#') {
            hashes += 1;
            self.at += 1;
        }
        self.at += 1;
        let start = self.at;
        while self.at < self.chars.len() {
            if self.peek(0) == Some('"') && (1..=hashes).all(|n| self.peek(n) == Some('#')) {
                let text: String = self.chars[start..self.at].iter().collect();
                self.at += 1 + hashes;
                self.out.push(Tok::Str(text));
                return;
            }
            self.at += 1;
        }
    }

    /// A character literal (`'x'`, `'\n'`) or a lifetime (`'a`).
    fn quote(&mut self) {
        let is_char = matches!(
            (self.peek(1), self.peek(2)),
            (Some('\\'), _) | (Some(_), Some('\''))
        );
        self.at += 1;
        if is_char {
            while let Some(c) = self.peek(0) {
                self.at += 1;
                if c == '\\' {
                    self.at += 1;
                } else if c == '\'' {
                    break;
                }
            }
            self.out.push(Tok::Other);
        } else {
            while self
                .peek(0)
                .is_some_and(|c| c.is_alphanumeric() || c == '_')
            {
                self.at += 1;
            }
            self.out.push(Tok::Lifetime);
        }
    }

    fn ident(&mut self) {
        let start = self.at;
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        self.out.push(Tok::Ident(text));
    }
}
