//! Tokens: a clause's normalized text as words, numbers, mana symbols and punctuation, and the
//! reader that walks them together with what the clause has asked for so far.

use super::subtypes::is_subtype;
use crate::mana::ManaCostX;
use crate::refs::{Ref, TargetSpec};

/// One token of a clause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token {
    /// A word in lowercase; apostrophes and hyphens inside it stay (`can't`).
    Word(String),
    /// A capitalized word inside a sentence, as written: a subtype ("Soldier") or a name.
    Name(String),
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
const SOURCE_NOUNS: [&str; 13] = [
    "creature",
    "spell",
    "artifact",
    "enchantment",
    "land",
    "permanent",
    "planeswalker",
    "vehicle",
    "spacecraft",
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
        let investigate = word.trim_end_matches(|c: char| !c.is_alphanumeric());
        if word.eq_ignore_ascii_case("this")
            && let Some(noun) = noun
            && let Some(next) = words.get(at + 1)
            && let stem = noun.strip_suffix("'s").unwrap_or(noun)
            && SOURCE_NOUNS.contains(&stem.to_ascii_lowercase().as_str())
        {
            out.push(format!("~{}", next.get(stem.len()..).unwrap_or_default()));
            at += 2;
        } else if word == "enters"
            && words.get(at + 1) == Some(&"the")
            && let Some(field) = words.get(at + 2)
            && field.trim_end_matches(|c: char| !c.is_alphanumeric()) == "battlefield"
        {
            // The older wording of "enters"; what follows (a comma, a period) stays.
            let tail = field.trim_start_matches(|c: char| c.is_alphanumeric());
            out.push(format!("enters{tail}"));
            at += 3;
        } else if word == "of"
            && words.get(at + 1) == Some(&"their")
            && let Some(choice) = words.get(at + 2)
            && choice.trim_end_matches(|c: char| !c.is_alphanumeric()) == "choice"
        {
            // The player who sacrifices or discards chooses anyway; what follows the words stays.
            let tail = choice.trim_start_matches(|c: char| c.is_alphanumeric());
            if let Some(last) = out.last_mut() {
                last.push_str(tail);
            }
            at += 3;
        } else if matches!(word, "first" | "second")
            && words.get(at + 1) == Some(&"main")
            && let Some(phase) = words.get(at + 2)
            && phase.trim_end_matches(|c: char| !c.is_alphanumeric()) == "phase"
        {
            // The newer wording of the main phases.
            let older = if word == "first" {
                "precombat"
            } else {
                "postcombat"
            };
            out.push(format!("{older} main {phase}"));
            at += 3;
        } else if matches!(word, "that" | "those")
            && let Some(card) = words.get(at + 1)
            && let Some(tail) = card
                .strip_prefix(if word == "that" { "card" } else { "cards" })
                .filter(|tail| !tail.starts_with(|c: char| c.is_alphanumeric()))
            && out.last().is_some_and(|last| {
                last.eq_ignore_ascii_case("put") || last.eq_ignore_ascii_case("reveal")
            })
        {
            // "put that card into your hand" says what "put it into your hand" says.
            out.push(format!(
                "{}{tail}",
                if word == "that" { "it" } else { "them" }
            ));
            at += 2;
        } else if let Some((text, used)) = pronoun(&words, at, &out) {
            out.push(text);
            at += used;
        } else if investigate.eq_ignore_ascii_case("investigate")
            || investigate.eq_ignore_ascii_case("investigates")
        {
            // The keyword action is the same as its reminder text.
            let create = match (investigate.starts_with('I'), investigate.ends_with('s')) {
                (true, false) => "Create",
                (false, false) => "create",
                (true, true) => "Creates",
                (false, true) => "creates",
            };
            let tail = word.get(investigate.len()..).unwrap_or_default();
            out.push(format!("{create} a Clue token{tail}"));
            at += 1;
        } else {
            out.push(in_cost_order(word));
            at += 1;
        }
    }
    then_as_sentence(&out.join(" "))
}

