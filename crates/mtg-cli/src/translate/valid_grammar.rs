//! The rest of Forge's filter grammar: atoms the older readers leave (the cast of a spell, shared colors, stats against
//! the source, counted bounds, keyword parameters, extremes of a kind) and the table of atoms that stay gaps, each with
//! the fact of the game it needs.

use serde_json::{Value, json};

use super::amount::amount;
use super::ctx::{CARD_TYPES, Ctx, Tr, unmapped};

/// The creature types that make a party (CR 700.8).
const PARTY_TYPES: [&str; 4] = ["Cleric", "Rogue", "Warrior", "Wizard"];

/// The predicates of an atom that is one of these, `None` for any other atom.
fn atom(ctx: &Ctx, prop: &str) -> Tr<Option<Vec<Value>>> {
    let one = |value: Value| Ok(Some(vec![value]));
    match prop {
        "Kicked" => return one(json!("Kicked")),
        "Mayhem" => return one(json!({"CastWith": "Mayhem"})),
        "Party" => return party(ctx).map(Some),
        _ => {}
    }
    // `CastSa Spell.ManaFromTreasure`: the properties of the spell that cast the card are the card's own cast facts.
    if let Some(rest) = prop.strip_prefix("CastSa Spell.") {
        return cast_by(ctx, rest).map(Some);
    }
    // `EquippedByValid Equipment.Legendary` is the older `EquippedBy Equipment.Legendary`.
    for (head, plain) in [
        ("EquippedByValid ", "EquippedBy "),
        ("EnchantedByValid ", "EnchantedBy "),
    ] {
        if let Some(arg) = prop.strip_prefix(head) {
            return super::valid_atoms::argument(ctx, &format!("{plain}{arg}"));
        }
    }
    if let Some(word) = prop.strip_prefix("SharesColorWith ") {
        return shares_color(ctx, word).map(Some);
    }
    if let Some(found) = against_source(prop) {
        return one(found);
    }
    // `ManaSpent GTX` with `X` as `Count$CardPower`: the mana spent to cast the spell against the source's stat.
    if let Some(rest) = prop.strip_prefix("ManaSpent ")
        && let Some((op, bound)) = rest.get(..2).and_then(super::valid::cmp).zip(rest.get(2..))
        && let Some(stat) = ctx.svar(bound).and_then(|text| match text.trim() {
            "Count$CardPower" => Some("Power"),
            "Count$CardToughness" => Some("Toughness"),
            "Count$CardManaCost" => Some("ManaValue"),
            _ => None,
        })
    {
        return one(json!({"Test": {
            "left": {"Stat": {"of": "Candidate", "stat": "ManaSpent"}},
            "op": op,
            "right": {"Stat": {"of": "This", "stat": stat}},
        }}));
    }
    if let Some(found) = counted_bound(ctx, prop)? {
        return one(found);
    }
    Ok(None)
}

/// The atoms the readers of `valid` leave: this module's, then the atom table of `filter`; an unknown word is a gap, and
/// an atom of the table of gaps is named by its row (one key per missing fact, whatever its argument).
pub(super) fn rest(ctx: &Ctx, prop: &str) -> Tr<Vec<Value>> {
    if let Some(preds) = atom(ctx, prop)? {
        return Ok(preds);
    }
    super::filter::atom(ctx, ctx.scope.get(), prop).unwrap_or_else(|| {
        let word = unmapped_row(prop).map_or(prop, |row| row.word);
        unmapped(format!("valid:{word}"))
    })
}

/// Party creatures: one of the four types, when the world knows them all.
fn party(ctx: &Ctx) -> Tr<Vec<Value>> {
    if !PARTY_TYPES
        .iter()
        .all(|name| ctx.world.subtypes.contains(*name))
    {
        return unmapped("valid:Party");
    }
    let any: Vec<Value> = PARTY_TYPES
        .iter()
        .map(|name| json!({"Subtype": name}))
        .collect();
    Ok(vec![json!({"AnyOf": any})])
}

/// The cast facts after `CastSa Spell.`: each part read as a property of the card, a negated part refused.
fn cast_by(ctx: &Ctx, rest: &str) -> Tr<Vec<Value>> {
    let mut preds = Vec::new();
    for part in super::valid::split_properties(rest) {
        if part.starts_with('!') {
            return unmapped(format!("valid:CastSa Spell.{part}"));
        }
        preds.extend(super::valid::property(ctx, &part)?);
    }
    if preds.is_empty() {
        return unmapped("valid:CastSa");
    }
    Ok(preds)
}

