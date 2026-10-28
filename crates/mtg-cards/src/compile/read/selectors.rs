//! Selectors: what a clause targets, counts or acts on, read from "target nonblack creature" and
//! "all creatures you control" and written back the same way.
//!
//! A phrase is adjectives, a noun (card types, "permanent" or "spell") and suffixes. The selector
//! lists the noun's predicate first (a spell's is `Spell`, then its adjectives and types), then the
//! adjectives in the order they are printed ("other" last, where the files put it), then the suffixes.

use std::iter::Peekable;
use std::slice::Iter;

use mtg_core::card::{CardType, Subtype, Supertype};
use mtg_core::color::Color;

use super::amounts::{count_word, number_word, stat_name};
use super::counters;
use super::subtypes::{plural_subtype as plural_subtype_word, singular_subtype};
use super::tokens::Reader;
use crate::compile::keywords;
use crate::cost::{Cost, CostItem};
use crate::keyword::Keyword;
use crate::modify::Modification;
use crate::refs::{Bounds, Pred, Ref, Selector, TargetSpec, Targetable, ZoneKind};
use crate::value::{Amount, Cmp, CounterKind, StatKind};

const COLORS: [(&str, Color); 5] = [
    ("white", Color::White),
    ("blue", Color::Blue),
    ("black", Color::Black),
    ("red", Color::Red),
    ("green", Color::Green),
];

/// The card types a permanent can have (CR 110.4), which "permanent spell" names.
const PERMANENT_TYPES: [CardType; 6] = [
    CardType::Artifact,
    CardType::Battle,
    CardType::Creature,
    CardType::Enchantment,
    CardType::Land,
    CardType::Planeswalker,
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
    "intimidate",
    "infect",
    "skulk",
    "horsemanship",
    "melee",
    "myriad",
    "fear",
    "intimidate",
    "exalted",
    "extort",
    "evolve",
    "persist",
    "undying",
    "wither",
    "flash",
    "shadow",
    "riot",
    "decayed",
    "exploit",
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
    /// After "for each": "creature you control", "creature card in your graveyard".
    Count,
}

impl Form {
    const fn plural(self) -> bool {
        matches!(self, Form::Targets | Form::All)
    }

    const fn connector(self) -> &'static str {
        match self {
            Form::All => "and",
            Form::Targets => "and/or",
            _ => "or",
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

/// At least two colors (CR 105.4).
fn multicolored() -> Pred {
    Pred::Compare {
        stat: StatKind::Colors,
        op: Cmp::Ge,
        to: Amount::Fixed(2),
    }
}

/// Exactly one color (CR 105.4).
fn monocolored() -> Pred {
    Pred::Compare {
        stat: StatKind::Colors,
        op: Cmp::Eq,
        to: Amount::Fixed(1),
    }
}

/// An attacking creature or a blocking one.
fn attacking_or_blocking() -> Pred {
    Pred::AnyOf(vec![Pred::Attacking, Pred::Blocking])
}

/// An artifact, a legendary object or a Saga (CR 700.6).
fn historic() -> Pred {
    Pred::AnyOf(vec![
        Pred::Type(CardType::Artifact),
        Pred::Supertype(Supertype::Legendary),
        Pred::Subtype(Subtype("Saga".to_owned())),
    ])
}

/// An Assassin, Mercenary, Pirate, Rogue or Warlock (CR 205.3m).
fn outlaw() -> Pred {
    Pred::AnyOf(
        ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"]
            .into_iter()
            .map(|name| Pred::Subtype(Subtype(name.to_owned())))
            .collect(),
    )
}

/// Held by the player the first target is, or by the controller of the permanent it is: "that player or that
/// planeswalker's controller".
fn target_or_its_controller() -> Vec<Pred> {
    vec![
        Pred::ControlledBy(Ref::Target(0)),
        Pred::ControlledBy(Ref::ControllerOf(Box::new(Ref::Target(0)))),
    ]
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
        "multicolored" => multicolored(),
        "monocolored" => monocolored(),
        "historic" => historic(),
        "outlaw" => outlaw(),
        "tapped" => Pred::Tapped,
        "untapped" => Pred::Untapped,
        "attacking" => Pred::Attacking,
        "blocking" => Pred::Blocking,
        "unblocked" => Pred::Unblocked,
        "legendary" => Pred::Supertype(Supertype::Legendary),
        "basic" => Pred::Supertype(Supertype::Basic),
        "other" => Pred::Other,
        _ => return None,
    })
}

/// The adjective at the reader: a word of `adjective`, "commander" before a card type ("Commander creatures you
/// control"), or "non-" with a subtype ("Non-Archon creatures").
fn next_adjective(r: &Reader<'_>, form: Form) -> Option<Pred> {
    let word = r.peek_word()?;
    if word == "commander" && r.word_at(1).and_then(type_noun).is_some() {
        return Some(Pred::Commander);
    }
    if word.starts_with("non-") {
        return non_subtype(r, word);
    }
    adjective(word, form)
}

/// The adjective for a predicate.
pub(super) fn adjective_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::Color(color) => color_name(*color).to_owned(),
        Pred::Colorless => "colorless".to_owned(),
        pred if *pred == multicolored() => "multicolored".to_owned(),
        pred if *pred == monocolored() => "monocolored".to_owned(),
        pred if *pred == attacking_or_blocking() => "attacking or blocking".to_owned(),
        pred if *pred == historic() => "historic".to_owned(),
        pred if *pred == outlaw() => "outlaw".to_owned(),
        Pred::Tapped => "tapped".to_owned(),
        Pred::Untapped => "untapped".to_owned(),
        Pred::Attacking => "attacking".to_owned(),
        Pred::Blocking => "blocking".to_owned(),
        Pred::Unblocked => "unblocked".to_owned(),
        Pred::Other if form == Form::Subject => "another".to_owned(),
        Pred::Other => "other".to_owned(),
        Pred::Supertype(Supertype::Legendary) => "legendary".to_owned(),
        Pred::Supertype(Supertype::Basic) => "basic".to_owned(),
        Pred::Not(inner) if **inner == outlaw() => "non-outlaw".to_owned(),
        Pred::Not(inner) => match &**inner {
            Pred::Color(color) => format!("non{}", color_name(*color)),
            Pred::Type(card_type) => format!("non{}", type_name(*card_type)),
            Pred::Supertype(Supertype::Legendary) => "nonlegendary".to_owned(),
            Pred::Supertype(Supertype::Basic) => "nonbasic".to_owned(),
            Pred::Subtype(subtype) => format!("non-{}", subtype.0),
            Pred::Token => "nontoken".to_owned(),
            Pred::Commander => "noncommander".to_owned(),
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
    if let Some(name) = KEYWORDS.iter().find(|name| r.words(name)) {
        return keywords::read(name);
    }
    // "islandwalk", "swampwalk": the land type is part of the word.
    let word = r.peek_word().filter(|word| word.ends_with("walk"))?;
    let found = keywords::read(word).filter(|found| matches!(found, Keyword::Landwalk(_)))?;
    r.skip();
    Some(found)
}

