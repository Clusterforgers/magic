//! Selectors: what a clause targets, counts or acts on, read from "target nonblack creature" and
//! "all creatures you control" and written back the same way.
//!
//! A phrase is adjectives, a noun (card types, "permanent" or "spell") and suffixes. The selector
//! lists the noun's predicate first (a spell's is `Spell`, then its adjectives and types), then the
//! adjectives in the order they are printed ("other" last, where the files put it), then the suffixes.

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;

use super::amounts::{count_word, number_word, stat_name};
use super::subtypes::{plural_subtype as plural_subtype_word, singular_subtype};
use super::tokens::Reader;
use crate::compile::keywords;
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::value::{Amount, Cmp, StatKind};

const COLORS: [(&str, Color); 5] = [
    ("white", Color::White),
    ("blue", Color::Blue),
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
];

const TYPES: [(&str, CardType); 8] = [
    ("creature", CardType::Creature),
    ("artifact", CardType::Artifact),
    ("enchantment", CardType::Enchantment),
    ("land", CardType::Land),
    ("planeswalker", CardType::Planeswalker),
    ("battle", CardType::Battle),
    ("instant", CardType::Instant),
    ("sorcery", CardType::Sorcery),
];

/// The keywords a creature can be given or asked for by name; each reads and writes as plain words.
const KEYWORDS: &[&str] = &[
    "first strike",
    "double strike",
    "flying",
    "reach",
    "deathtouch",
    "trample",
    "lifelink",
    "vigilance",
    "haste",
    "menace",
    "indestructible",
    "hexproof",
    "shroud",
    "defender",
    "prowess",
    "changeling",
    "infect",
];

/// How a phrase is introduced, which decides its number, how it joins card types and how it names
/// the opponents who control something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// After "target": "creature an opponent controls".
    Target,
    /// After "up to two target" or "any number of target": "creatures your opponents control".
    Targets,
    /// After "each": "creature your opponents control".
    Each,
    /// After "all", or as the subject of a sentence: "artifacts and enchantments you control".
    All,
    /// After "a", "an" or "another" as the subject of a trigger: "creature an opponent controls".
    Subject,
}

impl Form {
    const fn plural(self) -> bool {
        matches!(self, Form::Targets | Form::All)
    }

    const fn connector(self) -> &'static str {
        if matches!(self, Form::All) {
            "and"
        } else {
            "or"
        }
    }

    const fn opponents(self) -> &'static str {
        if matches!(self, Form::Target | Form::Subject) {
            "an opponent controls"
        } else {
            "your opponents control"
        }
    }
}

pub(super) fn color(word: &str) -> Option<Color> {
    COLORS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, color)| *color)
}

pub(super) fn color_name(color: Color) -> &'static str {
    COLORS
        .iter()
        .find(|(_, candidate)| *candidate == color)
        .map_or("", |(name, _)| name)
}

pub(super) fn card_type(word: &str) -> Option<CardType> {
    TYPES
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, card_type)| *card_type)
}

pub(super) fn type_name(card_type: CardType) -> &'static str {
    TYPES
        .iter()
        .find(|(_, candidate)| *candidate == card_type)
        .map_or("", |(name, _)| name)
}

fn plural_of(noun: &str) -> String {
    if noun == "sorcery" {
        "sorceries".to_owned()
    } else {
        format!("{noun}s")
    }
}

/// A card type in the singular or the plural; true for the plural.
fn type_noun(word: &str) -> Option<(CardType, bool)> {
    if let Some(card_type) = card_type(word) {
        return Some((card_type, false));
    }
    let singular = if word == "sorceries" {
        "sorcery"
    } else {
        word.strip_suffix('s')?
    };
    card_type(singular).map(|card_type| (card_type, true))
}

fn not(pred: Pred) -> Pred {
    Pred::Not(Box::new(pred))
}

/// What an adjective before the noun says; "another" stands for "other" after an article.
pub(super) fn adjective(word: &str, form: Form) -> Option<Pred> {
    if word == "another" {
        return (form == Form::Subject).then_some(Pred::Other);
    }
    if let Some(color) = color(word) {
        return Some(Pred::Color(color));
    }
    if let Some(rest) = word.strip_prefix("non") {
        if let Some(color) = color(rest) {
            return Some(not(Pred::Color(color)));
        }
        if let Some(card_type) = card_type(rest) {
            return Some(not(Pred::Type(card_type)));
        }
        return match rest {
            "legendary" => Some(not(Pred::Supertype(Supertype::Legendary))),
            "basic" => Some(not(Pred::Supertype(Supertype::Basic))),
            "token" => Some(not(Pred::Token)),
            _ => None,
        };
    }
    Some(match word {
        "colorless" => Pred::Colorless,
        "tapped" => Pred::Tapped,
        "untapped" => Pred::Untapped,
        "attacking" => Pred::Attacking,
        "blocking" => Pred::Blocking,
        "legendary" => Pred::Supertype(Supertype::Legendary),
        "basic" => Pred::Supertype(Supertype::Basic),
        "other" => Pred::Other,
        _ => return None,
    })
}

