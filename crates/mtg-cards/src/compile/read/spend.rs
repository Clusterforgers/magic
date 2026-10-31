//! "Spend this mana only to cast creature spells.": what the mana a sentence adds may be spent on, one use per sentence.

use std::fmt::Write as _;

use mtg_core::card::{CardType, Subtype, Supertype};

use super::selectors::{card_type, plural_subtype, subtype, type_name};
use super::subtypes::plural_subtype as plural_name;
use super::tokens::Reader;
use crate::spend::{ManaLasts, SpendFact, SpendFilter, SpendOn};

const LEAD: &str = "spend this mana only";

/// The words that name the creature type the source chose.
const CHOSEN_TYPE: &str = " of the chosen type";

/// Things joined by commas, "or" and "and/or": "Vampire, Cleric, and/or Demon", "instant or sorcery".
fn joined<T>(r: &mut Reader<'_>, one: impl Fn(&mut Reader<'_>) -> Option<T>) -> Option<Vec<T>> {
    let mut found = vec![one(r)?];
    while let Some(next) = r.attempt(|r| {
        let comma = r.punct(',');
        let both = r
            .attempt(|r| {
                r.word("and").then_some(())?;
                r.punct('/').then_some(())?;
                r.word("or").then_some(())
            })
            .is_some();
        let either = !both && r.word("or");
        (comma || both || either).then_some(())?;
        one(r)
    }) {
        found.push(next);
    }
    Some(found)
}

/// A card type in the singular ("artifact").
fn singular_type(r: &mut Reader<'_>) -> Option<CardType> {
    let found = card_type(r.peek_word()?)?;
    r.skip();
    Some(found)
}

/// A card type in the plural ("artifacts").
fn plural_type(r: &mut Reader<'_>) -> Option<CardType> {
    let found = card_type(r.peek_word()?.strip_suffix('s')?)?;
    r.skip();
    Some(found)
}

/// The words that stand for a spell of a kind: card types or subtypes (alone or with a card type), or an adjective.
fn kind(r: &mut Reader<'_>) -> Option<SpendFilter> {
    // "Dragon spells", "Vampire, Cleric, and/or Demon spells", "Dragon creature spells".
    if let Some(names) = r.attempt(|r| joined(r, subtype)) {
        let types = r.attempt(|r| joined(r, singular_type)).unwrap_or_default();
        return Some(SpendFilter {
            subtypes: names.into_iter().map(Subtype).collect(),
            types,
            ..SpendFilter::default()
        });
    }
    let adjective = match r.peek_word()? {
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
    if let Some(mut filter) = adjective {
        r.skip();
        // "legendary creature spells".
        if filter.supertypes.is_empty() {
            return Some(filter);
        }
        filter.types = r.attempt(|r| joined(r, singular_type)).unwrap_or_default();
        return Some(filter);
    }
    Some(SpendFilter {
        types: joined(r, singular_type)?,
        ..SpendFilter::default()
    })
}

/// What a spell is said to be after its noun, in any order: "of the chosen type", "with mana value 5 or greater", "with
/// {X} in their mana costs", "you don't own", "from your graveyard".
fn qualifiers(r: &mut Reader<'_>, filter: &mut SpendFilter) {
    loop {
        if r.words("of the chosen type") {
            filter.chosen_type = true;
        } else if let Some(value) = r.attempt(|r| {
            r.need("with mana value")?;
            let value = r.number()?;
            r.need("or greater")?;
            Some(value)
        }) {
            filter.min_value = value;
        } else if r
            .attempt(|r| {
                r.need("with")?;
                (r.symbol()? == "X").then_some(())?;
                r.need("in")?;
                (r.word("their") || r.word("its")).then_some(())?;
                r.need("mana")?;
                (r.word("costs") || r.word("cost")).then_some(())
            })
            .is_some()
        {
            filter.facts.push(SpendFact::XInCost);
        } else if r.words("you don't own") {
            filter.facts.push(SpendFact::NotOwned);
        } else if r.words("from your graveyard") {
            filter.facts.push(SpendFact::FromGraveyard);
        } else {
            return;
        }
    }
}

/// "creature spells", "a creature spell", "spells" or "a spell", then what is said of the spell.
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
    qualifiers(r, &mut filter);
    Some(filter)
}

/// "activate an ability of a creature source of the chosen type": a source of the kind the chosen type names.
fn chosen_source(r: &mut Reader<'_>) -> Option<SpendFilter> {
    r.need("an ability of")?;
    (r.word("a") || r.word("an")).then_some(())?;
    let mut filter = r.attempt(kind)?;
    r.need("source of the chosen type")?;
    filter.chosen_type = true;
    Some(filter)
}