/// A word that begins with mana symbols, with those symbols in the order a mana cost is written: a card may print
/// "{2}{G}{U}" where the cost is kept as "{2}{U}{G}". Any other word stays as it is.
fn in_cost_order(word: &str) -> String {
    let mut run = 0;
    while word[run..].starts_with('{') {
        match word[run..].find('}') {
            Some(close) => run += close + 1,
            None => break,
        }
    }
    let (symbols, tail) = word.split_at(run);
    match symbols.parse::<ManaCostX>() {
        Ok(cost) if run > 0 => format!("{cost}{tail}"),
        _ => word.to_owned(),
    }
}

/// A pronoun for the object a verb or a copy is about, and how many words it takes: "a token that's a copy of that
/// creature" says what "a copy of it" says, and "sacrifice it" says what "sacrifice ~" says (the readers accept "it"
/// for the card only where the ability spoke of it).
fn pronoun(words: &[&str], at: usize, out: &[String]) -> Option<(String, usize)> {
    let word = *words.get(at)?;
    let bare = |text: &str| {
        text.trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_owned()
    };
    let tail = |text: &str| text.get(bare(text).len()..).unwrap_or_default().to_owned();
    let last = out.last()?;
    if word == "that"
        && let Some(noun) = words.get(at + 1)
        && matches!(bare(noun).as_str(), "creature" | "permanent")
        && last == "of"
        && out
            .get(out.len().checked_sub(2)?)
            .is_some_and(|copy| matches!(copy.as_str(), "copy" | "copies"))
    {
        return Some((format!("it{}", tail(noun)), 2));
    }
    (bare(word) == "it"
        && last.eq_ignore_ascii_case("sacrifice")
        && matches!(tail(word).as_str(), "" | "." | "," | ".\""))
    .then(|| (format!("~{}", tail(word)), 1))
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
/// ("this creature" for `~`), ", then" for a new sentence and the order of the symbols in a run of mana symbols
/// (`{G}{W}` and `{W}{G}` are one cost) are normalized.
pub fn same_text(left: &str, right: &str) -> bool {
    let sorted = |text: &str| symbols_sorted(without_period(&canonical(text)));
    sorted(left) == sorted(right)
}

/// The text with the symbols of each run of adjacent mana symbols in one fixed order: a cost has no order, and the
/// renderer writes the one the data keeps.
pub fn symbols_sorted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run: Vec<&str> = Vec::new();
    let mut rest = text;
    loop {
        let symbol = rest
            .strip_prefix('{')
            .and_then(|tail| tail.find('}'))
            .and_then(|end| rest.split_at_checked(end + 2));
        if let Some((symbol, after)) = symbol {
            run.push(symbol);
            rest = after;
            continue;
        }
        run.sort_unstable();
        run.drain(..).for_each(|symbol| out.push_str(symbol));
        let mut chars = rest.chars();
        let Some(next) = chars.next() else { break };
        out.push(next);
        rest = chars.as_str();
    }
    out
}

/// The text without its closing period.
pub fn without_period(text: &str) -> &str {
    let text = text.trim_end();
    text.strip_suffix('.').unwrap_or(text)
}

/// A run of letters or digits as a token; `opens` says it begins a sentence, where a capital is
/// grammar, and a single capital ("X") is never a name.
fn word_or_number(run: &str, opens: bool) -> Token {
    if run.chars().all(|c| c.is_ascii_digit())
        && let Ok(number) = run.parse()
    {
        return Token::Number(number);
    }
    let capital = run.chars().next().is_some_and(char::is_uppercase);
    // The verbs of a cost follow a comma: "{T}, Pay 1 life, Sacrifice ~".
    let cost_verb = matches!(
        run,
        "Pay" | "Sacrifice" | "Discard" | "Tap" | "Exile" | "Return" | "Remove"
    );
    if capital && !opens && !cost_verb && run.chars().count() > 1 {
        return Token::Name(run.replace('\u{2019}', "'"));
    }
    Token::Word(run.replace('\u{2019}', "'").to_lowercase())
}