/// The adjective for a predicate.
pub(super) fn adjective_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::Color(color) => color_name(*color).to_owned(),
        Pred::Colorless => "colorless".to_owned(),
        Pred::Tapped => "tapped".to_owned(),
        Pred::Untapped => "untapped".to_owned(),
        Pred::Attacking => "attacking".to_owned(),
        Pred::Blocking => "blocking".to_owned(),
        Pred::Other if form == Form::Subject => "another".to_owned(),
        Pred::Other => "other".to_owned(),
        Pred::Supertype(Supertype::Legendary) => "legendary".to_owned(),
        Pred::Supertype(Supertype::Basic) => "basic".to_owned(),
        Pred::Not(inner) => match &**inner {
            Pred::Color(color) => format!("non{}", color_name(*color)),
            Pred::Type(card_type) => format!("non{}", type_name(*card_type)),
            Pred::Supertype(Supertype::Legendary) => "nonlegendary".to_owned(),
            Pred::Supertype(Supertype::Basic) => "nonbasic".to_owned(),
            Pred::Token => "nontoken".to_owned(),
            _ => return None,
        },
        _ => return None,
    })
}

/// The card types a predicate names, when it names only card types.
fn type_preds(pred: &Pred) -> Option<Vec<CardType>> {
    let types: Vec<CardType> = match pred {
        Pred::Type(card_type) => vec![*card_type],
        Pred::AnyOf(list) => list
            .iter()
            .map(|pred| match pred {
                Pred::Type(card_type) => Some(*card_type),
                _ => None,
            })
            .collect::<Option<_>>()?,
        _ => return None,
    };
    (!types.is_empty()).then_some(types)
}

/// A keyword by its words ("flying", "first strike").
pub fn keyword(r: &mut Reader<'_>) -> Option<Keyword> {
    let name = KEYWORDS.iter().find(|name| r.words(name))?;
    keywords::read(name)
}

/// The words of a keyword the readers know.
pub fn keyword_text(keyword: &Keyword) -> Option<String> {
    let name = keyword.name().to_lowercase();
    KEYWORDS.contains(&name.as_str()).then_some(name)
}

/// What one phrase of a list of keywords gives: a keyword by its words, or "protection from white and from black", one
/// protection for each quality.
pub fn granted_keywords(r: &mut Reader<'_>) -> Option<Vec<Keyword>> {
    if !r.words("protection from") {
        return keyword(r).map(|keyword| vec![keyword]);
    }
    let mut found = vec![protection_from(r)?];
    while let Some(next) = r.attempt(|r| {
        r.need("and from")?;
        protection_from(r)
    }) {
        found.push(next);
    }
    Some(found)
}

fn protection_from(r: &mut Reader<'_>) -> Option<Keyword> {
    let quality = keywords::quality(r.peek_word()?)?;
    r.skip();
    Some(Keyword::Protection(quality))
}

/// The keywords given, written as `granted_keywords` reads them: "flying and haste", "protection from white and from
/// black". `None` when a modification is not a keyword the readers know.
pub fn granted_text(mods: &[Modification]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut protecting = false;
    for modification in mods {
        let Modification::AddKeyword(keyword) = modification else {
            return None;
        };
        if let Keyword::Protection(quality) = keyword {
            let word = keywords::quality_word(quality)?;
            match parts.last_mut() {
                Some(last) if protecting => {
                    last.push_str(" and from ");
                    last.push_str(word);
                }
                _ => parts.push(format!("protection from {word}")),
            }
            protecting = true;
        } else {
            parts.push(keyword_text(keyword)?);
            protecting = false;
        }
    }
    Some(parts.join(" and "))
}

fn stat_word(r: &mut Reader<'_>) -> Option<StatKind> {
    if r.word("power") {
        Some(StatKind::Power)
    } else if r.word("toughness") {
        Some(StatKind::Toughness)
    } else if r.words("mana value") {
        Some(StatKind::ManaValue)
    } else {
        None
    }
}

