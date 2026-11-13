//! "Spend this mana only to cast creature spells.": what the mana a sentence adds may be spent on, one use per sentence.

use std::fmt::Write as _;

use mtg_core::card::{CardType, Subtype, Supertype};

use super::selectors::{card_type, plural_subtype, subtype, type_name};
use super::subtypes::plural_subtype as plural_name;
use super::tokens::Reader;
use crate::keyword::Keyword;
use crate::spend::{EntersBonus, ManaLasts, SpendFact, SpendFilter, SpendOn};

const LEAD: &str = "spend this mana only";

/// The words that name the creature type the source chose.
const CHOSEN_TYPE: &str = " of the chosen type";

/// The creature types "outlaw" names (CR 205.3m), in the order the subtypes of a filter list them.
const OUTLAWS: [&str; 5] = ["Assassin", "Mercenary", "Pirate", "Rogue", "Warlock"];

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
    // "colorless spells", "colorless Eldrazi spells".
    if r.word("colorless") {
        let mut filter = r.attempt(kind).unwrap_or_default();
        filter.facts.insert(0, SpendFact::Colorless);
        return Some(filter);
    }
    // "kicked spells", "a kicked spell".
    if r.word("kicked") {
        let mut filter = r.attempt(kind).unwrap_or_default();
        filter.facts.insert(0, SpendFact::Kicked);
        return Some(filter);
    }
    // "non-Human creature spells".
    if let Some(name) = r.peek_word().and_then(|word| word.strip_prefix("non-")) {
        let mut chars = name.chars();
        let name = format!("{}{}", chars.next()?.to_uppercase(), chars.as_str());
        r.is_subtype(&name).then_some(())?;
        r.skip();
        let mut filter = r.attempt(kind).unwrap_or_default();
        filter.not_subtypes.insert(0, Subtype(name));
        return Some(filter);
    }
    // "monocolored spells".
    if r.word("monocolored") {
        let mut filter = r.attempt(kind).unwrap_or_default();
        filter.facts.insert(0, SpendFact::Monocolored);
        return Some(filter);
    }
    // "outlaw spells" (CR 205.3m).
    if r.word("outlaw") {
        return Some(SpendFilter {
            subtypes: OUTLAWS
                .iter()
                .map(|name| Subtype((*name).to_owned()))
                .collect(),
            ..SpendFilter::default()
        });
    }
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
        } else if r.words("of that color") {
            filter.chosen_color = true;
        } else if r.words("that's one or more colors") {
            filter.facts.push(SpendFact::Colored);
        } else if r
            .attempt(|r| {
                r.need("without")?;
                (r.symbol()? == "X").then_some(())?;
                r.need("in its mana cost")
            })
            .is_some()
        {
            filter.facts.push(SpendFact::NoXInCost);
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
            filter.facts.push(SpendFact::FromYourGraveyard);
        } else if r.words("from a graveyard") {
            filter.facts.push(SpendFact::FromGraveyard);
        } else if r.words("from exile") {
            filter.facts.push(SpendFact::FromExile);
        } else if r.words("from anywhere other than your hand") {
            filter.facts.push(SpendFact::NotFromHand);
        } else if r.words("with devoid") {
            filter.facts.push(SpendFact::Devoid);
        } else if r.words("with flashback") {
            filter.facts.push(SpendFact::Flashback);
        } else if r.words("that has freerunning") || r.words("with freerunning") {
            filter.facts.push(SpendFact::Freerunning);
        } else if r.words("with no abilities") {
            filter.facts.push(SpendFact::NoAbilities);
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
    if r.words("outlaw sources") {
        return Some((subtypes(OUTLAWS.map(str::to_owned).to_vec()), Vec::new()));
    }
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

/// "This mana can't be spent to cast spells from your hand.": the spells cast from anywhere else and the abilities.
const NOT_FROM_HAND: &str = "This mana can't be spent to cast spells from your hand";

/// The spells `NOT_FROM_HAND` allows.
fn not_from_hand() -> SpendFilter {
    SpendFilter {
        facts: vec![SpendFact::NotFromHand],
        ..SpendFilter::default()
    }
}

/// "This mana can't be spent to cast nonartifact spells.": the spells of the type, the abilities and the other costs.
fn not_of_type(r: &mut Reader<'_>) -> Option<Vec<SpendOn>> {
    r.need("this mana can't be spent to cast")?;
    let card_type = card_type(r.peek_word()?.strip_prefix("non")?)?;
    r.skip();
    r.need("spells")?;
    Some(vec![
        SpendOn::Spell(of_type(card_type)),
        SpendOn::abilities(),
        SpendOn::OtherCosts,
    ])
}

/// The spells of one card type.
fn of_type(card_type: CardType) -> SpendFilter {
    SpendFilter {
        types: vec![card_type],
        ..SpendFilter::default()
    }
}

/// "or to activate an ability" after the spells: the abilities of any source.
fn any_ability(r: &mut Reader<'_>) -> Option<SpendOn> {
    r.attempt(|r| {
        r.word("to");
        r.need("activate an ability")?;
        // "an ability of an Elf" is the abilities of a kind, which `abilities_like` reads.
        (r.peek_word() != Some("of")).then(SpendOn::abilities)
    })
}

/// The spells cast for their disturb cost: "pay a disturb cost".
fn disturb() -> SpendFilter {
    SpendFilter {
        facts: vec![SpendFact::Disturb],
        ..SpendFilter::default()
    }
}

/// The equip abilities of any source.
fn equip() -> SpendFilter {
    SpendFilter {
        facts: vec![SpendFact::Equip],
        ..SpendFilter::default()
    }
}

/// "activate an equip ability" or "activate equip abilities" after the spells.
fn equip_abilities(r: &mut Reader<'_>) -> Option<SpendOn> {
    r.attempt(|r| {
        r.need("activate")?;
        (r.words("an equip ability") || r.words("equip abilities"))
            .then(|| SpendOn::Ability(equip()))
    })
}

/// The sentence "Spend this mana only to ..." read after the period that ends the sentence adding the mana.
pub(super) fn only(r: &mut Reader<'_>) -> Option<Vec<SpendOn>> {
    r.attempt(|r| {
        r.punct('.').then_some(())?;
        if r.words("this mana can't be spent to cast spells from your hand") {
            return Some(vec![
                SpendOn::Spell(not_from_hand()),
                SpendOn::abilities(),
                SpendOn::OtherCosts,
            ]);
        }
        if let Some(only) = r.attempt(not_of_type) {
            return Some(only);
        }
        r.need(LEAD)?;
        if r.words("on costs that contain") {
            return (r.symbol()? == "X").then(|| vec![SpendOn::CostWithX]);
        }
        r.need("to")?;
        if r.words("activate abilities") {
            return Some(vec![SpendOn::abilities()]);
        }
        if r.words("pay cumulative upkeep costs") {
            return Some(vec![SpendOn::CumulativeUpkeep]);
        }
        // "to pay a disturb cost or cast an instant or sorcery spell".
        if r.words("pay a disturb cost or cast") {
            return Some(vec![SpendOn::Spell(disturb()), SpendOn::Spell(spells(r)?)]);
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
                .or_else(|| equip_abilities(r))
                .or_else(|| any_ability(r))
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
    let mut kind = plain_kind_text(filter)?;
    // "non-Human creature spells".
    for name in filter.not_subtypes.iter().rev() {
        kind = format!("non-{} {kind}", name.0);
    }
    // "monocolored spells".
    if filter.facts.contains(&SpendFact::Monocolored) {
        kind = format!("monocolored {kind}");
    }
    // "colorless spells", "colorless Eldrazi spells", "kicked spells".
    Some(if filter.facts.contains(&SpendFact::Colorless) {
        format!("colorless {kind}")
    } else if filter.facts.contains(&SpendFact::Kicked) {
        format!("kicked {kind}")
    } else {
        kind
    })
}

/// The kind of spell as `kind_text` reads it, without the colour words.
fn plain_kind_text(filter: &SpendFilter) -> Option<String> {
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
        let names = if names == OUTLAWS {
            "outlaw".to_owned()
        } else {
            list_words(&names, "or")
        };
        let parts = [names, kinds.join(" or ")];
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
        // "a multicolored creature spell".
        (types, [], [], true) if !types.is_empty() => {
            Some(format!("multicolored {}", kind_words(types)))
        }
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
    if filter.chosen_color {
        text.push_str(" of that color");
    }
    if filter.min_value > 0 {
        let _ = write!(text, " with mana value {} or greater", filter.min_value);
    }
    // "spells with flashback from a graveyard": what a spell has comes before where it is cast from.
    let mut facts: Vec<&SpendFact> = filter.facts.iter().collect();
    facts.sort_by_key(|fact| {
        matches!(
            fact,
            SpendFact::FromGraveyard | SpendFact::FromYourGraveyard | SpendFact::FromExile
        )
    });
    for fact in facts {
        match fact {
            SpendFact::XInCost => text.push_str(" with {X} in their mana costs"),
            SpendFact::NotOwned => text.push_str(" you don't own"),
            SpendFact::FromGraveyard => text.push_str(" from a graveyard"),
            SpendFact::FromYourGraveyard => text.push_str(" from your graveyard"),
            SpendFact::SharesCommanderType => {
                text.push_str(" that shares a creature type with your commander");
            }
            SpendFact::OnBattlefield => text.push_str(" on the battlefield"),
            // Colorless and monocolored are written before the noun (`kind_text`), your commander alone
            // (`ep33_spend_text`), equip abilities and disturb costs by `text`; `text` writes no face-down spells.
            SpendFact::Colorless
            | SpendFact::Kicked
            | SpendFact::YourCommander
            | SpendFact::FaceDown
            | SpendFact::Monocolored
            | SpendFact::Equip
            | SpendFact::Disturb => {}
            SpendFact::FromExile => text.push_str(" from exile"),
            SpendFact::NotFromHand => text.push_str(" from anywhere other than your hand"),
            SpendFact::Devoid => text.push_str(" with devoid"),
            SpendFact::Flashback => text.push_str(" with flashback"),
            SpendFact::Freerunning => text.push_str(" with freerunning"),
            SpendFact::NoAbilities => text.push_str(" with no abilities"),
            // One spell's words: `text` writes a filter with them one spell at a time.
            SpendFact::Colored => text.push_str(" that's one or more colors"),
            SpendFact::NoXInCost => text.push_str(" without {X} in its mana cost"),
        }
    }
    text
}

/// "creature spells of the chosen type": the spells of a kind with what is said of them.
fn spells_text(filter: &SpendFilter) -> Option<String> {
    if unworded(filter) {
        return None;
    }
    Some(format!(
        "{}spells{}",
        kind_text(filter)?,
        qualifier_text(filter)
    ))
}

/// The same with one spell and its article: "a creature spell of the chosen type".
fn one_spell_text(filter: &SpendFilter) -> Option<String> {
    if unworded(filter) {
        return None;
    }
    let kind = kind_text(filter)?;
    // One spell "has" freerunning: "a spell that has freerunning".
    let qualifier = qualifier_text(filter).replace(" with freerunning", " that has freerunning");
    Some(format!(
        "{} {kind}spell{qualifier}",
        super::triggers::article(&kind),
    ))
}

/// The sentence the uses are written as, with its capital and no period; `None` for uses the readers do not write.
pub(super) fn text(only: &[SpendOn]) -> Option<String> {
    if let Some(text) = super::ep33_spend::ep33_spend_text(only) {
        return Some(text);
    }
    if let Some(text) = r405_text(only) {
        return Some(text);
    }
    // The facts no other wording says are left to `ep33_spend_text`.
    let unsaid = |filter: &SpendFilter| {
        filter
            .facts
            .iter()
            .any(|fact| matches!(fact, SpendFact::YourCommander | SpendFact::FaceDown))
    };
    if only.iter().any(|on| match on {
        SpendOn::Spell(filter) | SpendOn::Ability(filter) => unsaid(filter),
        SpendOn::Special(_) => true,
        _ => false,
    }) {
        return None;
    }
    Some(match only {
        [SpendOn::Enters(bonus)] => enters_text(bonus)?,
        [SpendOn::CostWithX] => "Spend this mana only on costs that contain {X}".to_owned(),
        [SpendOn::CumulativeUpkeep] => {
            "Spend this mana only to pay cumulative upkeep costs".to_owned()
        }
        [SpendOn::Ability(filter)] if *filter == SpendFilter::default() => {
            "Spend this mana only to activate abilities".to_owned()
        }
        [SpendOn::Spell(filter)] if !unworded(filter) => {
            format!("Spend this mana only to cast {}", spells_text(filter)?)
        }
        [
            SpendOn::Spell(spell),
            SpendOn::Ability(source),
            SpendOn::OtherCosts,
        ] if *source == SpendFilter::default() && *spell == not_from_hand() => {
            NOT_FROM_HAND.to_owned()
        }
        [
            SpendOn::Spell(spell),
            SpendOn::Ability(source),
            SpendOn::OtherCosts,
        ] if *source == SpendFilter::default()
            && let [card_type] = spell.types[..]
            && *spell == of_type(card_type) =>
        {
            format!(
                "This mana can't be spent to cast non{} spells",
                type_name(card_type)
            )
        }
        // "Spend this mana only to cast a colorless spell or to activate an ability."
        [SpendOn::Spell(spell), SpendOn::Ability(source)]
            if *source == SpendFilter::default() && *spell != SpendFilter::default() =>
        {
            format!(
                "Spend this mana only to cast {} or to activate an ability",
                one_spell_text(spell)?
            )
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
        // "Spend this mana only to cast an Assassin spell or a spell that has freerunning, or to activate an ability of an
        // Assassin source."
        [
            SpendOn::Spell(first),
            SpendOn::Spell(second),
            SpendOn::Ability(source),
        ] if *source != SpendFilter::default() && source.facts.is_empty() => {
            let kind = kind_text(source)?;
            format!(
                "Spend this mana only to cast {} or {}, or to activate an ability of {} {kind}source",
                one_spell_text(first)?,
                one_spell_text(second)?,
                super::triggers::article(&kind)
            )
        }
        [SpendOn::Spell(first), SpendOn::Spell(second)] => format!(
            "Spend this mana only to cast {} or {}",
            if r281_singular(first, second) {
                one_spell_text(first)?
            } else {
                spells_text(first)?
            },
            if r281_singular(first, second) {
                one_spell_text(second)?
            } else {
                spells_text(second)?
            }
        ),
        _ => return None,
    })
}

/// The sentences of the r40-5 uses: a spell that's one or more colors without {X}, a disturb cost, equip abilities;
/// `None` for any other uses.
fn r405_text(only: &[SpendOn]) -> Option<String> {
    Some(match only {
        // "Spend this mana only to cast a spell that's one or more colors without {X} in its mana cost."
        [SpendOn::Spell(filter)]
            if filter
                .facts
                .iter()
                .any(|fact| matches!(fact, SpendFact::Colored | SpendFact::NoXInCost)) =>
        {
            format!("Spend this mana only to cast {}", one_spell_text(filter)?)
        }
        // "Spend this mana only to pay a disturb cost or cast an instant or sorcery spell."
        [SpendOn::Spell(first), SpendOn::Spell(spell)]
            if *first == disturb() && !unworded(spell) =>
        {
            format!(
                "Spend this mana only to pay a disturb cost or cast {}",
                one_spell_text(spell)?
            )
        }
        // "Spend this mana only to cast Equipment spells or activate equip abilities."
        [SpendOn::Spell(spell), SpendOn::Ability(source)]
            if *source == equip() && !unworded(spell) =>
        {
            format!(
                "Spend this mana only to cast {} or activate equip abilities",
                spells_text(spell)?
            )
        }
        _ => return None,
    })
}

/// Whether a filter of spells holds a fact only `text` words as a use of its own (an equip ability, a disturb cost).
fn unworded(filter: &SpendFilter) -> bool {
    filter
        .facts
        .iter()
        .any(|fact| matches!(fact, SpendFact::Equip | SpendFact::Disturb))
}

/// Whether two kinds of spell are named one spell at a time: "a creature spell of the chosen type or ...", "an instant
/// or sorcery spell or a kicked spell".
fn r281_singular(first: &SpendFilter, second: &SpendFilter) -> bool {
    [first, second]
        .iter()
        .any(|filter| filter.chosen_type || filter.facts.contains(&SpendFact::Kicked))
}

/// The sentence of a rider that gives the spell's permanent counters or haste: "If that mana is spent on a creature
/// spell, it gains haste until end of turn"; `None` for more than one counter, both at once, or another keyword.
fn enters_text(bonus: &EntersBonus) -> Option<String> {
    let spell = one_spell_text(&bonus.spell)?;
    let gets = match (bonus.counters, &bonus.keyword) {
        (1, None) => "that creature enters with an additional +1/+1 counter on it".to_owned(),
        (0, Some(Keyword::Haste)) if bonus.this_turn => {
            "it gains haste until end of turn".to_owned()
        }
        (0, Some(Keyword::Haste)) => "it gains haste".to_owned(),
        _ => return None,
    };
    Some(format!("If that mana is spent on {spell}, {gets}"))
}

/// The other wordings of a rider's sentence: "any of that mana" for mana of several, "this mana", and "spent to cast".
fn enters_forms(text: &str) -> Vec<String> {
    let Some(rest) = text.strip_prefix("If that mana is spent on ") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for subject in ["that mana", "any of that mana", "this mana"] {
        for verb in ["spent on", "spent to cast"] {
            found.push(format!("If {subject} is {verb} {rest}"));
        }
    }
    found.remove(0);
    found
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
    if text.contains("If that mana is spent on ") {
        return text
            .split_once("If that mana")
            .map(|(head, _)| head)
            .map_or_else(Vec::new, |head| {
                enters_forms(&text[head.len()..])
                    .into_iter()
                    .map(|form| format!("{head}{form}"))
                    .collect()
            });
    }
    // "cast an Equipment spell or activate an equip ability".
    if let Some((head, tail)) = text.split_once(" or activate equip abilities") {
        return singular(head)
            .map(|one| format!("{one} or activate an equip ability{tail}"))
            .into_iter()
            .collect();
    }
    let mut forms = vec![text.to_owned()];
    forms.extend(singular(text));
    let mut found: Vec<String> = forms[1..].to_vec();
    found.extend(pair_forms(text));
    for form in &forms {
        found.extend(and_or(form));
        found.extend(abilities(form));
    }
    found
}

/// "cast a Knight spell or an Equipment spell" and "cast a Knight or Equipment spell" for "cast Knight spells or Equipment
/// spells"; the short form only for two single-word kinds, where "or" cannot join a longer phrase.
fn pair_forms(text: &str) -> Vec<String> {
    let Some((head, nouns)) = text.split_once("only to cast ") else {
        return Vec::new();
    };
    let Some((first, rest)) = nouns.split_once(" spells or ") else {
        return Vec::new();
    };
    let Some((second, tail)) = rest.split_once(" spells") else {
        return Vec::new();
    };
    let plain = |kind: &str| {
        !kind.is_empty()
            && !kind.contains(" or ")
            && !kind.contains(',')
            && !kind.contains("spell")
            && !kind.contains(" of ")
    };
    if !(plain(first) && plain(second)) || tail.contains("spell") {
        return Vec::new();
    }
    let article = super::triggers::article;
    let mut forms = vec![format!(
        "{head}only to cast {} {first} spell or {} {second} spell{tail}",
        article(first),
        article(second)
    )];
    if !first.contains(' ') && !second.contains(' ') {
        forms.push(format!(
            "{head}only to cast {} {first} or {second} spell{tail}",
            article(first)
        ));
    }
    forms
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