/// The tokens of a clause's normalized text.
pub fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut at = 0;
    // A sentence opens at the start of the text, after a period, a colon, a dash or a bullet and on a new line.
    let mut opens = true;
    while let Some(&c) = chars.get(at) {
        if c.is_whitespace() {
            opens |= c == '\n';
            at += 1;
        } else if c == '{'
            && let Some(len) = chars.iter().skip(at).position(|&x| x == '}')
        {
            tokens.push(Token::Symbol(
                chars.iter().skip(at + 1).take(len - 1).collect(),
            ));
            at += len + 1;
            opens = false;
        } else if c == '~' {
            tokens.push(Token::Source);
            at += 1;
            opens = false;
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
            tokens.push(word_or_number(&run, opens));
            opens = false;
        } else {
            tokens.push(Token::Punct(c));
            // A quote that opens an ability starts a sentence of its own.
            let opening =
                c == '"' && (at == 0 || chars.get(at - 1).is_some_and(|c| c.is_whitespace()));
            opens = if c == '"' {
                opening || opens
            } else {
                matches!(c, '.' | ':' | '\u{2014}' | '\u{2022}')
            };
            at += 1;
        }
    }
    merge_source(tokens)
}

/// "this" and a noun for the card ("this creature", "this Aura") become the source token.
fn merge_source(tokens: Vec<Token>) -> Vec<Token> {
    let mut merged = Vec::with_capacity(tokens.len());
    let mut iter = tokens.into_iter().peekable();
    while let Some(token) = iter.next() {
        let this = matches!(&token, Token::Word(word) if word == "this");
        // "This creature's power": the possessive stays after the source.
        let (noun, possessive) = match iter.peek() {
            Some(Token::Word(word) | Token::Name(word)) => {
                let word = word.to_lowercase();
                let stem = word.strip_suffix("'s");
                (
                    SOURCE_NOUNS.contains(&stem.unwrap_or(word.as_str())),
                    stem.is_some(),
                )
            }
            _ => (false, false),
        };
        if this && noun {
            iter.next();
            merged.push(Token::Source);
            if possessive {
                merged.push(Token::Punct('\''));
                merged.push(Token::Word("s".to_owned()));
            }
        } else {
            merged.push(token);
        }
    }
    merged
}

