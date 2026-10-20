//! Tokens: a clause's normalized text as words, numbers, mana symbols and punctuation, and the
//! reader that walks them together with what the clause has asked for so far.

use crate::refs::{Ref, TargetSpec};

/// One token of a clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A word in lowercase; apostrophes and hyphens inside it stay (`can't`).
    Word(String),
    /// A run of digits.
    Number(u32),
    /// A symbol in braces, without the braces (`T`, `G`, `2`).
    Symbol(String),
    /// The card itself: `~`, or "this creature" and the like.
    Source,
    /// Any other character.
    Punct(char),
}

/// The nouns a card calls itself by after "this".
const SOURCE_NOUNS: [&str; 12] = [
    "creature",
    "spell",
    "artifact",
    "enchantment",
    "land",
    "permanent",
    "planeswalker",
    "vehicle",
    "battle",
    "equipment",
    "aura",
    "card",
];

/// The text with its whitespace collapsed and the card's names ("this creature") written as `~`.
pub fn canonical(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(words.len());
    let mut at = 0;
    while let Some(&word) = words.get(at) {
        let noun = words
            .get(at + 1)
            .map(|next| next.trim_end_matches(|c: char| !c.is_alphanumeric()));
        if word.eq_ignore_ascii_case("this")
            && let Some(noun) = noun
            && let Some(next) = words.get(at + 1)
            && SOURCE_NOUNS.contains(&noun.to_ascii_lowercase().as_str())
        {
            out.push(format!("~{}", next.get(noun.len()..).unwrap_or_default()));
            at += 2;
        } else {
            out.push(word.to_owned());
            at += 1;
        }
    }
    then_as_sentence(&out.join(" "))
}

/// ", then" between steps is the same sequence as a new sentence, so it is written as one.
fn then_as_sentence(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some((before, after)) = rest.split_once(", then ") {
        out.push_str(before);
        out.push_str(". ");
        let mut chars = after.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
        }
        rest = chars.as_str();
    }
    out.push_str(rest);
    out
}

/// Whether two wordings are the same text once whitespace, a closing period, the card's own name
/// ("this creature" for `~`) and ", then" for a new sentence are normalized.
pub fn same_text(left: &str, right: &str) -> bool {
    without_period(&canonical(left)) == without_period(&canonical(right))
}

/// The text without its closing period.
pub fn without_period(text: &str) -> &str {
    let text = text.trim_end();
    text.strip_suffix('.').unwrap_or(text)
}

fn word_or_number(run: &str) -> Token {
    if run.chars().all(|c| c.is_ascii_digit())
        && let Ok(number) = run.parse()
    {
        return Token::Number(number);
    }
    Token::Word(run.replace('\u{2019}', "'").to_lowercase())
}

/// The tokens of a clause's normalized text.
pub fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut at = 0;
    while let Some(&c) = chars.get(at) {
        if c.is_whitespace() {
            at += 1;
        } else if c == '{'
            && let Some(len) = chars.iter().skip(at).position(|&x| x == '}')
        {
            tokens.push(Token::Symbol(
                chars.iter().skip(at + 1).take(len - 1).collect(),
            ));
            at += len + 1;
        } else if c == '~' {
            tokens.push(Token::Source);
            at += 1;
        } else if c.is_alphanumeric() {
            let start = at;
            while let Some(&x) = chars.get(at) {
                let inner = at > start
                    && matches!(x, '\'' | '\u{2019}' | '-')
                    && chars.get(at + 1).is_some_and(|next| next.is_alphanumeric());
                if x.is_alphanumeric() || inner {
                    at += 1;
                } else {
                    break;
                }
            }
            let run: String = chars.iter().skip(start).take(at - start).collect();
            tokens.push(word_or_number(&run));
        } else {
            tokens.push(Token::Punct(c));
            at += 1;
        }
    }
    merge_source(tokens)
}