/// The words of a keyword the readers know.
pub fn keyword_text(keyword: &Keyword) -> Option<String> {
    if let Keyword::Landwalk(Selector(preds)) = keyword {
        return match preds.as_slice() {
            [Pred::Subtype(Subtype(land))] => Some(format!("{}walk", land.to_lowercase())),
            _ => None,
        };
    }
    let name = keyword.name().to_lowercase();
    KEYWORDS.contains(&name.as_str()).then_some(name)
}

/// What one phrase of a list of keywords gives: a keyword by its words, or "protection from white and from black", one
/// protection for each quality.
pub fn granted_keywords(r: &mut Reader<'_>) -> Option<Vec<Keyword>> {
    if r.word("ward") {
        return ward_cost(r).map(|cost| vec![Keyword::Ward(cost)]);
    }
    if let Some(found) = r.attempt(numbered_keyword) {
        return Some(vec![found]);
    }
    if r.word("crew") {
        return Some(vec![Keyword::Crew(r.number()?)]);
    }
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

/// The keywords a creature is given with a number after the word: "fabricate 1", "annihilator 2".
const NUMBERED_KEYWORDS: [&str; 4] = ["fabricate", "annihilator", "toxic", "afterlife"];

fn numbered_keyword(r: &mut Reader<'_>) -> Option<Keyword> {
    let name = NUMBERED_KEYWORDS
        .iter()
        .find(|name| r.peek_word() == Some(**name))?;
    r.skip();
    let number = r.number()?;
    keywords::read(&format!("{name} {number}"))
}

/// The words of a keyword with a number, as `numbered_keyword` reads them.
fn numbered_keyword_text(keyword: &Keyword) -> Option<String> {
    let number = match keyword {
        Keyword::Fabricate(number)
        | Keyword::Annihilator(number)
        | Keyword::Toxic(number)
        | Keyword::Afterlife(number) => *number,
        _ => return None,
    };
    Some(format!("{} {number}", keyword.name().to_lowercase()))
}

/// The mana after "ward": "{1}", "{2}{U}".
fn ward_cost(r: &mut Reader<'_>) -> Option<Cost> {
    let mut text = String::new();
    while let Some(symbol) = r.attempt(|r| {
        r.symbol()
            .filter(|symbol| !matches!(*symbol, "T" | "Q" | "X"))
    }) {
        text = format!("{text}{{{symbol}}}");
    }
    text.parse().ok().map(Cost::mana)
}

fn protection_from(r: &mut Reader<'_>) -> Option<Keyword> {
    if r.words("each color") {
        return Some(Keyword::Protection(keywords::each_color()));
    }
    // What the controller chose as the permanent entered: "protection from the chosen color", "... creatures of the
    // chosen type".
    if r.static_grant && r.words("the chosen color") {
        return Some(Keyword::Protection(Selector(vec![Pred::ChosenColor])));
    }
    if r.static_grant && r.words("creatures of the chosen type") {
        return Some(Keyword::Protection(chosen_creatures()));
    }
    if r.static_grant && r.words("the chosen player") {
        return Some(Keyword::Protection(chosen_player()));
    }
    if let Some(quality) = r.peek_word().and_then(keywords::quality) {
        r.skip();
        return Some(Keyword::Protection(quality));
    }
    // A creature type in the plural: "protection from Rats".
    let name = plural_subtype(r)?;
    Some(Keyword::Protection(Selector(vec![Pred::Subtype(Subtype(
        name,
    ))])))
}

/// The objects the player the controller chose controls.
fn chosen_player() -> Selector {
    Selector(vec![Pred::ControlledBy(Ref::Chosen)])
}

/// The creatures of the type the controller chose.
fn chosen_creatures() -> Selector {
    Selector(vec![Pred::Type(CardType::Creature), Pred::ChosenSubtype])
}

/// The words after "protection from" for a quality: a color or kind of source, "the chosen color", "creatures of the
/// chosen type", or a creature type in the plural.
fn protection_word(quality: &Selector) -> Option<String> {
    if let Some(word) = keywords::quality_word(quality) {
        return Some(word.to_owned());
    }
    if *quality == chosen_creatures() {
        return Some("creatures of the chosen type".to_owned());
    }
    if *quality == chosen_player() {
        return Some("the chosen player".to_owned());
    }
    match quality.0.as_slice() {
        [Pred::ChosenColor] => Some("the chosen color".to_owned()),
        [Pred::Subtype(Subtype(name))] => Some(plural_subtype_word(name)),
        _ => None,
    }
}

/// The keywords given, written as `granted_keywords` reads them: "flying and haste", "protection from white and from
/// black". `None` when a modification is not a keyword the readers know.
pub fn granted_text(mods: &[Modification]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    let mut protecting = false;
    for modification in mods {
        if *modification == Modification::AddProtectionFromChosenColor {
            parts.push("protection from the color of your choice".to_owned());
            protecting = false;
            continue;
        }
        let Modification::AddKeyword(keyword) = modification else {
            return None;
        };
        if let Keyword::Protection(quality) = keyword {
            let word = protection_word(quality)?;
            match parts.last_mut() {
                Some(last) if protecting => {
                    last.push_str(" and from ");
                    last.push_str(&word);
                }
                _ => parts.push(format!("protection from {word}")),
            }
            protecting = true;
        } else if let Keyword::Ward(Cost(items)) = keyword {
            let [CostItem::Mana(mana)] = items.as_slice() else {
                return None;
            };
            parts.push(format!("ward {mana}"));
            protecting = false;
        } else if let Some(numbered) = numbered_keyword_text(keyword) {
            parts.push(numbered);
            protecting = false;
        } else if let Keyword::Crew(power) = keyword {
            parts.push(format!("crew {power}"));
            protecting = false;
        } else {
            parts.push(keyword_text(keyword)?);
            protecting = false;
        }
    }
    // Cards write three or more as a list: "flying, vigilance, and haste".
    Some(match parts.as_slice() {
        [rest @ .., last] if rest.len() >= 2 => format!("{}, and {last}", rest.join(", ")),
        _ => parts.join(" and "),
    })
}

pub(super) fn stat_word(r: &mut Reader<'_>) -> Option<StatKind> {
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
    if r.words("defending player controls") {
        return Some(Pred::ControlledBy(Ref::Defending));
    }
    // What came onto the battlefield this turn: "creature token that entered the battlefield this turn".
    if r.words("that entered the battlefield this turn") || r.words("that entered this turn") {
        return Some(Pred::EnteredThisTurn);
    }
    if r.words("that attacked this turn") {
        return Some(Pred::AttackedThisTurn);
    }
    // The creatures that paid for the source: "creature that crewed it this turn".
    if r.words("that crewed it this turn") {
        return Some(Pred::Crewed(Ref::This));
    }
    if r.words("that saddled it this turn") {
        return Some(Pred::Saddled(Ref::This));
    }
    // What the source hurt: "creature dealt damage by this creature this turn".
    if r.attempt(|r| {
        r.need("dealt damage by")?;
        r.source().then_some(())?;
        r.need("this turn")
    })
    .is_some()
    {
        return Some(Pred::DamagedBy(Ref::This));
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
    if let Some(keyword) = r.attempt(|r| {
        r.need("without")?;
        keyword(r)
    }) {
        return Some(not(Pred::HasKeyword(keyword)));
    }
    r.attempt(|r| {
        r.need("with")?;
        if let Some(kind) = r.attempt(counter_on_it) {
            return Some(Pred::HasCounter(kind));
        }
        if let Some(keyword) = keyword(r) {
            return Some(Pred::HasKeyword(keyword));
        }
        if let Some(pred) = r.attempt(like_this) {
            return Some(pred);
        }
        let stat = stat_word(r)?;
        // The damage that triggered the ability: "mana value less than or equal to that damage".
        if stat == StatKind::ManaValue
            && r.triggered
            && r.words("less than or equal to that damage")
        {
            return Some(Pred::Compare {
                stat,
                op: Cmp::Le,
                to: Amount::Triggering,
            });
        }
        // "mana value X or less": the X of the card's own cost.
        let to = if r.x_free && !r.x_unpaid && r.word("x") {
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

/// "greater power", "power less than this creature's power": a stat compared with the same stat of the source.
fn like_this(r: &mut Reader<'_>) -> Option<Pred> {
    let (stat, op) = if r.words("greater power") {
        (StatKind::Power, Cmp::Gt)
    } else {
        let stat = stat_word(r)?;
        let op = if r.words("less than") {
            Cmp::Lt
        } else if r.words("greater than") {
            Cmp::Gt
        } else {
            return None;
        };
        r.source().then_some(())?;
        (r.punct('\'') && r.word("s")).then_some(())?;
        (stat_word(r)? == stat).then_some(())?;
        (stat, op)
    };
    Some(Pred::Compare {
        stat,
        op,
        to: Amount::Stat {
            of: Ref::This,
            stat,
        },
    })
}

pub(super) fn suffix_text(pred: &Pred, form: Form) -> Option<String> {
    Some(match pred {
        Pred::Compare {
            stat,
            op: op @ (Cmp::Lt | Cmp::Gt),
            to:
                Amount::Stat {
                    of: Ref::This,
                    stat: same,
                },
        } if stat == same => {
            let more = if *op == Cmp::Lt { "less" } else { "greater" };
            let name = stat_name(*stat)?;
            format!("with {name} {more} than ~'s {name}")
        }
        Pred::ControlledBy(Ref::You) => "you control".to_owned(),
        Pred::ControlledBy(Ref::Opponents) => form.opponents().to_owned(),
        Pred::ControlledBy(Ref::Triggering) => "that player controls".to_owned(),
        Pred::EnteredThisTurn => "that entered this turn".to_owned(),
        Pred::AttackedThisTurn => "that attacked this turn".to_owned(),
        Pred::Crewed(Ref::This) => "that crewed it this turn".to_owned(),
        Pred::Saddled(Ref::This) => "that saddled it this turn".to_owned(),
        Pred::DamagedBy(Ref::This) => "dealt damage by ~ this turn".to_owned(),
        Pred::ControlledBy(Ref::AttachedPlayer) => "enchanted player controls".to_owned(),
        Pred::ControlledBy(Ref::Defending) => "defending player controls".to_owned(),
        Pred::ControlledBy(Ref::Monarch) => "the monarch controls".to_owned(),
        Pred::ControlledBy(Ref::Target(0)) => "target player controls".to_owned(),
        Pred::ControlledBy(Ref::Target(1)) => "another target player controls".to_owned(),
        // The player target, or the controller of the planeswalker target.
        Pred::AnyOf(held) if *held == target_or_its_controller() => {
            "that player or that planeswalker's controller controls".to_owned()
        }
        Pred::Not(inner) if **inner == Pred::ControlledBy(Ref::You) => {
            "you don't control".to_owned()
        }
        Pred::HasKeyword(keyword) => format!("with {}", keyword_text(keyword)?),
        Pred::HasCounter(kind) if form.plural() => {
            format!("with {} counters on them", counters::kind_text(kind)?)
        }
        Pred::HasCounter(kind) => {
            let words = counters::trigger_kind_text(Some(kind))?;
            format!("with {words} on it")
        }
        Pred::Not(inner) => match &**inner {
            Pred::HasKeyword(keyword) => format!("without {}", keyword_text(keyword)?),
            Pred::CastFrom(ZoneKind::Hand) => "from anywhere other than your hand".to_owned(),
            Pred::EnteredThisTurn => "that didn't enter this turn".to_owned(),
            _ => return None,
        },
        Pred::Compare {
            stat: StatKind::ManaValue,
            op: Cmp::Le,
            to: Amount::Triggering,
        } => "with mana value less than or equal to that damage".to_owned(),
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

/// "a +1/+1 counter on it", "a level counter on it": the counter an object holds, after "with".
fn counter_on_it(r: &mut Reader<'_>) -> Option<CounterKind> {
    (r.word("a") || r.word("an")).then_some(())?;
    let kind = counters::trigger_kind(r)?;
    r.words("on it").then_some(kind)
}

/// A subtype as it is written ("Clue", "Soldier"), of the rules' list or of the card's own type line; any other
/// capitalized word is not read.
pub fn subtype(r: &mut Reader<'_>) -> Option<String> {
    r.attempt(|r| {
        let name = r.name()?;
        if let Some(whole) = r.attempt(|r| {
            let second = r.name()?;
            super::subtypes::is_two_word_land(name, second).then(|| format!("{name} {second}"))
        }) {
            return Some(whole);
        }
        r.is_subtype(name).then(|| name.to_owned())
    })
}

/// A group of one subtype as it is written in the plural ("Heroes", "Elves", "Spies"): the subtype in the singular.
pub(super) fn plural_subtype(r: &mut Reader<'_>) -> Option<String> {
    r.attempt(|r| {
        let name = group_name(r)?;
        singular_subtype(&name, |one| r.is_subtype(one))
    })
}

/// The capitalized word of a group: a name inside a sentence, or the word that opens it.
fn group_name(r: &mut Reader<'_>) -> Option<String> {
    match r.name() {
        Some(name) => Some(name.to_owned()),
        None => r.opening_name(),
    }
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
    /// "token" or "tokens", after the card types before it: "creature tokens", "target creature token".
    Token,
    /// "commander": a player's commander, with no card type before it.
    Commander,
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
    if let Some(word @ ("commander" | "commanders")) = r.peek_word()
        && (word == "commanders") == plural
    {
        r.skip();
        return Some((Vec::new(), Noun::Commander));
    }
    if r.word_at(0) == Some("permanent")
        && matches!(r.word_at(1), Some("spell" | "spells"))
        && (r.word_at(1) == Some("spells")) == plural
    {
        r.skip();
        r.skip();
        return Some((PERMANENT_TYPES.to_vec(), Noun::Spell));
    }
    // "permanent card": a card with any of the permanent types, in a zone the phrase names.
    if r.word_at(0) == Some("permanent")
        && matches!(r.word_at(1), Some("card" | "cards"))
        && (r.word_at(1) == Some("cards")) == plural
    {
        r.skip();
        r.skip();
        return Some((PERMANENT_TYPES.to_vec(), Noun::Card));
    }
    let mut types = Vec::new();
    let mut forms = Vec::new();
    while let Some((card_type, is_plural)) = r.peek_word().and_then(type_noun) {
        r.skip();
        types.push(card_type);
        forms.push(is_plural);
        if !type_separator(r, form) {
            break;
        }
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
        Some(word @ ("token" | "tokens"))
            if (word == "tokens") == plural && forms.iter().all(|is_plural| !is_plural) =>
        {
            r.skip();
            Some((types, Noun::Token))
        }
        _ if !types.is_empty() && forms.iter().all(|is_plural| *is_plural == plural) => {
            Some((types, Noun::Plain))
        }
        _ => None,
    }
}

/// Reads the word that joins two card types: the form's connector, which is the three tokens "and/or" for the targets of a
/// group ("artifacts and/or enchantments").
fn connect(r: &mut Reader<'_>, form: Form) -> bool {
    let and_or = |r: &mut Reader<'_>| {
        r.attempt(|r| {
            r.word("and").then_some(())?;
            r.punct('/').then_some(())?;
            r.word("or").then_some(())
        })
        .is_some()
    };
    if form == Form::Targets {
        return and_or(r);
    }
    // A count says it either way: "for each artifact or enchantment you control", "... artifact and/or enchantment ...".
    r.word(form.connector()) || (form == Form::Count && and_or(r))
}

/// The words between two card types of a phrase: the form's connector ("artifact or creature"), or a comma with the
/// connector after it ("artifact, creature, or planeswalker"); reads them when a card type follows.
fn type_separator(r: &mut Reader<'_>, form: Form) -> bool {
    r.attempt(|r| {
        if r.punct(',') {
            connect(r, form);
        } else {
            connect(r, form).then_some(())?;
        }
        r.peek_word().and_then(type_noun).map(|_| ())
    })
    .is_some()
}

/// "in target opponent's hand", "in target player's hand": the hand of the one player the sentence targets.
fn target_hand(r: &mut Reader<'_>) -> Option<Ref> {
    r.need("in target")?;
    let what = if r.word("player's") {
        Targetable::Player
    } else {
        r.need("opponent's")?;
        Targetable::Opponent
    };
    r.need("hand")?;
    r.targets.is_empty().then_some(())?;
    r.targets.push(TargetSpec {
        what,
        bounds: Bounds::default(),
        divide: None,
    });
    Some(Ref::Target(0))
}

/// "from your graveyard", "from your hand": the zone and its owner.
fn zone_suffix(r: &mut Reader<'_>, form: Form) -> Option<Vec<Pred>> {
    let (hand, graveyard) = if form == Form::Count {
        ("in your hand", "in your graveyard")
    } else {
        ("from your hand", "from your graveyard")
    };
    if form == Form::Count
        && let Some(owner) = r.attempt(target_hand)
    {
        return Some(vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(owner)]);
    }
    // A group of cards says where they are as the count does: "nonland cards in your hand".
    let in_zone = form == Form::All;
    if r.words(hand) || (in_zone && r.words("in your hand")) {
        return Some(vec![Pred::In(ZoneKind::Hand), Pred::OwnedBy(Ref::You)]);
    }
    if r.words(graveyard) || (in_zone && r.words("in your graveyard")) {
        return Some(vec![Pred::In(ZoneKind::Graveyard), Pred::OwnedBy(Ref::You)]);
    }
    if form == Form::Count {
        return None;
    }
    // The graveyard of any player, or of the one the phrase names.
    OTHER_GRAVEYARDS
        .iter()
        .find(|(words, _)| r.words(words))
        .map(|(_, owner)| {
            let mut preds = vec![Pred::In(ZoneKind::Graveyard)];
            preds.extend(owner.clone().map(Pred::OwnedBy));
            preds
        })
}

/// The graveyards a card is taken from besides one's own: "from a graveyard", "from an opponent's graveyard", "from
/// defending player's graveyard".
const OTHER_GRAVEYARDS: [(&str, Option<Ref>); 3] = [
    ("from a graveyard", None),
    ("from an opponent's graveyard", Some(Ref::Opponents)),
    ("from defending player's graveyard", Some(Ref::Defending)),
];

/// The subtypes before "spell" in the singular: "Merfolk spell", "Vampire creature spell", "Faerie or Wizard permanent
/// spell" (several are an `AnyOf`).
fn spell_subtypes(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        let one = |r: &mut Reader<'_>| {
            r.attempt(|r| group_name(r).filter(|name| r.is_subtype(name)))
                .map(|name| Pred::Subtype(Subtype(name)))
        };
        let mut found = vec![one(r)?];
        // "Aura or Equipment", "Aura, Equipment, or Vehicle".
        while r.punct(',') | r.word("or") {
            r.word("or");
            found.push(one(r)?);
        }
        r.lookahead(|r| {
            while r.peek_word() == Some("permanent")
                || r.peek_word()
                    .and_then(type_noun)
                    .is_some_and(|(_, plural)| !plural)
            {
                r.skip();
            }
            (r.word("spell") || r.word("card")).then_some(())
        })?;
        Some(if found.len() == 1 {
            found.remove(0)
        } else {
            Pred::AnyOf(found)
        })
    })
}

/// A subtype in front of a card type in the singular, or of "card": "Cleric creature", "Equipment artifact", "Giant card"; a
/// card type in front of another one: "artifact creature".
fn singular_qualifier(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        if let Some((card_type, false)) = r.peek_word().and_then(type_noun)
            && r.word_at(1)
                .and_then(type_noun)
                .is_some_and(|(_, plural)| !plural)
        {
            r.skip();
            return Some(Pred::Type(card_type));
        }
        let name = group_name(r).filter(|name| r.is_subtype(name))?;
        let typed = r
            .peek_word()
            .and_then(type_noun)
            .is_some_and(|(_, plural)| !plural)
            || r.peek_word() == Some("card");
        typed.then_some(Pred::Subtype(Subtype(name)))
    })
}

/// A subtype or a card type in the singular that qualifies the plural noun after it: "Sliver creatures", "artifact
/// creatures", "creature tokens" (the card types of a token are the noun).
fn qualifier(r: &mut Reader<'_>) -> Option<Pred> {
    r.attempt(|r| {
        let plural_noun = |word: Option<&str>| {
            word.is_some_and(|word| {
                matches!(word, "tokens" | "spells" | "cards")
                    || type_noun(word).is_some_and(|(_, plural)| plural)
            })
        };
        let name = r.attempt(|r| group_name(r).filter(|name| r.is_subtype(name)));
        if let Some(name) = name {
            return plural_noun(r.peek_word()).then_some(Pred::Subtype(Subtype(name)));
        }
        let (card_type, plural) = r.peek_word().and_then(type_noun)?;
        r.skip();
        (!plural && plural_noun(r.peek_word())).then_some(Pred::Type(card_type))
    })
}

/// "nonland permanent card": the permanent types but land, with the adjective "nonland" taken into them.
fn nonland_permanent_card(noun: &Noun, types: &mut Vec<CardType>, adjectives: &mut Vec<Pred>) {
    if *noun == Noun::Card
        && is_permanent(types)
        && let Some(at) = adjectives
            .iter()
            .position(|pred| *pred == not(Pred::Type(CardType::Land)))
    {
        adjectives.remove(at);
        types.retain(|card_type| *card_type != CardType::Land);
    }
}

/// A phrase naming a set of objects ("nonblack creature", "creatures you control", "noncreature spell").
pub fn phrase(r: &mut Reader<'_>, form: Form) -> Option<Selector> {
    phrase_in(r, form, false)
}

/// "non-Elf", "non-Vampire": anything but a creature type (the word is lowercase in the tokens).
fn non_subtype(r: &Reader<'_>, word: &str) -> Option<Pred> {
    let name = word.strip_prefix("non-")?;
    let mut chars = name.chars();
    let name = format!("{}{}", chars.next()?.to_uppercase(), chars.as_str());
    r.is_subtype(&name)
        .then(|| not(Pred::Subtype(Subtype(name))))
}

/// A phrase like `phrase`, except that "cards" may stand without the zone they are in, which the caller reads next
/// ("creature cards" before "in your graveyard").
pub(super) fn phrase_in(r: &mut Reader<'_>, form: Form, unplaced: bool) -> Option<Selector> {
    r.attempt(|r| {
        // "your commander": the commander its owner designated, whoever controls it.
        let own_commander = r
            .attempt(|r| {
                (r.word("your") && matches!(r.peek_word(), Some("commander" | "commanders")))
                    .then_some(())
            })
            .is_some();
        let mut adjectives = Vec::new();
        while let Some(pred) = next_adjective(r, form) {
            r.skip();
            adjectives.push(pred);
            // "attacking or blocking creature": either state.
            if adjectives.last() == Some(&Pred::Attacking) && r.words("or blocking") {
                adjectives.pop();
                adjectives.push(attacking_or_blocking());
            }
            // "nonartifact, nonblack creature": a comma joins two negations.
            let negation = |word: Option<&str>| {
                word.and_then(|word| adjective(word, form).or_else(|| non_subtype(r, word)))
                    .is_some_and(|pred| matches!(pred, Pred::Not(_)))
            };
            if matches!(adjectives.last(), Some(Pred::Not(_)))
                && r.at_punct(',')
                && negation(r.word_at(1))
            {
                r.skip();
            }
        }
        // The files put "other" after the adjectives it is printed before ("other legendary creatures").
        adjectives.sort_by_key(|pred| *pred == Pred::Other);
        let mut qualifiers = Vec::new();
        if form.plural() {
            while let Some(pred) = qualifier(r) {
                qualifiers.push(pred);
            }
        } else {
            qualifiers.extend(spell_subtypes(r).or_else(|| singular_qualifier(r)));
        }
        // The files list the card types first, then the subtype ("Creature, Goblin").
        let (kinds, subtypes): (Vec<Pred>, Vec<Pred>) = qualifiers
            .into_iter()
            .partition(|pred| matches!(pred, Pred::Type(_)));
        let (mut types, noun) = head(r, form)?;
        nonland_permanent_card(&noun, &mut types, &mut adjectives);
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
                preds.extend(kinds);
                preds.extend(type_pred);
                preds.extend(subtypes);
            }
            Noun::Subtype(name) => {
                preds.push(Pred::Subtype(Subtype(name.clone())));
                preds.extend(adjectives);
            }
            Noun::Plain | Noun::Card => {
                preds.extend(kinds);
                preds.extend(type_pred);
                preds.extend(subtypes);
                preds.extend(adjectives);
            }
            Noun::Token => {
                preds.extend(kinds);
                preds.extend(type_pred);
                preds.extend(subtypes);
                preds.push(Pred::Token);
                preds.extend(adjectives);
            }
            Noun::Commander => {
                preds.push(Pred::Commander);
                preds.extend(own_commander.then_some(Pred::OwnedBy(Ref::You)));
                preds.extend(adjectives);
            }
        }
        let adjectives_end = preds.len();
        loop {
            if let Some(zone) = zone_suffix(r, form) {
                preds.extend(zone);
            } else if let Some(pred) = suffix(r, form) {
                preds.push(pred);
            } else {
                break;
            }
        }
        // "artifact, enchantment, or creature with flying": the qualifier belongs to the last type only.
        let qualified = preds[adjectives_end..]
            .iter()
            .any(|pred| matches!(pred, Pred::HasKeyword(_) | Pred::Compare { .. }));
        // A permanent card is the one noun for its types: "permanent card with mana value 3 or less".
        let permanent_card =
            noun == Noun::Card && (is_permanent(&types) || is_nonland_permanent(&types));
        if types.len() > 2 && qualified && !permanent_card {
            return None;
        }
        let placed = preds.iter().any(|pred| matches!(pred, Pred::In(_)));
        (noun != Noun::Card || placed || unplaced).then_some(Selector(preds))
    })
}