/// A reader over a clause's tokens, with the targets the clause has asked for so far. The flags are independent facts
/// about the ability being read.
#[allow(clippy::struct_excessive_bools)]
pub struct Reader<'a> {
    tokens: &'a [Token],
    at: usize,
    /// The targets the clause asks for; `Ref::Target(i)` is the i-th.
    pub targets: Vec<TargetSpec>,
    /// Whether the card's mana cost has an `{X}`, so `X` can stand for an amount.
    pub x_free: bool,
    /// What "it" and "its" mean: the object the last sentence acted on.
    pub it: Option<Ref>,
    /// Whether the event of the trigger is about a player: "that player" is who cast the spell, drew the card,
    /// lost the life or was dealt the damage.
    pub that_player: bool,
    /// Whether the event of the trigger is about one permanent: "that permanent's controller" is who controls it.
    pub that_permanent: bool,
    /// Whether the event of the trigger is the card itself entering, attacking or dealing damage: "it" in a keyword
    /// action ("it explores") is the card.
    pub acts_itself: bool,
    /// The subtypes on the card's own type line, which are subtypes whether or not the rules' list has them.
    pub subtypes: Vec<String>,
    /// The power printed on the card when it is a number, which a characteristic-defining ability of the toughness
    /// alone leaves as it is.
    pub power: Option<i32>,
    /// The toughness printed on the card when it is a number, which an ability that defines the power alone leaves.
    pub toughness: Option<i32>,
    /// Whether the trigger is about an attack at the monarch, so "that player controls" is the monarch's.
    pub at_monarch: bool,
    /// Whether the trigger is about the enchanted player being attacked, so "each opponent attacking that player"
    /// names the opponents with a creature attacking them.
    pub attacked: bool,
    /// Whether the ability works from the graveyard (its event is not about the card itself), so "return this card
    /// from your graveyard to the battlefield" has a card to return.
    pub from_graveyard: bool,
    /// Whether the event of the trigger is a spell being cast, so "it" and "that spell" in a copy are that spell.
    pub cast_spell: bool,
    /// Whether the event of the trigger is the card itself attacking, so "it" is the card.
    pub attacker: bool,
    /// Whether the sentence ends with ", where X is its power" (a characteristic of an object), so `X` in it stands
    /// for that amount, which `where_x::bind` puts in once the effects are read.
    pub where_x: bool,
    /// The object that "its" and "that creature" name in a where clause when no sentence named one: the card in an
    /// activation and in a trigger of its own event, the creature of the event in a trigger of another's.
    pub subject: Option<Ref>,
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
            that_player: false,
            that_permanent: false,
            acts_itself: false,
            subtypes: Vec::new(),
            power: None,
            toughness: None,
            at_monarch: false,
            attacked: false,
            from_graveyard: false,
            cast_spell: false,
            attacker: false,
            where_x: false,
            subject: None,
        }
    }

    /// Reads the `X` of an amount, which stands for the X the spell or ability announced or for the amount of the
    /// sentence's where clause.
    pub fn x(&mut self) -> bool {
        (self.x_free || self.where_x) && self.word("x")
    }

    /// Whether the word is a subtype: one of the rules' list or of the card's own type line.
    pub fn is_subtype(&self, word: &str) -> bool {
        is_subtype(word) || self.subtypes.iter().any(|own| own == word)
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

    /// Reads a capitalized word inside a sentence, as written.
    pub fn name(&mut self) -> Option<&'a str> {
        let Some(Token::Name(name)) = self.tokens.get(self.at) else {
            return None;
        };
        self.at += 1;
        Some(name.as_str())
    }

    /// Reads the capitalized word that opens a sentence ("Sliver creatures you control get +1/+1"), where the capital is
    /// grammar and the tokenizer wrote a plain word; the caller judges whether it is a name.
    pub fn opening_name(&mut self) -> Option<String> {
        let opens = self.at == 0
            || matches!(
                self.tokens.get(self.at - 1),
                Some(Token::Punct('.' | ':' | '\u{2014}' | '\u{2022}'))
            );
        let Some(Token::Word(word)) = self.tokens.get(self.at).filter(|_| opens) else {
            return None;
        };
        let mut letters = word.chars();
        let first = letters.next()?;
        self.at += 1;
        Some(first.to_uppercase().chain(letters).collect())
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

    /// Whether this punctuation mark is next, without reading it.
    pub fn at_punct(&self, mark: char) -> bool {
        self.tokens.get(self.at) == Some(&Token::Punct(mark))
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

    /// Reads what is between a pair of quotes with a reader of its own over those tokens, which `read` must read
    /// whole: the ability a copy or a permanent is given ("it has \"When ~ dies, draw a card.\"").
    pub fn quoted<T>(&mut self, read: impl FnOnce(&mut Reader<'a>) -> Option<T>) -> Option<T> {
        self.attempt(|r| {
            r.punct('"').then_some(())?;
            let tokens = r.tokens;
            let rest = tokens.get(r.at..)?;
            let close = r.at + rest.iter().position(|token| *token == Token::Punct('"'))?;
            let mut inner = Reader::new(tokens.get(r.at..close)?, r.x_free);
            inner.subtypes.clone_from(&r.subtypes);
            let found = read(&mut inner)?;
            inner.end().then_some(())?;
            r.at = close + 1;
            Some(found)
        })
    }

    /// Reads with `read` and puts the reader back where it was, with the same targets, whether it read or not.
    pub fn lookahead<T>(&mut self, read: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let (at, targets, it) = (self.at, self.targets.len(), self.it.clone());
        let found = read(self);
        self.at = at;
        self.targets.truncate(targets);
        self.it = it;
        found
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