/// The kind of source the abilities of a card are said to come from: "Elves", "Elemental sources", "artifacts", "a Hero
/// source" or "an artifact", as subtypes or card types.
fn source_kind(r: &mut Reader<'_>) -> Option<(Vec<Subtype>, Vec<CardType>)> {
    let subtypes = |names: Vec<String>| names.into_iter().map(Subtype).collect();
    if r.words("an ability of") {
        (r.word("a") || r.word("an")).then_some(())?;
        let found = match r.attempt(|r| joined(r, subtype)) {
            Some(names) => (subtypes(names), Vec::new()),
            None => (Vec::new(), joined(r, singular_type)?),
        };
        r.word("source");
        return Some(found);
    }
    r.need("abilities of")?;
    if let Some(names) = r.attempt(|r| joined(r, plural_subtype)) {
        return Some((subtypes(names), Vec::new()));
    }
    if let Some(types) = r.attempt(|r| joined(r, plural_type)) {
        return Some((Vec::new(), types));
    }
    let found = match r.attempt(|r| joined(r, subtype)) {
        Some(names) => (subtypes(names), Vec::new()),
        None => (Vec::new(), joined(r, singular_type)?),
    };
    r.need("sources")?;
    Some(found)
}

/// "activate abilities of Elves" (or "to activate an ability of a Dragon source"): the abilities of sources of the kind
/// the spells are, as the one use that lists them.
fn abilities_like(r: &mut Reader<'_>, spell: &SpendFilter) -> Option<SpendFilter> {
    r.attempt(|r| {
        r.word("to");
        r.need("activate")?;
        if spell.chosen_type {
            return chosen_source(r);
        }
        let plain = SpendFilter {
            types: spell.types.clone(),
            subtypes: spell.subtypes.clone(),
            ..SpendFilter::default()
        };
        let (subtypes, types) = source_kind(r)?;
        let same = (spell.subtypes.is_empty() != spell.types.is_empty())
            && subtypes == spell.subtypes
            && types == spell.types;
        same.then_some(plain)
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
        let first = spells(r)?;
        let mut uses = vec![SpendOn::Spell(first.clone())];
        // "or a legendary creature spell", "or spells with {X} in their mana costs", "or activate abilities of Elves".
        while let Some(next) = r.attempt(|r| {
            r.word("or").then_some(())?;
            spells(r)
                .map(SpendOn::Spell)
                .or_else(|| abilities_like(r, &first).map(SpendOn::Ability))
        }) {
            uses.push(next);
        }
        // An "or" left over is a use no reader knows.
        (r.peek_word() != Some("or")).then_some(uses)
    })
}

/// The sentence "Until end of turn, you don't lose this mana as steps and phases end." (or "Until end of combat, ...
/// as steps end.") read after the period that ends the sentence adding the mana.
pub(super) fn lasts(r: &mut Reader<'_>) -> Option<ManaLasts> {
    r.attempt(|r| {
        r.punct('.').then_some(())?;
        r.need("until end of")?;
        let lasts = if r.word("turn") {
            ManaLasts::Turn
        } else {
            r.need("combat")?;
            ManaLasts::Combat
        };
        r.punct(',').then_some(())?;
        r.need("you don't lose this mana as steps")?;
        if lasts == ManaLasts::Turn {
            r.need("and phases")?;
        }
        r.word("end").then_some(lasts)
    })
}

/// A list in words: "Elf", "Ninja or Turtle", "Vampire, Cleric, or Demon".
fn list_words(names: &[&str], last: &str) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first, second] => format!("{first} {last} {second}"),
        [init @ .., final_one] => format!("{}, {last} {final_one}", init.join(", ")),
    }
}

/// The kind of spell as the plural noun phrase reads, up to "spells": `None` when no wording of the kind exists.
fn kind_text(filter: &SpendFilter) -> Option<String> {
    let SpendFilter {
        types,
        without,
        supertypes,
        subtypes,
        multicolored,
        ..
    } = filter;
    let kinds: Vec<&str> = types.iter().map(|kind| type_name(*kind)).collect();
    if !subtypes.is_empty() {
        if !without.is_empty() || !supertypes.is_empty() || *multicolored {
            return None;
        }
        // "Elf spells", "Ninja or Turtle spells", "Dragon creature spells".
        let names: Vec<&str> = subtypes.iter().map(|one| one.0.as_str()).collect();
        let parts = [list_words(&names, "or"), kinds.join(" or ")];
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
        (types, [], [Supertype::Legendary], false) => {
            Some(format!("legendary {}", kind_words(types)))
        }
        ([], [], [], true) => Some("multicolored ".into()),
        (types, [], [], false) if !types.is_empty() => Some(kind_words(types)),
        _ => None,
    }
}

/// The card types in the singular, joined by "or", with the space after: "instant or sorcery ".
fn kind_words(types: &[CardType]) -> String {
    let names: Vec<&str> = types.iter().map(|t| type_name(*t)).collect();
    if names.is_empty() {
        String::new()
    } else {
        format!("{} ", names.join(" or "))
    }
}

/// What is said of the spell after its noun.
fn qualifier_text(filter: &SpendFilter) -> String {
    let mut text = String::new();
    if filter.chosen_type {
        text.push_str(CHOSEN_TYPE);
    }
    if filter.min_value > 0 {
        let _ = write!(text, " with mana value {} or greater", filter.min_value);
    }
    for fact in &filter.facts {
        text.push_str(match fact {
            SpendFact::XInCost => " with {X} in their mana costs",
            SpendFact::NotOwned => " you don't own",
            SpendFact::FromGraveyard => " from your graveyard",
            SpendFact::SharesCommanderType => " that shares a creature type with your commander",
        });
    }
    text
}