/// What may follow the noun: who controls it, a keyword or a number it has.
pub(super) fn suffix(r: &mut Reader<'_>, form: Form) -> Option<Pred> {
    if r.words("you control") {
        return Some(Pred::ControlledBy(Ref::You));
    }
    if r.words("you don't control") {
        return Some(not(Pred::ControlledBy(Ref::You)));
    }
    if r.words(form.opponents()) {
        return Some(Pred::ControlledBy(Ref::Opponents));
    }
    if r.words("enchanted player controls") {
        return Some(Pred::ControlledBy(Ref::AttachedPlayer));
    }
    if r.words("the monarch controls") || (r.at_monarch && r.words("that player controls")) {
        return Some(Pred::ControlledBy(Ref::Monarch));
    }
    // The player a trigger is about: "planeswalker that player controls".
    if r.that_player && r.words("that player controls") {
        return Some(Pred::ControlledBy(Ref::Triggering));
    }
    r.attempt(|r| {
        r.need("with")?;
        if let Some(keyword) = keyword(r) {
            return Some(Pred::HasKeyword(keyword));
        }
        let stat = stat_word(r)?;
        // "mana value X or less": the X of the card's own cost.
        let to = if r.x_free && r.word("x") {
            Amount::X
        } else {
            Amount::Fixed(i32::try_from(r.number()?).ok()?)
        };
        let op = if r.words("or less") {
            Cmp::Le
        } else if r.words("or greater") {
            Cmp::Ge
        } else {
            Cmp::Eq
        };
        Some(Pred::Compare { stat, op, to })
    })
}

pub(super) fn suffix_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::ControlledBy(Ref::You) => "you control".to_owned(),
        Pred::ControlledBy(Ref::Opponents) => form.opponents().to_owned(),
        Pred::ControlledBy(Ref::Triggering) => "that player controls".to_owned(),
        Pred::ControlledBy(Ref::AttachedPlayer) => "enchanted player controls".to_owned(),
        Pred::ControlledBy(Ref::Monarch) => "the monarch controls".to_owned(),
        Pred::Not(inner) if **inner == Pred::ControlledBy(Ref::You) => {
            "you don't control".to_owned()
        }
        Pred::HasKeyword(keyword) => format!("with {}", keyword_text(keyword)?),
        Pred::Compare { stat, op, to } => {
            let number = match to {
                Amount::Fixed(number) => number.to_string(),
                Amount::X => "X".to_owned(),
                _ => return None,
            };
            let tail = match op {
                Cmp::Le => " or less",
                Cmp::Ge => " or greater",
                Cmp::Eq => "",
                _ => return None,
            };
            format!("with {} {number}{tail}", stat_name(*stat)?)
        }
        _ => return None,
    })
}

/// A subtype as it is written ("Clue", "Soldier"), of the rules' list or of the card's own type line; any other
/// capitalized word is not read.
pub fn subtype(r: &mut Reader<'_>) -> Option<String> {
    r.attempt(|r| {
        let name = r.name()?;
        r.is_subtype(name).then(|| name.to_owned())
    })
}

/// A group of one subtype as it is written in the plural ("Heroes", "Elves", "Spies"): the subtype in the singular.
fn plural_subtype(r: &mut Reader<'_>) -> Option<String> {
    r.attempt(|r| {
        let name = r.name()?;
        singular_subtype(name, |one| r.is_subtype(one))
    })
}

/// What ends a phrase: a card type or "permanent", "spell", "card", or a subtype.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Noun {
    /// A card type or "permanent".
    Plain,
    /// "spell".
    Spell,
    /// "card": always followed by the zone it is in.
    Card,
    /// A subtype alone ("Clue"), singular only.
    Subtype(String),
}

/// The card types before the noun and the noun; "permanent" names no type.
fn head(r: &mut Reader<'_>, form: Form) -> Option<(Vec<CardType>, Noun)> {
    let plural = form.plural();
    let group = if plural {
        plural_subtype(r)
    } else {
        subtype(r)
    };
    if let Some(name) = group {
        return Some((Vec::new(), Noun::Subtype(name)));
    }
    let mut types = Vec::new();
    let mut forms = Vec::new();
    while let Some((card_type, is_plural)) = r.peek_word().and_then(type_noun) {
        r.skip();
        types.push(card_type);
        forms.push(is_plural);
        let more =
            r.word_at(0) == Some(form.connector()) && r.word_at(1).and_then(type_noun).is_some();
        if !more {
            break;
        }
        r.skip();
    }
    let singular_types = forms.iter().all(|is_plural| !is_plural);
    match r.peek_word() {
        Some(word @ ("spell" | "spells")) if singular_types && (word == "spells") == plural => {
            r.skip();
            Some((types, Noun::Spell))
        }
        Some(word @ ("card" | "cards")) if singular_types && (word == "cards") == plural => {
            r.skip();
            Some((types, Noun::Card))
        }
        Some(word @ ("permanent" | "permanents"))
            if types.is_empty() && (word == "permanents") == plural =>
        {
            r.skip();
            Some((types, Noun::Plain))
        }
        _ if !types.is_empty() && forms.iter().all(|is_plural| *is_plural == plural) => {
            Some((types, Noun::Plain))
        }
        _ => None,
    }
}