/// "this" and a noun for the card become the source token.
fn merge_source(tokens: Vec<Token>) -> Vec<Token> {
    let mut merged = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        let this = matches!(&token, Token::Word(word) if word == "this");
        let noun =
            matches!(iter.peek(), Some(Token::Word(word)) if SOURCE_NOUNS.contains(&word.as_str()));
        if this && noun {
            iter.next();
            merged.push(Token::Source);
        } else {
            merged.push(token);
        }
    }
    merged
}

/// A reader over a clause's tokens, with the targets the clause has asked for so far.
pub struct Reader<'a> {
    tokens: &'a [Token],
    at: usize,
    /// The targets the clause asks for; `Ref::Target(i)` is the i-th.
    pub targets: Vec<TargetSpec>,
    /// Whether the card's mana cost has an `{X}`, so `X` can stand for an amount.
    pub x_free: bool,
    /// What "it" and "its" mean: the object the last sentence acted on.
    pub it: Option<Ref>,
}

impl<'a> Reader<'a> {
    /// A reader at the start of the tokens.
    pub fn new(tokens: &'a [Token], x_free: bool) -> Self {
        Reader {
            tokens,
            at: 0,
            targets: Vec::new(),
            x_free,
            it: None,
        }
    }

    /// Whether every token has been read.
    pub fn end(&self) -> bool {
        self.at >= self.tokens.len()
    }

    /// The word `ahead` tokens on, when it is one.
    pub fn word_at(&self, ahead: usize) -> Option<&'a str> {
        match self.tokens.get(self.at + ahead)? {
            Token::Word(word) => Some(word.as_str()),
            _ => None,
        }
    }

    /// The next word, when the next token is one.
    pub fn peek_word(&self) -> Option<&'a str> {
        self.word_at(0)
    }

    /// Moves past one token.
    pub fn skip(&mut self) {
        self.at += 1;
    }

    /// Reads any word.
    pub fn take_word(&mut self) -> Option<&'a str> {
        let word = self.peek_word()?;
        self.at += 1;
        Some(word)
    }

    /// Reads this word if it is next.
    pub fn word(&mut self, word: &str) -> bool {
        if self.peek_word() == Some(word) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Reads these words (separated by spaces), all or none.
    pub fn words(&mut self, phrase: &str) -> bool {
        self.attempt(|r| phrase.split(' ').all(|word| r.word(word)).then_some(()))
            .is_some()
    }

    /// Reads these words or fails.
    pub fn need(&mut self, phrase: &str) -> Option<()> {
        self.words(phrase).then_some(())
    }

    /// Reads this punctuation mark if it is next.
    pub fn punct(&mut self, mark: char) -> bool {
        if self.tokens.get(self.at) == Some(&Token::Punct(mark)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Reads a number.
    pub fn number(&mut self) -> Option<u32> {
        let Some(&Token::Number(number)) = self.tokens.get(self.at) else {
            return None;
        };
        self.at += 1;
        Some(number)
    }

    /// Reads a symbol in braces.
    pub fn symbol(&mut self) -> Option<&'a str> {
        let Some(Token::Symbol(symbol)) = self.tokens.get(self.at) else {
            return None;
        };
        self.at += 1;
        Some(symbol.as_str())
    }

    /// Reads the card's own name.
    pub fn source(&mut self) -> bool {
        if self.tokens.get(self.at) == Some(&Token::Source) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// Reads the end of a sentence: its period, or the end of the text.
    pub fn sentence_end(&mut self) -> bool {
        self.punct('.') || self.end()
    }

    /// Reads with `read`; when it fails the reader is where it was, with the same targets.
    pub fn attempt<T>(&mut self, read: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let (at, targets, it) = (self.at, self.targets.len(), self.it.clone());
        let found = read(self);
        if found.is_none() {
            self.at = at;
            self.targets.truncate(targets);
            self.it = it;
        }
        found
    }
}
