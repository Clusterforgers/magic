//! "Spend this mana only to cast creature spells.": what the mana a sentence adds may be spent on, one use per sentence.

use mtg_core::card::{CardType, Subtype, Supertype};

use super::selectors::{card_type, plural_subtype, subtype, type_name};
use super::subtypes::plural_subtype as plural_name;
use super::tokens::Reader;
use crate::spend::{SpendFilter, SpendOn};

const LEAD: &str = "spend this mana only";

/// The words that name the creature type the source chose.
const CHOSEN_TYPE: &str = " of the chosen type";

/// The words that stand for a spell of a kind: a card type or two ("instant or sorcery"), or one adjective.
fn kind(r: &mut Reader<'_>) -> Option<SpendFilter> {
    // "Dragon spells": one creature type or other subtype, a capitalized word.
    if let Some(name) = subtype(r) {
        return Some(SpendFilter {
            subtypes: vec![Subtype(name)],
            ..SpendFilter::default()
        });
    }
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

/// "creature spells", "a creature spell", "spells" or "a spell", then "of the chosen type" for the creature type the
/// source chose as it entered.
fn spells(r: &mut Reader<'_>) -> Option<SpendFilter> {
    let mut filter = r
        .attempt(|r| {
            let filter = r.attempt(kind).unwrap_or_default();
            r.word("spells").then_some(filter)
        })
        .or_else(|| {
            r.attempt(|r| {
                (r.word("a") || r.word("an")).then_some(())?;
                let filter = r.attempt(kind).unwrap_or_default();
                r.word("spell").then_some(filter)
            })
        })?;
    filter.chosen_type = r.words("of the chosen type");
    Some(filter)
}

/// "activate an ability of a creature source of the chosen type": a source of the kind the chosen type names.
fn ability_source(r: &mut Reader<'_>) -> Option<SpendFilter> {
    r.attempt(|r| {
        r.need("activate an ability of")?;
        (r.word("a") || r.word("an")).then_some(())?;
        let mut filter = r.attempt(kind)?;
        r.need("source of the chosen type")?;
        filter.chosen_type = true;
        Some(filter)
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
        let spell = spells(r)?;
        let mut uses = vec![SpendOn::Spell(spell.clone())];
        if spell.chosen_type && r.word("or") {
            uses.push(SpendOn::Ability(ability_source(r)?));
        } else if let [one] = spell.subtypes.as_slice()
            && r.words("or activate abilities of")
        {
            // "Dragon spells or activate abilities of Dragons": the same subtype, in the plural.
            (plural_subtype(r)? == one.0).then_some(())?;
            uses.push(SpendOn::Ability(spell));
        }
        Some(uses)
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
        chosen_type: _,
    } = filter;
    if !subtypes.is_empty() {
        if !without.is_empty() || !supertypes.is_empty() || *multicolored {
            return None;
        }
        // "Elf spells", "Ninja or Turtle spells", "Dragon creature spells".
        let names: Vec<&str> = subtypes.iter().map(|one| one.0.as_str()).collect();
        let kinds: Vec<&str> = types.iter().map(|kind| type_name(*kind)).collect();
        let parts = [names.join(" or "), kinds.join(" or ")];
        let parts: Vec<&str> = parts
            .iter()
            .map(String::as_str)
            .filter(|part| !part.is_empty())
            .collect();
        return Some(format!("{} ", parts.join(" ")));
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
        [SpendOn::Spell(filter)] => format!(
            "Spend this mana only to cast {}spells{}",
            kind_text(filter)?,
            CHOSEN_TYPE.repeat(usize::from(filter.chosen_type))
        ),
        [SpendOn::Spell(spell), SpendOn::Ability(source)]
            if spell == source && spell.chosen_type =>
        {
            let kind = kind_text(source)?;
            let article = if kind.starts_with(['a', 'e', 'i', 'o', 'u']) {
                "an"
            } else {
                "a"
            };
            format!(
                "Spend this mana only to cast {kind}spells{CHOSEN_TYPE} or activate an ability of {article} {kind}source{CHOSEN_TYPE}"
            )
        }
        [SpendOn::Spell(spell), SpendOn::Ability(source)] if spell == source => {
            if spell.subtypes.is_empty() || !spell.types.is_empty() {
                return None;
            }
            let plurals: Vec<String> = spell
                .subtypes
                .iter()
                .map(|one| plural_name(&one.0))
                .collect();
            format!(
                "Spend this mana only to cast {}spells or activate abilities of {}",
                kind_text(spell)?,
                plurals.join(" or ")
            )
        }
        _ => return None,
    })
}

/// The wording with one spell in place of spells ("cast creature spells" is "cast a creature spell").
pub(super) fn singular(text: &str) -> Option<String> {
    let (head, nouns) = text.split_once("only to cast ")?;
    let (nouns, tail) = nouns.split_once("spells")?;
    let article = if nouns.starts_with(['a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U']) {
        "an"
    } else {
        "a"
    };
    Some(format!("{head}only to cast {article} {nouns}spell{tail}"))
}

/// The other wordings of a sentence about what mana is spent on: spells in the singular, "and/or" for a list of the
/// plural kinds ("instant and/or sorcery spells"), and the abilities of the same kind in each wording the cards use
/// ("or activate an ability of a Hero source", "or to activate abilities of Heroes").
pub(super) fn more(text: &str) -> Vec<String> {
    let mut forms = vec![text.to_owned()];
    forms.extend(singular(text));
    let mut found: Vec<String> = forms[1..].to_vec();
    for form in &forms {
        found.extend(and_or(form));
        found.extend(abilities(form));
    }
    found
}

/// "cast instant and/or sorcery spells" for "cast instant or sorcery spells".
fn and_or(text: &str) -> Option<String> {
    let (head, nouns) = text.split_once("only to cast ")?;
    let (kinds, tail) = nouns.split_once(" spells")?;
    (kinds.contains(" or ") && !kinds.starts_with("a ")).then(|| {
        format!(
            "{head}only to cast {} spells{tail}",
            kinds.replace(" or ", " and/or ")
        )
    })
}

/// The ways the abilities of the kind of the spells are worded after the spells.
fn abilities(text: &str) -> Vec<String> {
    let Some((head, rest)) = text.split_once(" or activate abilities of ") else {
        return Vec::new();
    };
    let (plurals, tail) = rest.find('.').map_or((rest, ""), |at| rest.split_at(at));
    // The kind in the singular, from the spells: "a Time Lord or Alien spell" or "Time Lord or Alien spells".
    let Some((_, spells)) = head.split_once("only to cast ") else {
        return Vec::new();
    };
    let kind = spells
        .strip_prefix("an ")
        .or_else(|| spells.strip_prefix("a "))
        .unwrap_or(spells);
    let kind = kind
        .strip_suffix(" spells")
        .or_else(|| kind.strip_suffix(" spell"))
        .unwrap_or(kind);
    let article = super::triggers::article(kind);
    let mut found = Vec::new();
    for to in ["", "to "] {
        for ability in [
            format!("abilities of {plurals}"),
            format!("an ability of {article} {kind}"),
            format!("an ability of {article} {kind} source"),
        ] {
            found.push(format!("{head} or {to}activate {ability}{tail}"));
        }
    }
    found.remove(0);
    found
}