/// "creature spells of the chosen type": the spells of a kind with what is said of them.
fn spells_text(filter: &SpendFilter) -> Option<String> {
    Some(format!(
        "{}spells{}",
        kind_text(filter)?,
        qualifier_text(filter)
    ))
}

/// The same with one spell and its article: "a creature spell of the chosen type".
fn one_spell_text(filter: &SpendFilter) -> Option<String> {
    let kind = kind_text(filter)?;
    Some(format!(
        "{} {kind}spell{}",
        super::triggers::article(&kind),
        qualifier_text(filter)
    ))
}

/// The sentence the uses are written as, with its capital and no period; `None` for uses the readers do not write.
pub(super) fn text(only: &[SpendOn]) -> Option<String> {
    Some(match only {
        [SpendOn::CostWithX] => "Spend this mana only on costs that contain {X}".to_owned(),
        [SpendOn::Ability(filter)] if *filter == SpendFilter::default() => {
            "Spend this mana only to activate abilities".to_owned()
        }
        [SpendOn::Spell(filter)] => {
            format!("Spend this mana only to cast {}", spells_text(filter)?)
        }
        [SpendOn::Spell(spell), SpendOn::Ability(source)]
            if spell == source && spell.chosen_type =>
        {
            let kind = kind_text(source)?;
            format!(
                "Spend this mana only to cast {kind}spells{CHOSEN_TYPE} or activate an ability of {} {kind}source{CHOSEN_TYPE}",
                super::triggers::article(&kind)
            )
        }
        [SpendOn::Spell(spell), SpendOn::Ability(source)] if spell == source => {
            format!(
                "Spend this mana only to cast {} or activate abilities of {}",
                spells_text(spell)?,
                plural_sources(spell)?
            )
        }
        [SpendOn::Spell(first), SpendOn::Spell(second)] => format!(
            "Spend this mana only to cast {} or {}",
            if first.chosen_type || second.chosen_type {
                one_spell_text(first)?
            } else {
                spells_text(first)?
            },
            if first.chosen_type || second.chosen_type {
                one_spell_text(second)?
            } else {
                spells_text(second)?
            }
        ),
        _ => return None,
    })
}

/// The sources whose abilities are paid for, as the plural names them: "Elves", "artifacts"; `None` for a kind that
/// has a subtype with a card type, a filter of other parts, or none.
fn plural_sources(spell: &SpendFilter) -> Option<String> {
    let plain = spell.without.is_empty()
        && spell.supertypes.is_empty()
        && !spell.multicolored
        && spell.min_value == 0
        && spell.facts.is_empty();
    if !plain || spell.subtypes.is_empty() == spell.types.is_empty() {
        return None;
    }
    let plurals: Vec<String> = if spell.subtypes.is_empty() {
        spell
            .types
            .iter()
            .map(|kind| format!("{}s", type_name(*kind)))
            .collect()
    } else {
        spell
            .subtypes
            .iter()
            .map(|one| plural_name(&one.0))
            .collect()
    };
    let names: Vec<&str> = plurals.iter().map(String::as_str).collect();
    Some(list_words(&names, "or"))
}

/// The wording with one spell in place of spells ("cast creature spells" is "cast a creature spell").
pub(super) fn singular(text: &str) -> Option<String> {
    let (head, nouns) = text.split_once("only to cast ")?;
    let (nouns, tail) = nouns.split_once("spells")?;
    let article = super::triggers::article(nouns);
    Some(format!("{head}only to cast {article} {nouns}spell{tail}"))
}

/// The other wordings of a sentence about what mana is spent on: spells in the singular, "and/or" for a list of the
/// plural kinds ("instant and/or sorcery spells"), and the abilities of the same kind in each wording the cards use
/// ("or activate an ability of a Hero source", "or to activate abilities of Heroes", "or activate abilities of Elemental
/// sources").
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

/// "cast instant and/or sorcery spells" for "cast instant or sorcery spells", "Vampire, Cleric, and/or Demon spells" for
/// "Vampire, Cleric, or Demon spells".
fn and_or(text: &str) -> Option<String> {
    let (head, nouns) = text.split_once("only to cast ")?;
    let (kinds, tail) = nouns.split_once(" spells")?;
    (kinds.contains(" or ") && !kinds.contains("spell") && !kinds.starts_with("a ")).then(|| {
        format!(
            "{head}only to cast {} spells{tail}",
            kinds
                .replace(", or ", ", and/or ")
                .replace(" or ", " and/or ")
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
            format!("abilities of {kind} sources"),
            format!("an ability of {article} {kind}"),
            format!("an ability of {article} {kind} source"),
        ] {
            found.push(format!("{head} or {to}activate {ability}{tail}"));
        }
    }
    found.remove(0);
    found
}