/// "from your graveyard", "from your hand": the zone and its owner.
fn zone_suffix(r: &mut Reader<'_>) -> Option<[Pred; 2]> {
    if r.words("from your hand") {
        return Some([Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
    }
    r.words("from your graveyard")
        .then_some([Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)])
}

/// A phrase naming a set of objects ("nonblack creature", "creatures you control", "noncreature spell").
pub fn phrase(r: &mut Reader<'_>, form: Form) -> Option<Selector> {
    r.attempt(|r| {
        let mut adjectives = Vec::new();
        while let Some(pred) = r.peek_word().and_then(|word| adjective(word, form)) {
            r.skip();
            adjectives.push(pred);
        }
        // The files put "other" after the adjectives it is printed before ("other legendary creatures").
        adjectives.sort_by_key(|pred| *pred == Pred::Other);
        let (types, noun) = head(r, form)?;
        let type_pred = match types.as_slice() {
            [] => None,
            [only] => Some(Pred::Type(*only)),
            many => Some(Pred::AnyOf(many.iter().copied().map(Pred::Type).collect())),
        };
        let mut preds = Vec::new();
        match &noun {
            Noun::Spell => {
                preds.push(Pred::Spell);
                preds.extend(adjectives);
                preds.extend(type_pred);
            }
            Noun::Subtype(name) => {
                preds.push(Pred::Subtype(Subtype(name.clone())));
                preds.extend(adjectives);
            }
            Noun::Plain | Noun::Card => {
                preds.extend(type_pred);
                preds.extend(adjectives);
            }
        }
        loop {
            if let Some(zone) = zone_suffix(r) {
                preds.extend(zone);
            } else if let Some(pred) = suffix(r, form) {
                preds.push(pred);
            } else {
                break;
            }
        }
        let placed = preds.iter().any(|pred| matches!(pred, Pred::In(_)));
        (noun != Noun::Card || placed).then_some(Selector(preds))
    })
}

/// A selector as a phrase, the way `phrase` reads it.
pub fn phrase_text(selector: &Selector, form: Form) -> Option<String> {
    let (spell, preds) = match selector.0.split_first() {
        Some((Pred::Spell, rest)) => (true, rest),
        _ => (false, selector.0.as_slice()),
    };
    let (mut adjectives, mut types, mut suffixes) = (Vec::new(), Vec::new(), Vec::new());
    let (mut subtype, mut card) = (None, false);
    let mut rest = preds.iter().peekable();
    while let Some(pred) = rest.next() {
        if let Pred::In(zone @ (ZoneKind::Graveyard | ZoneKind::Hand)) = pred
            && rest
                .next_if(|next| **next == Pred::OwnedBy(Ref::You))
                .is_some()
        {
            card = true;
            suffixes.push(if *zone == ZoneKind::Hand {
                "from your hand".to_owned()
            } else {
                "from your graveyard".to_owned()
            });
        } else if let Pred::Subtype(Subtype(name)) = pred {
            if subtype.replace(name.as_str()).is_some() {
                return None;
            }
        } else if let Some(word) = adjective_text(pred, form) {
            if *pred == Pred::Other {
                adjectives.insert(0, word);
            } else {
                adjectives.push(word);
            }
        } else if let Some(found) = type_preds(pred) {
            if !types.is_empty() {
                return None;
            }
            types = found;
        } else {
            suffixes.push(suffix_text(pred, form)?);
        }
    }
    let plural = form.plural();
    // A subtype is the noun by itself; a spell is not a card in a zone.
    if (subtype.is_some() && (spell || card || !types.is_empty())) || (spell && card) {
        return None;
    }
    let names: Vec<String> = types
        .iter()
        .map(|card_type| {
            let name = type_name(*card_type);
            if plural && !spell && !card {
                plural_of(name)
            } else {
                name.to_owned()
            }
        })
        .collect();
    let mut words = adjectives;
    words.extend(subtype.map(|name| {
        if plural {
            plural_subtype_word(name)
        } else {
            name.to_owned()
        }
    }));
    if !names.is_empty() {
        words.push(names.join(&format!(" {} ", form.connector())));
    }
    if spell {
        words.push(if plural { "spells" } else { "spell" }.to_owned());
    } else if card {
        words.push(if plural { "cards" } else { "card" }.to_owned());
    } else if names.is_empty() && subtype.is_none() {
        words.push(if plural { "permanents" } else { "permanent" }.to_owned());
    }
    words.extend(suffixes);
    Some(words.join(" "))
}

fn plain(what: Targetable) -> TargetSpec {
    TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    }
}

