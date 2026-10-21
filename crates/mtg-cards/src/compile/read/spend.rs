//! "Spend this mana only to cast creature spells.": what the mana a sentence adds may be spent on, one use per sentence.

use mtg_core::card::{CardType, Supertype};

use super::selectors::{card_type, type_name};
use super::tokens::Reader;
use crate::spend::{SpendFilter, SpendOn};

const LEAD: &str = "spend this mana only";

/// The words that stand for a spell of a kind: a card type or two ("instant or sorcery"), or one adjective.
fn kind(r: &mut Reader<'_>) -> Option<SpendFilter> {
    let first = r.peek_word()?;
    let adjective = match first {
        "noncreature" => Some(SpendFilter {
            without: vec![CardType::Creature],
            ..SpendFilter::default()
        }),
        "multicolored" => Some(SpendFilter {
            multicolored: true,
            ..SpendFilter::default()
        }),
        "legendary" => Some(SpendFilter {
            supertypes: vec![Supertype::Legendary],
            ..SpendFilter::default()
        }),
        _ => None,
    };
    if adjective.is_some() {
        r.skip();
        return adjective;
    }
    let mut types = vec![card_type(first)?];
    r.skip();
    while let Some(next) = r.attempt(|r| {
        r.word("or").then_some(())?;
        let found = card_type(r.peek_word()?)?;
        r.skip();
        Some(found)
    }) {
        types.push(next);
    }
    Some(SpendFilter {
        types,
        ..SpendFilter::default()
    })
}

/// "creature spells", "a creature spell", "spells" or "a spell".
fn spells(r: &mut Reader<'_>) -> Option<SpendFilter> {
    r.attempt(|r| {
        let filter = r.attempt(kind).unwrap_or_default();
        r.word("spells").then_some(filter)
    })
    .or_else(|| {
        r.attempt(|r| {
            (r.word("a") || r.word("an")).then_some(())?;
            let filter = r.attempt(kind).unwrap_or_default();
            r.word("spell").then_some(filter)
        })
    })
}

/// The sentence "Spend this mana only to ..." read after the period that ends the sentence adding the mana.
pub(super) fn only(r: &mut Reader<'_>) -> Option<Vec<SpendOn>> {
    r.attempt(|r| {
        r.punct('.').then_some(())?;
        r.need(LEAD)?;
        if r.words("on costs that contain") {
            return (r.symbol()? == "X").then(|| vec![SpendOn::CostWithX]);
        }
        r.need("to")?;
        if r.words("activate abilities") {
            return Some(vec![SpendOn::abilities()]);
        }
        r.need("cast")?;
        Some(vec![SpendOn::Spell(spells(r)?)])
    })
}

/// The kind of spell as the plural noun phrase reads, up to "spells": `None` when no wording of the kind exists.
fn kind_text(filter: &SpendFilter) -> Option<String> {
    let SpendFilter {
        types,
        without,
        supertypes,
        subtypes,
        multicolored,
    } = filter;
    if !subtypes.is_empty() {
        return None;
    }
    match (
        types.as_slice(),
        without.as_slice(),
        supertypes.as_slice(),
        multicolored,
    ) {
        ([], [], [], false) => Some(String::new()),
        ([], [CardType::Creature], [], false) => Some("noncreature ".into()),
        ([], [], [Supertype::Legendary], false) => Some("legendary ".into()),
        ([], [], [], true) => Some("multicolored ".into()),
        (types, [], [], false) if !types.is_empty() => {
            let names: Vec<&str> = types.iter().map(|t| type_name(*t)).collect();
            Some(format!("{} ", names.join(" or ")))
        }
        _ => None,
    }
}

/// The sentence the uses are written as, with its capital and no period; `None` for uses the readers do not write.
pub(super) fn text(only: &[SpendOn]) -> Option<String> {
    Some(match only {
        [SpendOn::CostWithX] => "Spend this mana only on costs that contain {X}".to_owned(),
        [SpendOn::Ability(filter)] if *filter == SpendFilter::default() => {
            "Spend this mana only to activate abilities".to_owned()
        }
        [SpendOn::Spell(filter)] => {
            format!("Spend this mana only to cast {}spells", kind_text(filter)?)
        }
        _ => return None,
    })
}

/// The wording with one spell in place of spells ("cast creature spells" is "cast a creature spell").
pub(super) fn singular(text: &str) -> Option<String> {
    let (head, nouns) = text.split_once("only to cast ")?;
    let (nouns, tail) = nouns.split_once("spells")?;
    let article = if nouns.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    };
    Some(format!("{head}only to cast {article} {nouns}spell{tail}"))
}