/// Card types joined by the connector: "artifact or creature", "artifact, creature, or planeswalker".
fn list_text(names: &[String], connector: &str) -> String {
    match names {
        [rest @ .., last] if rest.len() > 1 => format!("{}, {connector} {last}", rest.join(", ")),
        _ => names.join(&format!(" {connector} ")),
    }
}

/// The words for the card types of a phrase: the kinds before them, "permanent" or "nonland permanent", "commander" as
/// an adjective of the types, and the types themselves.
fn type_phrase(
    kinds: &[CardType],
    (permanent, nonland_permanent): (bool, bool),
    commander: bool,
    names: &[String],
    form: Form,
) -> Vec<String> {
    let mut words: Vec<String> = kinds
        .iter()
        .map(|card_type| type_name(*card_type).to_owned())
        .collect();
    words.extend(permanent_words(permanent, nonland_permanent));
    if commander && !names.is_empty() {
        words.push("commander".to_owned());
    }
    if !names.is_empty() {
        words.push(list_text(names, form.connector()));
    }
    words
}

/// The adjectives of a phrase, with a comma after each negation that another follows: "nonartifact, nonblack".
fn comma_negations(adjectives: Vec<String>) -> Vec<String> {
    let mut words: Vec<String> = Vec::with_capacity(adjectives.len());
    for word in adjectives {
        if word.starts_with("non")
            && let Some(last) = words.last_mut().filter(|last| last.starts_with("non"))
        {
            last.push(',');
        }
        words.push(word);
    }
    words
}