/// "player" or "opponent", and "or planeswalker" after it.
fn player_kind(r: &mut Reader<'_>, player: Targetable) -> Targetable {
    if r.words("or planeswalker") {
        let planeswalker = Selector(vec![Pred::Type(CardType::Planeswalker)]);
        Targetable::Either(vec![player, Targetable::Object(planeswalker)])
    } else {
        player
    }
}

fn spec(r: &mut Reader<'_>) -> Option<TargetSpec> {
    if r.words("any target") {
        return Some(plain(Targetable::AnyTarget));
    }
    if r.words("any number of target") {
        let what = Targetable::Object(phrase(r, Form::Targets)?);
        let bounds = Bounds { min: 0, max: None };
        return Some(TargetSpec {
            what,
            bounds,
            divide: None,
        });
    }
    if r.words("up to") {
        let count = number_word(r.take_word()?)?;
        r.need("target")?;
        let form = if count == 1 {
            Form::Target
        } else {
            Form::Targets
        };
        let what = Targetable::Object(phrase(r, form)?);
        let max = Some(Amount::Fixed(i32::try_from(count).ok()?));
        return Some(TargetSpec {
            what,
            bounds: Bounds { min: 0, max },
            divide: None,
        });
    }
    r.need("target")?;
    let what = if r.word("player") {
        player_kind(r, Targetable::Player)
    } else if r.word("opponent") {
        player_kind(r, Targetable::Opponent)
    } else {
        Targetable::Object(phrase(r, Form::Target)?)
    };
    Some(plain(what))
}

/// A "target ..." phrase, which the clause then holds as its one target; later words refer to it
/// as `Target(0)`. A second target is not read.
pub fn target(r: &mut Reader<'_>) -> Option<Ref> {
    if !r.targets.is_empty() {
        return None;
    }
    let spec = r.attempt(spec)?;
    r.targets.push(spec);
    Some(Ref::Target(0))
}

/// "all creatures": every object matching a plural phrase.
pub fn all(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("all")?;
        phrase(r, Form::All).map(Ref::Each)
    })
}

/// "each creature": every object matching a singular phrase.
pub fn each(r: &mut Reader<'_>) -> Option<Ref> {
    r.attempt(|r| {
        r.need("each")?;
        phrase(r, Form::Each).map(Ref::Each)
    })
}

/// A target as the card words it.
pub fn spec_text(spec: &TargetSpec) -> Option<String> {
    if spec.divide.is_some() {
        return None;
    }
    if spec.bounds.is_one() {
        return Some(match &spec.what {
            Targetable::AnyTarget => "any target".to_owned(),
            Targetable::Player => "target player".to_owned(),
            Targetable::Opponent => "target opponent".to_owned(),
            Targetable::Either(kinds) => match kinds.as_slice() {
                [Targetable::Player, Targetable::Object(planeswalker)]
                    if planeswalker.0 == [Pred::Type(CardType::Planeswalker)] =>
                {
                    "target player or planeswalker".to_owned()
                }
                [Targetable::Opponent, Targetable::Object(planeswalker)]
                    if planeswalker.0 == [Pred::Type(CardType::Planeswalker)] =>
                {
                    "target opponent or planeswalker".to_owned()
                }
                _ => return None,
            },
            Targetable::Object(selector) => {
                let phrase = phrase_text(selector, Form::Target)?;
                // The card puts "another" before "target": "another target attacking creature".
                match phrase.strip_prefix("other ") {
                    Some(rest) => format!("another target {rest}"),
                    None => format!("target {phrase}"),
                }
            }
        });
    }
    let Targetable::Object(selector) = &spec.what else {
        return None;
    };
    if spec.bounds.min != 0 {
        return None;
    }
    match &spec.bounds.max {
        None => Some(format!(
            "any number of target {}",
            phrase_text(selector, Form::Targets)?
        )),
        Some(Amount::Fixed(1)) => Some(format!(
            "up to one target {}",
            phrase_text(selector, Form::Target)?
        )),
        Some(Amount::Fixed(count)) => Some(format!(
            "up to {} target {}",
            count_word(u32::try_from(*count).ok()?)?,
            phrase_text(selector, Form::Targets)?
        )),
        Some(_) => None,
    }
}