/// `SharesColorWith Equipped`, `SharesColorWith Valid Creature.YouCtrl`: shares a color with one of the objects the
/// word names. Mana, a cast of this turn and a bare word stay gaps.
fn shares_color(ctx: &Ctx, word: &str) -> Tr<Vec<Value>> {
    let word = word.trim();
    let named = word.starts_with("Valid ") || matches!(word, "Equipped" | "Enchanted" | "Self");
    if !named {
        return unmapped(format!("valid:SharesColorWith {word}"));
    }
    let objects = super::valid::objects(super::valid::defined(ctx, word)?)?;
    Ok(vec![json!({ "SharesColor": objects })])
}

/// `powerLTCount$CardPower`: a stat of the object against the same or another stat of the card the ability belongs to.
fn against_source(prop: &str) -> Option<Value> {
    const LEFT: [(&str, &str); 3] = [
        ("power", "Power"),
        ("toughness", "Toughness"),
        ("cmc", "ManaValue"),
    ];
    const RIGHT: [(&str, &str); 3] = [
        ("Count$CardPower", "Power"),
        ("Count$CardToughness", "Toughness"),
        ("Count$CardManaCost", "ManaValue"),
    ];
    let (left, rest) = LEFT
        .iter()
        .find_map(|(word, stat)| prop.strip_prefix(word).map(|rest| (*stat, rest)))?;
    let op = super::valid::cmp(rest.get(..2)?)?;
    let right = RIGHT
        .iter()
        .find(|(word, _)| rest.get(2..) == Some(*word))
        .map(|(_, stat)| *stat)?;
    Some(json!({"Test": {
        "left": {"Stat": {"of": "Candidate", "stat": left}},
        "op": op,
        "right": {"Stat": {"of": "This", "stat": right}},
    }}))
}

/// `numColorsEQX`, `counters_GEX_CHARGE`: a characteristic or a counter count against a part the script defines. A color count
/// takes any amount; a counter count only a part that is a written number.
fn counted_bound(ctx: &Ctx, prop: &str) -> Tr<Option<Value>> {
    let named = |bound: &str| {
        bound.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && bound.chars().all(|c| c.is_ascii_alphanumeric())
    };
    if let Some(rest) = prop.strip_prefix("numColors")
        && let Some((op, bound)) = rest.get(..2).and_then(super::valid::cmp).zip(rest.get(2..))
        && named(bound)
        && ctx.svar(bound).is_some()
    {
        let to = amount(ctx, bound)?;
        // The X paid for the source says nothing of an object it compares (`Count$xPaid` in a static ability).
        if to == json!("X") {
            return unmapped(format!("valid:{prop}"));
        }
        return Ok(Some(
            json!({"Compare": {"stat": "Colors", "op": op, "to": to}}),
        ));
    }
    if let Some(rest) = prop.strip_prefix("counters_")
        && let Some((bound, kind)) = rest.split_once('_')
        && let Some((op, part)) = bound
            .get(..2)
            .and_then(super::valid::cmp)
            .zip(bound.get(2..))
        && named(part)
        && ctx.svar(part).is_some()
    {
        let Some(to) = amount(ctx, part)?
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
        else {
            return unmapped(format!("valid:{prop}"));
        };
        let Some(kind) = super::amount::counter_kind(kind) else {
            return Ok(None);
        };
        return Ok(Some(
            json!({"CounterCount": {"kind": kind, "op": op, "to": to}}),
        ));
    }
    Ok(None)
}

/// `_Creature` after `greatestCMC`: the card type an extreme compares among and the rest of the word (a controller word,
/// `ControlledByRemembered`), `None` for a word that names no card type.
pub(super) fn extreme_kind(rest: &str) -> Option<(Vec<Value>, &str)> {
    let after = rest.strip_prefix('_')?;
    let end = after.find("ControlledBy").unwrap_or(after.len());
    let (kind, tail) = after.split_at(end);
    CARD_TYPES
        .contains(&kind)
        .then(|| (vec![json!({"Type": kind})], tail))
}

/// An atom that stays a gap: the word (or its head, before a space) and the fact of the game it needs.
pub struct Unmapped {
    /// The atom, matched whole or as the head before an argument.
    pub word: &'static str,
    /// What the engine or the selector vocabulary lacks.
    pub needs: &'static str,
}