/// The names of the card types of a phrase, in the plural when the phrase names them alone.
fn type_words(types: &[CardType], plural: bool) -> Vec<String> {
    types
        .iter()
        .map(|card_type| {
            let name = type_name(*card_type);
            if plural {
                plural_of(name)
            } else {
                name.to_owned()
            }
        })
        .collect()
}

/// The noun that ends a phrase: "tokens", "spell", "card", "commander", or "permanent" when the phrase names no type
/// or subtype; `which` says in that order which of them the phrase is, and the first one counts.
fn noun_word(which: [bool; 5], plural: bool) -> Option<String> {
    let nouns = ["token", "spell", "card", "commander", "permanent"];
    let at = which.iter().position(|chosen| *chosen)?;
    let noun = nouns[at];
    Some(if plural {
        format!("{noun}s")
    } else {
        noun.to_owned()
    })
}

/// The subtype names of a subtype or of an `AnyOf` of several.
fn subtype_list(pred: &Pred) -> Option<Vec<&str>> {
    let list = match pred {
        Pred::Subtype(Subtype(name)) => return Some(vec![name.as_str()]),
        Pred::AnyOf(list) if list.len() > 1 && *pred != outlaw() => list,
        _ => return None,
    };
    list.iter()
        .map(|one| match one {
            Pred::Subtype(Subtype(name)) => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// The names of an `AnyOf` of subtypes and card types together, in order: subtypes as they are written and card types in
/// lowercase ("Plant", "Treefolk", "land").
fn mixed_list(pred: &Pred) -> Option<Vec<String>> {
    let Pred::AnyOf(list) = pred else {
        return None;
    };
    let names: Vec<String> = list
        .iter()
        .map(|one| match one {
            Pred::Subtype(Subtype(name)) => Some(name.clone()),
            Pred::Type(card_type) => Some(type_name(*card_type).to_owned()),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let subtypes = list
        .iter()
        .filter(|one| matches!(one, Pred::Subtype(_)))
        .count();
    (list.len() > 1 && subtypes > 0 && subtypes < list.len()).then_some(names)
}

/// The words of a phrase that names the permanent types: "permanent" (a spell) or "nonland permanent" (a card in a zone).
fn permanent_words(permanent: bool, nonland: bool) -> Vec<String> {
    let mut words = Vec::new();
    if nonland {
        words.push("nonland".to_owned());
    }
    if permanent || nonland {
        words.push("permanent".to_owned());
    }
    words
}

/// Whether the card types are exactly those of a permanent but a land.
fn is_nonland_permanent(types: &[CardType]) -> bool {
    types.len() == PERMANENT_TYPES.len() - 1
        && PERMANENT_TYPES
            .iter()
            .filter(|found| **found != CardType::Land)
            .all(|found| types.contains(found))
}

/// Whether the card types are exactly those of a permanent.
fn is_permanent(types: &[CardType]) -> bool {
    types.len() == PERMANENT_TYPES.len()
        && PERMANENT_TYPES.iter().all(|found| types.contains(found))
}

/// The subtypes of a phrase: one name, or a list joined by "or".
fn subtype_words(subtypes: &[&str], plural: bool, joiner: &str) -> String {
    let words: Vec<String> = subtypes
        .iter()
        .map(|name| {
            if plural {
                plural_subtype_word(name)
            } else {
                (*name).to_owned()
            }
        })
        .collect();
    list_text(&words, joiner)
}

/// Whether the predicates name the commander its owner designated: "your commander".
fn names_own_commander(preds: &[Pred]) -> bool {
    preds.contains(&Pred::Commander) && preds.contains(&Pred::OwnedBy(Ref::You))
}

/// A selector as a phrase, the way `phrase` reads it.
pub fn phrase_text(selector: &Selector, form: Form) -> Option<String> {
    let (spell, preds) = match selector.0.split_first() {
        Some((Pred::Spell, rest)) => (true, rest),
        _ => (false, selector.0.as_slice()),
    };
    let (mut adjectives, mut types, mut suffixes) = (Vec::new(), Vec::new(), Vec::new());
    let (mut subtypes, mut card, mut token) = (Vec::<&str>::new(), false, false);
    // The zone of a card comes last ("permanent card with mana value 3 or less from your graveyard").
    let mut zone: Option<String> = None;
    let mut commander = false;
    let own_commander = names_own_commander(preds);
    let mut kinds: Vec<CardType> = Vec::new();
    // Subtypes and card types in one list: "Plant, Treefolk, or land card".
    let mut mixed: Option<Vec<String>> = None;
    let mut rest = preds.iter().peekable();
    while let Some(pred) = rest.next() {
        if let Some(place) = zone_words(pred, &mut rest, form) {
            card = true;
            if form == Form::Count {
                suffixes.push(place);
            } else {
                zone = Some(place);
            }
        } else if let Some(listed) = subtype_list(pred) {
            if !subtypes.is_empty() {
                return None;
            }
            subtypes = listed;
        } else if let Some(listed) =
            mixed_list(pred).filter(|_| mixed.is_none() && subtypes.is_empty() && !form.plural())
        {
            mixed = Some(listed);
        } else if let Some(word) = adjective_text(pred, form) {
            if *pred == Pred::Other {
                adjectives.insert(0, word);
            } else {
                adjectives.push(word);
            }
        } else if *pred == Pred::Token {
            if token {
                return None;
            }
            token = true;
        } else if *pred == Pred::Commander {
            if commander {
                return None;
            }
            commander = true;
        } else if own_commander && *pred == Pred::OwnedBy(Ref::You) {
            // Said as "your" before the noun.
        } else if let Some(found) = type_preds(pred) {
            // A second card type before the noun qualifies it: "artifact creatures".
            if !types.is_empty() {
                qualifies(&types, &found, form).then_some(())?;
                kinds.append(&mut types);
            }
            types = found;
        } else {
            suffixes.push(suffix_text(pred, form)?);
        }
    }
    let plural = form.plural();
    // A subtype is the noun by itself, or qualifies a plural noun ("Sliver creatures"); a spell is not a card in a zone.
    let qualified = ((plural || spell) && !card && (spell || !types.is_empty() || token))
        || (!plural && !spell && !card && !token && !types.is_empty());
    // Several subtypes are only for a spell ("Faerie or Wizard permanent spell").
    if (!subtypes.is_empty() && !types.is_empty() && !qualified && !card)
        || (subtypes.len() > 1 && !spell && !card && (qualified || !types.is_empty()))
        || (spell && card)
        || (token && (spell || card))
        || (commander && (spell || card || token || !subtypes.is_empty()))
        || (!kinds.is_empty() && !qualified)
    {
        return None;
    }
    let permanent = (spell || card) && is_permanent(&types);
    // The card types of a permanent but land, for a card in a zone: "nonland permanent card".
    let nonland_permanent = card && !spell && is_nonland_permanent(&types);
    let names = if permanent || nonland_permanent {
        Vec::new()
    } else {
        type_words(&types, plural && !spell && !card && !token)
    };
    let mut words = comma_negations(adjectives);
    words.splice(0..0, own_commander.then(|| "your".to_owned()));
    if !subtypes.is_empty() {
        let nouns = plural && !qualified && !card;
        let joiner = if nouns { form.connector() } else { "or" };
        words.push(subtype_words(&subtypes, nouns, joiner));
    }
    words.extend(mixed.iter().map(|listed| list_text(listed, "or")));
    words.extend(type_phrase(
        &kinds,
        (permanent, nonland_permanent),
        commander,
        &names,
        form,
    ));
    // "Commander creatures you control": the commander is an adjective of the card types.
    let commander_noun = commander && names.is_empty();
    let bare = names.is_empty() && subtypes.is_empty() && mixed.is_none();
    words.extend(noun_word(
        [token, spell, card, commander_noun, bare],
        plural,
    ));
    words.extend(without_list(suffixes));
    words.extend(zone);
    Some(words.join(" "))
}

/// Whether the card type `found` qualifies the noun of the type before it: "artifact creatures", and in the singular
/// only "artifact creature".
fn qualifies(types: &[CardType], found: &[CardType], form: Form) -> bool {
    let creature = found.contains(&CardType::Creature);
    (form.plural() || creature) && types.len() == 1 && found.len() == 1
}

/// Several "without ..." suffixes are one: "creatures without flying or islandwalk".
fn without_list(suffixes: Vec<String>) -> Vec<String> {
    let lacks: Vec<&str> = suffixes
        .iter()
        .filter_map(|suffix| suffix.strip_prefix("without "))
        .collect();
    if lacks.len() < 2 {
        return suffixes;
    }
    let joined = format!("without {}", join_or(&lacks));
    let mut merged = Vec::with_capacity(suffixes.len());
    for suffix in &suffixes {
        if !suffix.starts_with("without ") {
            merged.push(suffix.clone());
        } else if !merged.contains(&joined) {
            merged.push(joined.clone());
        }
    }
    merged
}

/// "a or b", "a, b, or c".
fn join_or(items: &[&str]) -> String {
    match items {
        [first, second] => format!("{first} or {second}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
        [] => String::new(),
    }
}

/// The predicates after `pred` that name the zone of a card, said as the card words it: "from your graveyard", "in your
/// hand" (counting), "from an opponent's graveyard", "from a graveyard".
fn zone_words(pred: &Pred, rest: &mut Peekable<Iter<'_, Pred>>, form: Form) -> Option<String> {
    let Pred::In(zone @ (ZoneKind::Graveyard | ZoneKind::Hand)) = pred else {
        return None;
    };
    if rest
        .next_if(|next| **next == Pred::OwnedBy(Ref::You))
        .is_some()
    {
        let preposition = if form == Form::Count { "in" } else { "from" };
        let place = if *zone == ZoneKind::Hand {
            "hand"
        } else {
            "graveyard"
        };
        return Some(format!("{preposition} your {place}"));
    }
    // The hand of the player an event is about is counted as "their hand".
    if form == Form::Count
        && *zone == ZoneKind::Hand
        && rest
            .next_if(|next| **next == Pred::OwnedBy(Ref::Triggering))
            .is_some()
    {
        return Some("in their hand".to_owned());
    }
    // The hand of the one player the sentence targets: the writer of the effect says "player's" for a target player.
    if form == Form::Count
        && *zone == ZoneKind::Hand
        && rest
            .next_if(|next| **next == Pred::OwnedBy(Ref::Target(0)))
            .is_some()
    {
        return Some("in target opponent's hand".to_owned());
    }
    if *zone != ZoneKind::Graveyard || form == Form::Count {
        return None;
    }
    for (words, owner) in OTHER_GRAVEYARDS {
        if let Some(owner) = owner
            && rest
                .next_if(|next| **next == Pred::OwnedBy(owner.clone()))
                .is_some()
        {
            return Some(words.to_owned());
        }
    }
    Some(OTHER_GRAVEYARDS[0].0.to_owned())
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

/// "another target artifact card from your graveyard": the phrase with "other" after its adjectives, before the zone and
/// who controls it, where the files put it.
fn another(mut selector: Selector) -> Selector {
    let zoned = |pred: &Pred| match pred {
        Pred::Not(inner) => matches!(**inner, Pred::ControlledBy(_)),
        other => matches!(
            other,
            Pred::In(_)
                | Pred::OwnedBy(_)
                | Pred::ControlledBy(_)
                | Pred::HasKeyword(_)
                | Pred::Compare { .. }
        ),
    };
    let at = selector
        .0
        .iter()
        .position(zoned)
        .unwrap_or(selector.0.len());
    selector.0.insert(at, Pred::Other);
    selector
}

fn spec(r: &mut Reader<'_>) -> Option<TargetSpec> {
    if r.words("any target") {
        return Some(plain(Targetable::AnyTarget));
    }
    if r.words("any number of target") {
        let what = Targetable::Object(phrase(r, Form::Targets)?);
        let bounds = Bounds::up_to(None);
        return Some(TargetSpec {
            what,
            bounds,
            divide: None,
        });
    }
    if r.words("up to") {
        let count = number_word(r.take_word()?)?;
        // "up to one other target Vehicle you control".
        let other = count == 1 && r.word("other");
        r.need("target")?;
        let form = if count == 1 {
            Form::Target
        } else {
            Form::Targets
        };
        let selector = phrase(r, form)?;
        let what = Targetable::Object(if other { another(selector) } else { selector });
        let max = Some(Amount::Fixed(i32::try_from(count).ok()?));
        return Some(TargetSpec {
            what,
            bounds: Bounds::up_to(max),
            divide: None,
        });
    }
    // "X target creature cards": exactly the X announced.
    if r.attempt(|r| (r.x() && r.word("target")).then_some(()))
        .is_some()
    {
        let what = Targetable::Object(phrase(r, Form::Targets)?);
        return Some(TargetSpec {
            what,
            bounds: Bounds::exactly(&Amount::X),
            divide: None,
        });
    }
    // "two target creatures", "six target nonland permanents": exactly that many.
    if let Some(count) = r.attempt(|r| {
        let count = number_word(r.take_word()?).filter(|count| *count > 1)?;
        r.need("target")?;
        Some(count)
    }) {
        let what = Targetable::Object(phrase(r, Form::Targets)?);
        let max = Some(Amount::Fixed(i32::try_from(count).ok()?));
        return Some(TargetSpec {
            what,
            bounds: Bounds {
                min: Amount::Fixed(i32::try_from(count).ok()?),
                max,
            },
            divide: None,
        });
    }
    let other = r.word("another");
    r.need("target")?;
    let what = if other {
        Targetable::Object(another(phrase(r, Form::Target)?))
    } else if r.word("player") {
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

/// "other than the first target": what "another target" says after a first target (CR 115.3), which the engine keeps
/// apart when the targets are chosen.
pub fn other_than_first() -> Pred {
    Pred::Not(Box::new(Pred::Is(Ref::Target(0))))
}

/// A "target ..." phrase after the first target of the clause: `Target(1)`. "Another" and "other" in it mean other than
/// the first target, not other than the card.
pub fn second_target(r: &mut Reader<'_>) -> Option<Ref> {
    (r.targets.len() == 1).then_some(())?;
    let mut spec = r.attempt(spec)?;
    if let Targetable::Object(selector) = &mut spec.what {
        for pred in &mut selector.0 {
            if *pred == Pred::Other {
                *pred = other_than_first();
            }
        }
    }
    r.targets.push(spec);
    Some(Ref::Target(1))
}

/// One target of several kinds of objects: "target artifact, enchantment, or creature with power 4 or greater".
fn either_objects(kinds: &[Targetable]) -> Option<String> {
    let phrases: Vec<String> = kinds
        .iter()
        .map(|kind| match kind {
            Targetable::Object(selector) => phrase_text(selector, Form::Target),
            _ => None,
        })
        .collect::<Option<_>>()?;
    let phrases: Vec<&str> = phrases.iter().map(String::as_str).collect();
    Some(format!("target {}", join_or(&phrases)))
}

/// A target as the card words it.
pub fn spec_text(spec: &TargetSpec) -> Option<String> {
    if spec.divide.is_some() {
        return None;
    }
    // "Another target creature" after a first target: the same word as "other than the card".
    let mut own;
    let mut spec = spec;
    if let Targetable::Object(selector) = &spec.what
        && selector.0.contains(&other_than_first())
    {
        own = spec.clone();
        if let Targetable::Object(chosen) = &mut own.what {
            for pred in &mut chosen.0 {
                if *pred == other_than_first() {
                    *pred = Pred::Other;
                }
            }
        }
        spec = &own;
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
                // Objects of different kinds: "target artifact, enchantment, or creature with power 4 or greater".
                [_, _, ..] => either_objects(kinds)?,
                _ => return None,
            },
            Targetable::Object(selector) => {
                let phrase = phrase_text(selector, Form::Target).or_else(|| {
                    super::c8_kwq_abilities::c8_kwq_ability_phrase(selector).map(str::to_owned)
                })?;
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
    if spec.bounds == Bounds::exactly(&Amount::X) {
        return Some(format!(
            "X target {}",
            phrase_text(selector, Form::Targets)?
        ));
    }
    if let (Some(count @ 2..), Some(Amount::Fixed(max))) =
        (spec.bounds.fixed_min(), &spec.bounds.max)
        && i64::from(count) == i64::from(*max)
    {
        return Some(format!(
            "{} target {}",
            count_word(count)?,
            phrase_text(selector, Form::Targets)?
        ));
    }
    if spec.bounds.fixed_min() != Some(0) {
        return None;
    }
    match &spec.bounds.max {
        None => Some(format!(
            "any number of target {}",
            phrase_text(selector, Form::Targets)?
        )),
        Some(Amount::Fixed(1)) => {
            let phrase = phrase_text(selector, Form::Target)?;
            Some(match phrase.strip_prefix("other ") {
                Some(rest) => format!("up to one other target {rest}"),
                None => format!("up to one target {phrase}"),
            })
        }
        Some(Amount::Fixed(count)) => Some(format!(
            "up to {} target {}",
            count_word(u32::try_from(*count).ok()?)?,
            phrase_text(selector, Form::Targets)?
        )),
        Some(Amount::X) => Some(format!(
            "up to X target {}",
            phrase_text(selector, Form::Targets)?
        )),
        Some(_) => None,
    }
}