/// A row of the table of gaps.
const fn row(word: &'static str, needs: &'static str) -> Unmapped {
    Unmapped { word, needs }
}

/// The atoms in use in Forge's scripts that no reader maps, with the reason each stays a gap (`valid:<atom>`).
pub const UNMAPPED: &[Unmapped] = &[
    row(
        "hasManaAbility",
        "the abilities an object has, as a selector",
    ),
    row(
        "hasNonManaActivatedAbility",
        "the abilities an object has, as a selector",
    ),
    row("NoAbilities", "the abilities an object has, as a selector"),
    row("hasAbility", "the abilities an object has, as a selector"),
    row(
        "firstTurnControlled",
        "the turn control of a permanent began",
    ),
    row("attackedLastTurn", "the records of the previous turn"),
    row("dealtDamagetoAny", "damage records over the whole game"),
    row(
        "dealtCombatDamagetoAny",
        "damage records over the whole game",
    ),
    row(
        "Damaged",
        "damage records to a source that has left the battlefield",
    ),
    row("Above", "positions in a graveyard"),
    row("DirectlyAbove", "positions in a graveyard"),
    row("BottomGraveyard", "positions in a graveyard"),
    row(
        "BottomLibrary_Creature",
        "the bottommost card of a kind in a library",
    ),
    row("withFlanking", "the keyword flanking"),
    row("withoutFlanking", "the keyword flanking"),
    row("withoutRampage", "the keyword rampage"),
    row("withWarp", "the keyword warp"),
    row("withAugment", "the keyword augment"),
    row("kicked 1", "which of two kicker costs was paid"),
    row("kicked 2", "which of two kicker costs was paid"),
    row(
        "ManaSpent LTX",
        "a bound read against the spell or the source by script",
    ),
    row(
        "ManaSpent GTX",
        "a bound read against the spell or the source by script",
    ),
    row(
        "ManaFromCard.StrictlySelf",
        "mana spent from one named source",
    ),
    row("canProduceManaColor", "the mana an object could produce"),
    row("CostsHybridMana", "the symbols of a mana cost"),
    row("CostsPhyrexianMana", "the symbols of a mana cost"),
    row("SharesCMCWith", "a shared mana value predicate"),
    row("sharesLandTypeWith", "a shared land type predicate"),
    row(
        "sharesCreatureTypeWithAll",
        "a type shared with each of several objects",
    ),
    row("sharesControllerWith", "a shared controller predicate"),
    row("sharesOwnerWith", "a shared owner predicate"),
    row(
        "sharesBlockingAssignmentWith",
        "the blocks of one attacker as a selector",
    ),
    row(
        "attackingSame",
        "the attacked player or planeswalker of another attacker",
    ),
    row("WithSameName", "a group constraint of a cost"),
    row("WithDifferentNames", "a group constraint of a cost"),
    row("withTypesGE4", "a group constraint of a cost"),
    row("withTotalCMCGE30", "a group constraint of a cost"),
    row("HasDevoured", "what a permanent devoured"),
    row("exploited", "what a permanent exploited"),
    row("foretold", "foretold cards in exile"),
    row(
        "prepared",
        "a prepared spell (the spell, not the permanent)",
    ),
    row("copiedSpell", "copies of spells as a selector"),
    row("activated", "the loyalty activations of the turn"),
    row("enlistedThisCombat", "the enlist records of the combat"),
    row("hadToAttackThisCombat", "attack requirements as a selector"),
    row("noName", "nameless objects"),
    row("EnemyColor", "pairs of enemy colors"),
    row("ColorsYouCtrl", "the colors among your permanents"),
    row(
        "OwnerDoesntControl",
        "the owner of the object itself as a reference",
    ),
    row(
        "ControllerControlsMoreLand",
        "counts of the candidate's controller",
    ),
    row(
        "ControllerControlsLand_LE3",
        "counts of the candidate's controller",
    ),
    row("OppProtect", "the protector of a battle"),
    row("EnteredUnder", "the controller a permanent entered under"),
    row("numTypesGE2", "the number of card types of an object"),
    row("hasANonBasicLandType", "the nonbasic land types"),
    row("yardGreatestPower", "an extreme among graveyard cards"),
];

/// The row of an atom, matched whole or by the head before a space.
fn unmapped_row(prop: &str) -> Option<&'static Unmapped> {
    let head = prop.split(' ').next().unwrap_or(prop);
    UNMAPPED
        .iter()
        .find(|row| row.word == prop || row.word == head)
}
